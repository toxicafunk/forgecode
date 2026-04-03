# Context Management Analysis (Agent/Harness)

## Scope
This summary describes what is actually sent to model APIs at call time, based on the request-construction and provider layers.

## 1) What gets stuffed into context before each model call

At provider call time, the harness serializes an in-memory `Context` object into provider-specific request payloads. The provider layers themselves do not read repository files on each call; they transform `Context` into API messages.

- OpenAI Chat Completions path: `Request::from(context)` then provider pipeline transforms the request before sending (`crates/forge_repo/src/provider/openai.rs:124-132`).
- Anthropic path: `Request::try_from(context)` after a reasoning transform, then request transforms are applied (`crates/forge_repo/src/provider/anthropic.rs:101-132`).
- OpenAI Responses path: `CreateResponse::from_domain(context)` and then stream request is sent (`crates/forge_repo/src/provider/openai_responses/repository.rs:127-153`).

## 2) Conversation history that is included

For the Responses API path, conversion is explicit and shows exactly what conversation material is serialized:

- Every `context.messages` entry is iterated and converted (`crates/forge_repo/src/provider/openai_responses/request.rs:177-274`).
- System messages:
  - First system message is mapped to top-level `instructions` (`crates/forge_repo/src/provider/openai_responses/request.rs:180-183,306-308`).
  - Additional system messages are mapped as `developer` messages in the input stream (`crates/forge_repo/src/provider/openai_responses/request.rs:184-188`).
- User messages are mapped to user inputs (`crates/forge_repo/src/provider/openai_responses/request.rs:191-197`).
- Assistant messages are included when non-empty; assistant tool calls are also serialized (`crates/forge_repo/src/provider/openai_responses/request.rs:198-233`).
- Tool results are included as `function_call_output` items (`crates/forge_repo/src/provider/openai_responses/request.rs:235-257`).
- Image messages are included as input image content (`crates/forge_repo/src/provider/openai_responses/request.rs:258-272`).

## 3) Tools, reasoning, and metadata included

- Tool catalog and schema are included from `context.tools` (`crates/forge_repo/src/provider/openai_responses/request.rs:281-297`).
- Tool choice is included from `context.tool_choice` (`crates/forge_repo/src/provider/openai_responses/request.rs:298-301,326-328`).
- Reasoning config is included if present (`crates/forge_repo/src/provider/openai_responses/request.rs:330-334`).
- `conversation_id` is used as `prompt_cache_key` (`crates/forge_repo/src/provider/openai_responses/request.rs:172,336-338`).

## 4) Caching behavior (what gets cache markers)

Caching is provider/request-transform behavior, not additional file loading:

- OpenAI/OpenRouter transform pipeline applies cache transform for specific models/providers (`crates/forge_app/src/dto/openai/transformers/pipeline.rs:34-47`).
- OpenAI cache transform marks first and last messages cached; removes cache on second-to-last when applicable (`crates/forge_app/src/dto/openai/transformers/set_cache.rs:14-47`).
- Anthropic cache transform caches first system (or first conversation message if no system) and last message (`crates/forge_app/src/dto/anthropic/transforms/set_cache.rs:14-44`).

## 5) Direct answer to “which files/docs/history are in the model window?”

At the call boundary, the model window contains the serialized `Context` contents (messages + tools + options), not direct on-demand file reads from the repo.

So the effective inclusion is:

1. Conversation messages currently present in `Context` (system/user/assistant/tool/image).
2. Tool definitions and tool choice.
3. Optional reasoning settings.
4. Optional metadata like `conversation_id` mapped to cache key (Responses API path).

If repo files/documentation (e.g., system instructions, custom instruction docs, summary frames) appear in the model window, they do so only because earlier orchestration already injected them into `Context` as message content before this provider serialization step.
