# Tool use orchestration

## Can the model read files, write files, run shell commands, call APIs? How are those capabilities exposed and sequenced?

In this harness, the model can trigger operations that read files, write/edit files, run shell commands, and fetch remote HTTP/HTTPS content. It can also invoke delegated agent tools and MCP-backed integrations.


## How capabilities are exposed

Capabilities are exposed as a structured tool catalog (ToolCatalog) that includes file ops, shell, network fetch, planning/skills, etc.:
crates/forge_domain/src/tools/catalog.rs:41-58

Key capability families in that catalog include:
• File read/write/search/edit/remove/undo: Read, Write, FsSearch, Patch, Remove, Undo
  crates/forge_domain/src/tools/catalog.rs:42-51
• Shell execution: Shell
  crates/forge_domain/src/tools/catalog.rs:51
• API/network fetch (HTTP/HTTPS text): Fetch
  crates/forge_domain/src/tools/catalog.rs:52
• Additional integration surfaces: delegated agents + MCP tools are routed in registry logic
  crates/forge_app/src/tool_registry.rs:141-157

The runtime builds an overview of system tools + agent tools + MCP tools for exposure:
crates/forge_app/src/tool_registry.rs:211-232
crates/forge_app/src/tool_registry.rs:236-287


## How they are sequenced

The harness controls sequencing as a loop:

1. Send request to model
   crates/forge_app/src/orch.rs:209-253

1. Inspect model response (complete/yield/tool path)
   crates/forge_app/src/orch.rs:264-275

1. Execute tool calls via runtime dispatch
   crates/forge_app/src/orch.rs:277-279
   crates/forge_app/src/agent.rs:55-63
   crates/forge_app/src/tool_registry.rs:181-192

1. Dispatch target
      ◦ Built-in system capability execution
     crates/forge_app/src/tool_registry.rs:105-140
      • Agent delegation
     crates/forge_app/src/tool_registry.rs:141-153
      • MCP execution
     crates/forge_app/src/tool_registry.rs:154-176

1. Append assistant output + tool results back into context, then next pass
   crates/forge_app/src/orch.rs:304-312

So the model proposes actions; the harness executes, records results, and decides whether to continue/yield/interrupt.


## Concrete execution mapping (examples)

Inside execution mapping:
• File read/write: services.read(...), services.write(...)
  crates/forge_app/src/tool_executor.rs:157-176
• Shell command execution: services.execute(...)
  crates/forge_app/src/tool_executor.rs:251-269
• HTTP/HTTPS fetch: services.fetch(...)
  crates/forge_app/src/tool_executor.rs:270-273


## Guardrails / constraints

• Read-before-edit enforcement for patch and overwrite writes:
  crates/forge_app/src/tool_executor.rs:329-339
• Timeout around tool calls:
  crates/forge_app/src/tool_registry.rs:47-62
• Permission checks in restricted mode before execution:
  crates/forge_app/src/tool_registry.rs:113-127

So: capabilities exist and are broad, but execution is policy/timeout/guardrail-mediated by the harness.
