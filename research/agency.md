## Does this agent make decisions?

*Yes — but at two different layers*:

Based on what we already mapped, the agent does make decisions, but they’re split between:

1. Model-driven decisions (LLM behavior)
      ◦ The model decides what to output each turn: plain answer vs tool calls, tool names/args, etc., and that output directly drives next actions in the loop
        (crates/forge_app/src/orch.rs:264-279).

2. Harness-driven decisions (rule-based controller)
      ◦ The orchestrator deterministically decides whether to continue, yield, complete, interrupt, or retry based on response/tool/error state
        (crates/forge_app/src/orch.rs:209-351).
      ◦ Retry decisions are policy-based (Retryable only), not free-form reasoning (crates/forge_app/src/retry.rs:22-38).
      ◦ It also applies guardrail decisions like doom-loop reminder injection and compaction triggers (crates/forge_app/src/hooks/doom_loop.rs:228-245,
        crates/forge_app/src/hooks/compaction.rs:37-45).

So: yes, decision-making exists — the model makes task-level action choices, and the harness makes execution-control/safety decisions.

## Difference between agent and harness

### Agent
The agent is the decision-making policy/persona (mostly the model + prompt + tool affordances):
• It decides what to do next in-task (answer directly, call a tool, which tool, with what arguments).
• In this codepath, that intent arrives as model output (content + tool calls), then is executed by the runtime loop
  (crates/forge_app/src/orch.rs:264-279).

### Harness
The harness is the execution/runtime controller around the agent:
• It builds context, sends requests, executes tools, appends results, and loops (crates/forge_app/src/orch.rs:209-312).
• It enforces stop/yield/interrupt policies (complete, follow-up yield, max failures, max requests) (crates/forge_app/src/orch.rs:264-275,
  crates/forge_app/src/orch.rs:313-351).
• It handles operational resilience (retry policy, hooks like doom-loop detection and compaction) (crates/forge_app/src/retry.rs:16-38,
  crates/forge_app/src/hooks/doom_loop.rs:228-245, crates/forge_app/src/hooks/compaction.rs:37-45).

## One-line mental model
• Agent = “brain/strategy.”
• Harness = “orchestrator/safety rails/runtime.”

The harness (orchestrator/runtime) is in charge of the state machine.

It controls loop transitions like:
• request → response (crates/forge_app/src/orch.rs:224-253)
• response → tools / complete / yield (crates/forge_app/src/orch.rs:264-275)
• tools → append → next request (crates/forge_app/src/orch.rs:277-312)
• interrupt on limits (crates/forge_app/src/orch.rs:313-351)

The agent/model proposes actions; the harness decides how the execution state advances.
