# Mental Model

## Missing gaps in the flow (high level)

1. Tool inventory is assembled before calls
      ◦ The runtime has a canonical tool catalog (read, write, fs_search, patch, shell, fetch, etc.).
     crates/forge_domain/src/tools/catalog.rs:41-58
      • It builds a tools overview that combines:
          ◦ system tools
          ◦ agent-delegation tools
          ◦ MCP tools
     crates/forge_app/src/tool_registry.rs:211-232
      • System tool exposure is dynamically rendered from environment/model context.
     crates/forge_app/src/tool_registry.rs:236-287

1. How the model knows available tools
      ◦ Tool definitions are serialized into the outbound provider request (tools and tool_choice from context).
     crates/forge_repo/src/provider/openai_responses/request.rs:281-301

1. The harness loop controls execution
      ◦ Per iteration: send request, get response, detect tool calls, execute tools, append results back to context, repeat.
     crates/forge_app/src/orch.rs:209-312

1. Execution is guarded (not raw/unbounded)
      ◦ Tool calls are validated against what the active agent is allowed to use.
     crates/forge_app/src/tool_registry.rs:293-307
      • Restricted-mode policy checks can deny operations before execution.
     crates/forge_app/src/tool_registry.rs:64-74
     crates/forge_app/src/tool_registry.rs:113-127
      • Tool timeouts are enforced.
     crates/forge_app/src/tool_registry.rs:47-62
      • Read-before-edit is enforced for patch/overwrite writes.
     crates/forge_app/src/tool_executor.rs:329-339

1. Stop/yield/interrupt conditions
      ◦ Complete when finish reason is stop and no tool calls.
     crates/forge_app/src/orch.rs:264-268
      • Yield when follow-up is requested.
     crates/forge_app/src/orch.rs:270-275
      • Interrupt on safety limits (tool failures / request count).
     crates/forge_app/src/orch.rs:313-351
     crates/forge_domain/src/chat_response.rs:97-105

────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────


## High-level Mermaid flow (until completion)

flowchart TD
    A[User prompt] --> B[Build/refresh context\nmessages + tool definitions]
    B --> C[Request phase\n(on_request hooks)]
    C --> D[Send model call]

    D --> E{Call error?}
    E -- Retryable --> R[Retry with backoff] --> D
    E -- Non-retryable / exhausted --> X[Interrupt/exit]

    E -- No --> F[Model response\n(on_response hooks)]
    F --> G{finish=stop AND no tool calls?}
    G -- Yes --> TC[TaskComplete] --> Z([End])

    G -- No --> H{followup tool requested?}
    H -- Yes --> Y[Yield to user for clarification] --> Z

    H -- No --> I[Execute tool calls]
    I --> J[Per-call guards:\nallowed tool, policy checks,\ntimeout, read-before-edit]
    J --> K[Run tool implementation\n(file/shell/fetch/agent/MCP)]
    K --> L[Collect tool outputs]
    L --> M[Append assistant output + tool results to context]
    M --> N{Limits reached?\nmax tool failures or max requests/turn}
    N -- Yes --> X
    N -- No --> C
