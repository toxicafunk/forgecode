# Retrieval and indexing

## How does the harness find relevant code across a large codebase? Does it use embeddings, AST parsing, dependency graphs, or just grep?

Yes. In this harness, the model can trigger operations that read files, write/edit files, run shell commands, and fetch remote HTTP/HTTPS content. It can also invoke delegated agent tools and MCP-backed integrations.


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
➜  forgecode git:(main) ✗ : based on current knowledge (no more read), How does the harness find relevant code across a large codebase? Does it use embeddings, AST parsing, dependency graphs, or just grep?
● [09:42:38] Continue 327b8afc-efcf-4d4f-9f83-5dfd5f51f639

## It’s a hybrid search setup, not just grep

From what we already mapped, the harness exposes two primary code-discovery paths:

1. Regex/file search (grep-style)
      ◦ fs_search is explicitly ripgrep-oriented (rg PATH, --glob, --type, -A/-B/-C, etc.) in the tool schema/docs:
     crates/forge_domain/src/tools/catalog.rs:214-275
      • Execution route goes through the search service in tool execution:
     crates/forge_app/src/tool_executor.rs:178-186

1. Semantic search (embeddings + reranking)
      ◦ The semantic-search docs explicitly describe:
          ▪ embedding query for vector similarity
          ▪ separate use_case reranking intent
     crates/forge_domain/src/tools/catalog.rs:293-337
      • Runtime builds SearchParams, runs multiple semantic queries in parallel, then deduplicates merged results:
     crates/forge_app/src/tool_executor.rs:187-213
      • Results are returned as codebase search results objects:
     crates/forge_app/src/tool_executor.rs:214-227


## Is it AST/dependency-graph based?

Based on the exposed tool catalog + execution paths we reviewed, I only see:
• regex/ripgrep-style search
• semantic embedding/reranking search

I do not see an AST parser or dependency-graph retrieval mechanism wired into this tool-execution path. The available search primitives are represented in the catalog and executor as above:
crates/forge_domain/src/tools/catalog.rs:41-58
crates/forge_app/src/tool_executor.rs:178-227


## Large-codebase behavior

Semantic search availability is gated by workspace indexing/auth state, which indicates an indexed retrieval backend rather than raw file scan only:
crates/forge_app/src/tool_registry.rs:215-231
crates/forge_app/src/tool_registry.rs:247-253
