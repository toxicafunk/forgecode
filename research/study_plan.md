# ForgeCode Codebase Study Plan

## Overview

This document provides a structured learning path for understanding the ForgeCode codebase. The codebase follows Clean Architecture principles with clear separation between domain logic, application services, and infrastructure.

## Entry Point: `forge_domain` Crate

**Location**: `crates/forge_domain/`

The **`forge_domain`** crate is the **primary entry point** for understanding this codebase. Start here to learn the core vocabulary and concepts.

### Why Start with `forge_domain`?

1. **Zero External Dependencies**: Minimal dependencies, focuses purely on domain concepts
2. **Core Vocabulary**: Defines all key types used throughout the codebase
3. **Clean Architecture**: Represents the innermost layer - pure business logic without infrastructure concerns

### Key Domain Concepts

The domain defines the "language" of the system:

- **Agent System**: `Agent`, `AgentId`, `AgentDefinition` - AI agent concepts and configurations
- **Communication**: `ChatRequest`, `ChatResponse` - Request/response protocols
- **Conversation**: `Conversation`, `Message` - Chat data structures and history
- **Provider Abstractions**: `Provider`, `Model` - LLM provider interfaces
- **Tool System**: `Tool`, `ToolDefinition` - Tool definitions and metadata
- **Orchestration**: `Workflow`, `Hook`, `Event` - Workflow and event primitives
- **Configuration**: `Environment`, `AppConfig` - Settings and environment

---

## Learning Path

### Phase 1: Domain Understanding (Start Here)

**Crate**: `forge_domain`  
**Location**: `crates/forge_domain/`

**Key Files to Read (in order)**:

1. **`src/agent.rs`** - Understand agent concepts and capabilities
   - What agents are and how they're configured
   - Agent roles and permissions

2. **`src/conversation.rs`** - Chat data structures
   - How conversations are structured
   - Message history management

3. **`src/message.rs`** - Message types and patterns
   - Different message roles (user, assistant, system)
   - Message content types

4. **`src/tools.rs`** - Tool system foundations
   - Tool definitions and metadata
   - Tool parameters and validation

5. **`src/provider.rs`** - LLM provider abstractions
   - Provider interface design
   - Model capabilities and configuration

6. **`src/workflow.rs`** - Workflow orchestration
   - Workflow definitions
   - Event-driven architecture

7. **`src/hook.rs`** - Hook system
   - Lifecycle hooks (on_start, on_request, on_response)
   - Hook composition patterns

**What You'll Learn**:
- The core vocabulary of the system
- How different components relate to each other
- The data structures that flow through the application

---

### Phase 2: Application Logic

**Crate**: `forge_app`  
**Location**: `crates/forge_app/`

After understanding the domain, study how business logic is implemented.

**Key Files to Read (in order)**:

1. **`src/app.rs`** (lines 1-150) - Core `ForgeApp` orchestration
   - Main application entry point for chat functionality
   - How conversations are initialized
   - System and user prompt generation

2. **`src/orch.rs`** - The `Orchestrator` implementation
   - Core conversation loop
   - Streaming response handling
   - State management

3. **`src/agent_executor.rs`** - Agent execution logic
   - How agents process requests
   - Agent-specific behavior

4. **`src/tool_executor.rs`** - Tool execution
   - How tools are invoked
   - Error handling and retries
   - Timeout management

5. **`src/tool_registry.rs`** - Tool registration system
   - How tools are discovered and registered
   - Tool metadata management

6. **`src/tool_resolver.rs`** - Tool resolution
   - How tools are matched to agents
   - Permission and capability checking

7. **`src/services/`** - Service layer implementations
   - `AgentRegistry` - Agent management
   - `ConversationService` - Conversation persistence
   - `ProviderService` - Provider management
   - `WorkflowService` - Workflow handling

**What You'll Learn**:
- How conversations flow through the system
- How tools are resolved and executed
- How agents interact with LLM providers
- Service patterns and dependency management

---

### Phase 3: Entry Point & CLI

**Crate**: `forge_main`  
**Location**: `crates/forge_main/`

Understand how the application starts and how users interact with it.

**Key Files to Read (in order)**:

1. **`src/main.rs`** (lines 1-100) - Application entry point
   - Initialization sequence
   - Panic hook setup
   - CLI argument processing

2. **`src/cli.rs`** - Command-line interface
   - Available commands and options
   - Argument parsing

3. **`src/ui.rs`** - User interface logic
   - Interactive mode
   - Input handling
   - Output rendering

**What You'll Learn**:
- How the application bootstraps
- CLI design and user interaction patterns
- Integration between UI and core application

---

### Phase 4: Infrastructure & Integration

**Crate**: `forge_infra`  
**Location**: `crates/forge_infra/`

Explore concrete implementations of infrastructure concerns.

**Key Areas to Study**:

1. **Provider Implementations**
   - OpenAI integration
   - Anthropic integration
   - Other LLM providers
   - Authentication and token management

2. **File System Operations**
   - File reading and writing
   - Directory traversal
   - File watching and change detection

3. **API Clients**
   - HTTP client configuration
   - Retry logic
   - Error handling

4. **Storage Implementations**
   - Conversation persistence
   - Configuration storage
   - Cache management

**What You'll Learn**:
- How abstract domain concepts become concrete implementations
- Integration patterns with external services
- Error handling and resilience strategies

---

## Architecture Overview

The codebase follows **Clean Architecture** principles:

```
┌─────────────────────────────────────┐
│         forge_main (Entry)          │
│      CLI, UI, Initialization        │
└────────────┬────────────────────────┘
             │
┌────────────▼────────────────────────┐
│          forge_app (Core)           │
│   Orchestration, Agent Execution,   │
│     Tool Execution, Services        │
└────────────┬────────────────────────┘
             │
┌────────────▼────────────────────────┐
│      forge_domain (Domain)          │
│  Pure Domain Types & Business Rules │
└────────────┬────────────────────────┘
             │
┌────────────▼────────────────────────┐
│     forge_infra (Infrastructure)    │
│  Providers, APIs, File System, etc. │
└─────────────────────────────────────┘
```

### Key Design Patterns

From `AGENTS.md`:

1. **Service Pattern**: Services have single infrastructure generic parameter using `Arc<T>`
2. **Clean Dependency Flow**: No service-to-service dependencies
3. **Constructor Pattern**: `new()` without bounds, trait bounds only on methods
4. **Composition**: Use `+` to combine trait bounds

**Example Service Pattern**:
```rust
pub struct FileService<F>(Arc<F>);

impl<F> FileService<F> {
    pub fn new(infra: Arc<F>) -> Self { ... }
}

impl<F: FileReader + Environment> FileService<F> {
    pub async fn read_with_validation(&self, path: &Path) -> Result<String> { ... }
}
```

---

## Application Flow

### Initialization Flow
(From `crates/forge_main/src/main.rs:11-76`)

1. **Parse CLI arguments** - Process user input and options
2. **Set up panic hooks** - Configure error display
3. **Initialize ForgeAPI** - Set up configuration and restricted mode
4. **Launch UI** - Start interactive interface

### Chat Flow
(From `crates/forge_app/src/app.rs:48-150`)

1. **Load conversation history** - Retrieve existing conversation
2. **Discover files and register templates** - Scan workspace
3. **Resolve agent and provider** - Determine which agent and LLM to use
4. **Get tool definitions** - Gather available tools
5. **Resolve tools for agent** - Filter tools based on agent permissions
6. **Generate system prompt** - Create system instructions with context
7. **Generate user prompt** - Format user input with attachments
8. **Initialize conversation metrics** - Track performance
9. **Create orchestrator with hooks** - Set up conversation loop
10. **Stream responses** - Stream LLM responses back to user

---

## Key Abstractions

### Agents
- Represent different AI personas (e.g., "Forge" for coding, "Sage" for research)
- Each has specific tools, models, and system prompts
- Configured via workflow files and agent definitions

### Tools
- Extensible tool system (file operations, search, shell commands, etc.)
- Tools are resolved per agent based on permissions
- Executed via `ToolExecutor` with timeout and error handling
- See `forge_app/src/tool_registry.rs` and `forge_app/src/tool_executor.rs`

### Orchestrator
- Core conversation loop in `forge_app/src/orch.rs`
- Handles streaming, retries, hooks, and state management
- Coordinates between agent, tools, and providers

### Hooks
- Lifecycle hooks: `on_start`, `on_request`, `on_response`, `on_complete`
- Composable using `.and()` for chaining multiple hooks
- Used for logging, title generation, doom loop detection, compaction

---

## Design Philosophy

From `AGENTS.md`:

### Error Management
- Use `anyhow::Result` for services and repositories
- Use `thiserror` for domain errors
- Never implement `From` for domain errors (explicit conversions only)

### Testing
- Three-step test pattern: fixture → actual → expected
- Use `pretty_assertions` for better error messages
- Tests in same file as source code
- Use `derive_setters` for building test fixtures

### Documentation
- Write Rust docs (`///`) for all public items
- Focus on functionality descriptions (for LLMs)
- No code examples needed

### Verification
- Run `cargo insta test --accept` for snapshot tests
- Use `cargo check` for fast verification
- **Never** run `cargo build --release` unless distributing binaries

---

## Deep Dive Topics

### 1. Tool System
**Question**: How are tools registered, resolved, and executed?

**Study Path**:
1. `forge_domain/src/tools.rs` - Tool definitions
2. `forge_app/src/tool_registry.rs` - Registration
3. `forge_app/src/tool_resolver.rs` - Resolution logic
4. `forge_app/src/tool_executor.rs` - Execution with timeouts

### 2. Provider Integration
**Question**: How are different LLM providers supported?

**Study Path**:
1. `forge_domain/src/provider.rs` - Provider abstractions
2. `forge_infra/` - Concrete implementations (OpenAI, Anthropic, etc.)
3. `forge_app/src/agent_provider_resolver.rs` - Provider selection

### 3. Workflow System
**Question**: How do workflows and events work?

**Study Path**:
1. `forge_domain/src/workflow.rs` - Workflow definitions
2. `forge_domain/src/event.rs` - Event types
3. `forge_app/src/services/workflow_service.rs` - Workflow execution

### 4. Streaming Architecture
**Question**: How do responses stream from LLMs to UI?

**Study Path**:
1. `forge_stream/` - Stream utilities
2. `forge_app/src/orch.rs` - Orchestrator streaming
3. `forge_main/src/ui.rs` - UI rendering

### 5. Agent Execution
**Question**: How does the agent executor handle retries and error recovery?

**Study Path**:
1. `forge_app/src/agent_executor.rs` - Core execution logic
2. `forge_app/src/retry.rs` - Retry strategies
3. `forge_domain/src/retry_config.rs` - Configuration

### 6. Hook System
**Question**: What hooks are available and how do they intercept flow?

**Study Path**:
1. `forge_domain/src/hook.rs` - Hook definitions
2. `forge_app/src/hooks/` - Hook implementations:
   - `tracing_handler.rs` - Logging
   - `title_generation_handler.rs` - Title generation
   - `doom_loop_detector.rs` - Infinite loop prevention
   - `compaction_handler.rs` - Context compression

### 7. MCP Integration
**Question**: How does Model Context Protocol integration work?

**Study Path**:
1. `forge_domain/src/mcp.rs` - MCP types
2. `forge_domain/src/mcp_servers.rs` - Server configuration
3. `forge_app/src/mcp_executor.rs` - MCP tool execution

### 8. Conversation Persistence
**Question**: How are conversations persisted and retrieved?

**Study Path**:
1. `forge_domain/src/conversation.rs` - Conversation structure
2. `forge_app/src/services/conversation_service.rs` - Persistence logic
3. Storage implementation in `forge_infra/`

---

## Additional Resources

### Documentation
- `README.md` - Project overview and quickstart
- `AGENTS.md` - Agent guidelines and best practices
- `docs/` - Additional documentation

### Configuration
- `forge.yaml` - Main configuration file
- `forge.schema.json` - Configuration schema
- `.mcp.json` - MCP server configuration

### Testing
- Use `cargo insta test --accept` for snapshot testing
- Use `cargo test` for unit tests
- Use `cargo check` for fast compilation checks

---

## Next Steps

### Immediate Actions
1. ✅ Read `forge_domain/src/lib.rs` - Get familiar with all available domain types
2. ✅ Explore `forge_domain/src/agent.rs` - Understand agent concepts
3. ✅ Study `forge_domain/src/conversation.rs` - Learn conversation structure
4. ✅ Review `forge_app/src/app.rs` - Understand application flow

### Questions to Explore
1. How does the agent executor handle retries and error recovery?
2. What hooks are available and how do they intercept conversation flow?
3. How does the MCP (Model Context Protocol) integration work?
4. How are conversations persisted and retrieved?
5. How does the streaming architecture work end-to-end?
6. What are the testing patterns used across the codebase?

### Hands-On Learning
1. Try modifying an existing tool in `forge_app/src/tool_registry.rs`
2. Create a simple custom hook in `forge_app/src/hooks/`
3. Add a new domain type in `forge_domain/src/`
4. Write tests following the patterns in `AGENTS.md`

---

## Summary

**Start with `forge_domain`** to learn the vocabulary, then move to **`forge_app`** to understand the orchestration, explore **`forge_main`** for the entry point, and finally dive into **`forge_infra`** for concrete implementations.

The architecture emphasizes:
- **Clean separation** between domain, application, and infrastructure
- **Service patterns** with single generic parameters and `Arc<T>`
- **Testability** through pure domain types
- **Flexibility** to swap implementations without changing business logic
- **Agent-oriented design** with tools, workflows, and hooks

Happy learning! 🚀
