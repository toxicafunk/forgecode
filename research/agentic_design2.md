## Agentic loop design (from current implementation)

Yes — this harness is a true multi-pass loop.

### 1) Does model output get fed back in?
Yes.
Each iteration gets an LLM response, executes tool calls, then appends the assistant message + tool calls + tool results back into Context, and runs another request if not yielded:

• Main loop: while !should_yield crates/forge_app/src/orch.rs:209-210
• Call model each iteration: crates/forge_app/src/orch.rs:224-253
• Execute tool calls: crates/forge_app/src/orch.rs:277-279
• Append assistant output + tool records back to context: crates/forge_app/src/orch.rs:304-312
• append_message actually stores assistant message and adds tool results as messages: crates/forge_domain/src/context.rs:549-593

So the next pass uses an updated conversation state, not a fresh prompt.

────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────

### 2) When does it stop?
The loop stops when should_yield becomes true. That happens under these conditions:

1. Task complete signal from model
   Finish reason is stop and no tool calls:
      • crates/forge_app/src/orch.rs:264-268

1. Model asks for follow-up/yield tool
   Any tool call that maps to “followup” triggers yield:
      • crates/forge_app/src/orch.rs:270-275
      • Followup detection: crates/forge_domain/src/tools/catalog.rs:844-850

1. Too many tool failures in one turn
      ◦ Tracker limit check interrupts and yields: crates/forge_app/src/orch.rs:313-323
      ◦ Interruption type: MaxToolFailurePerTurnLimitReached crates/forge_domain/src/chat_response.rs:97-105
      ◦ Error tracking mechanics: crates/forge_domain/src/tools/call/tool_call.rs:244-319

2. Max requests-per-turn reached
      ◦ Hard cap check triggers interrupt + yield: crates/forge_app/src/orch.rs:331-351
      ◦ Interruption type: MaxRequestPerTurnLimitReached crates/forge_domain/src/chat_response.rs:102-104

After loop exits, it emits TaskComplete only if it ended via true completion (is_complete), not just interruption:
• crates/forge_app/src/orch.rs:375-377

────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────

### 3) How does it recover from errors?
There are multiple layers of recovery/safety:

1. Retry transient LLM call failures
      ◦ Wrapped with exponential retry: crates/forge_app/src/orch.rs:224-253
      ◦ Retry policy only retries Error::Retryable: crates/forge_app/src/retry.rs:22-38
      ◦ Retry events emitted to client: crates/forge_app/src/orch.rs:247-249

2. Tool-error feedback loop to the model
      ◦ After tool execution, failed tool outputs get augmented with retry context (attempts_left, limits) via template text before being appended back:
      ◦ crates/forge_app/src/orch.rs:286-302
      ◦ This is how the model gets structured feedback and can self-correct next pass.

3. Doom-loop mitigation
      ◦ Hook runs on each request and detects repetitive tool-call patterns from conversation history:
      ◦ crates/forge_app/src/app.rs:149
      ◦ crates/forge_app/src/hooks/doom_loop.rs:52-58
      ◦ On detection, it injects a reminder message into context to steer behavior:
      ◦ crates/forge_app/src/hooks/doom_loop.rs:236-245

4. Context-size recovery via compaction
      ◦ Response hook can compact context when thresholds are exceeded:
      ◦ Hook registration: crates/forge_app/src/app.rs:150-154
      ◦ Handler behavior: crates/forge_app/src/hooks/compaction.rs:37-45
      ◦ Compaction writes a summary frame and replaces old ranges: crates/forge_app/src/compact.rs:90-95,130-139

5. Persistence + surfaced failure
      ◦ Even if orchestration fails, conversation is still saved, and error is emitted to stream:
      ◦ crates/forge_app/src/app.rs:169-183
