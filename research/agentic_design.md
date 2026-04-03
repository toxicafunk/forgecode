# Agentic Loop Design

## Does the harness feed model output back in for another pass?

Yes. The orchestrator runs a multi-pass loop (`while !should_yield`) and, on each pass, it:

1. Calls the model with the current context.
2. Executes returned tool calls.
3. Appends assistant output + tool call records + tool results back into context.
4. Starts another pass unless a yield/stop condition is hit.

References:
- `crates/forge_app/src/orch.rs:209-253`
- `crates/forge_app/src/orch.rs:277-312`
- `crates/forge_domain/src/context.rs:549-593`

## Under what conditions does it stop?

The loop exits when `should_yield` becomes true.

1. **Task is complete**: finish reason is `stop` and there are no tool calls.
   - `crates/forge_app/src/orch.rs:264-268`

2. **A follow-up/yield tool is called**: any tool that maps to follow-up behavior triggers yield.
   - `crates/forge_app/src/orch.rs:270-275`
   - `crates/forge_domain/src/tools/catalog.rs:844-850`

3. **Too many tool failures in a turn**: tool error tracker reaches configured limit, emits interrupt, and yields.
   - `crates/forge_app/src/orch.rs:313-323`
   - `crates/forge_domain/src/tools/call/tool_call.rs:307-319`
   - `crates/forge_domain/src/chat_response.rs:97-105`

4. **Max requests per turn reached**: emits interrupt and yields.
   - `crates/forge_app/src/orch.rs:331-351`
   - `crates/forge_domain/src/chat_response.rs:102-104`

After exiting, `TaskComplete` is only emitted if the turn actually completed (`is_complete`), not for interrupt exits.
- `crates/forge_app/src/orch.rs:375-377`

## How does it recover from errors?

1. **Retry on retryable model-call failures**
   - Chat turn execution is wrapped in retry with exponential backoff.
   - Only domain `Retryable` errors are retried.
   - Retry notifications are streamed as `RetryAttempt`.
   - `crates/forge_app/src/orch.rs:224-253`
   - `crates/forge_app/src/retry.rs:16-38`
   - `crates/forge_domain/src/chat_response.rs:68-74`

2. **Tool-failure feedback to the model**
   - Failed tool results are augmented with retry context (`attempts_left`, max attempts) before appending to context.
   - This gives the model explicit error state for next-pass correction.
   - `crates/forge_app/src/orch.rs:286-302`

3. **Doom-loop detection and steering**
   - A request hook checks repetitive tool-call patterns.
   - On detection, it injects a reminder message into context to break looping behavior.
   - `crates/forge_app/src/app.rs:149`
   - `crates/forge_app/src/hooks/doom_loop.rs:52-58`
   - `crates/forge_app/src/hooks/doom_loop.rs:236-245`

4. **Context compaction under pressure**
   - A response hook compacts context when thresholds are hit.
   - Compaction replaces old ranges with summary frames and removes droppable entries.
   - `crates/forge_app/src/app.rs:150-154`
   - `crates/forge_app/src/hooks/compaction.rs:37-45`
   - `crates/forge_app/src/compact.rs:90-95`
   - `crates/forge_app/src/compact.rs:130-139`

5. **Persist-on-exit safety**
   - After orchestration, conversation is saved regardless of success/failure; stream receives the resulting error if any.
   - `crates/forge_app/src/app.rs:169-183`
