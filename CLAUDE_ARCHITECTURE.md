# Codex Architecture Documentation

## Repository Overview

This is the OpenAI Codex CLI repository - a sophisticated coding agent that runs locally in the terminal. The codebase contains:
- **Rust implementation** in `codex-rs/` - The current, actively developed implementation
- **TypeScript implementation** in `codex-cli/` (legacy, being phased out)

## Core Architecture Principles

### 1. Event-Driven Architecture (SQ/EQ Pattern)

The system uses a **Submission Queue (SQ) / Event Queue (EQ)** pattern for asynchronous communication:

```rust
// User → Codex via Submission Queue
Submission { id, op: UserInput | Interrupt | ExecApproval | ... }

// Codex → User via Event Queue  
Event { id, msg: AgentMessage | ExecApprovalRequest | TaskComplete | ... }
```

**Key Benefits:**
- Non-blocking communication between UI and core logic
- Clear request-response correlation via IDs
- Enables multiple UI implementations (CLI, TUI, MCP server)
- Supports streaming responses and real-time updates

### 2. Layered Architecture

```
┌─────────────────────────────────────┐
│     Application Layer               │
│  (cli, tui, exec, mcp-server)      │
├─────────────────────────────────────┤
│        Core Layer                   │
│  (codex, conversation, protocol)    │
├─────────────────────────────────────┤
│    Infrastructure Layer             │
│ (sandboxing, auth, model clients)   │
└─────────────────────────────────────┘
```

### 3. Security-First Design

**Multi-Layer Sandboxing:**
- **macOS**: Apple Seatbelt (`/usr/bin/sandbox-exec`) 
- **Linux**: Landlock + seccomp syscall filtering
- **Policy-based execution**: `execpolicy` validates all commands
- **Environment isolation**: Controlled environment variables
- **Network isolation**: Optional network disable via sandbox

**Approval Workflows:**
- Command execution approval (`ExecApprovalRequest`)
- Patch application approval (`ApplyPatchApprovalRequest`)
- Configurable policies: `never`, `auto`, `always`

## Key Components

### Core Module (`codex-rs/core/`)

#### `Codex` (codex.rs)
- Main orchestrator using async channels
- Spawns background tasks as actors
- Manages conversation lifecycle
- Coordinates between model clients and execution environment

#### `Protocol` (protocol.rs)
- Defines all message types for SQ/EQ communication
- Key entities: `Session`, `Task`, `Turn`, `Submission`, `Event`
- Supports streaming deltas and final messages

#### `ModelClient` (client.rs, client_common.rs)
- Unified interface for multiple AI providers
- Supports both Chat Completions and Responses API
- Provider configuration via `ModelProviderInfo`
- Built-in retry logic with exponential backoff

#### `CodexConversation` (codex_conversation.rs)
- Manages individual conversation state
- Handles message history and context
- Coordinates tool calls and responses
- Implements prompt caching strategies

### Execution Layer

#### `exec` Module
- **ExecParams**: Configuration for command execution
- **process_exec_tool_call**: Main execution handler
- **StreamOutput**: Real-time output streaming
- Integrates with sandbox policies

#### `execpolicy` Module
- Policy language for command validation
- Pattern matching for allowed/denied commands
- Argument resolution and validation
- Special handling for common tools (sed, awk, etc.)

### MCP (Model Context Protocol) Integration

#### `mcp-server`
- Exposes Codex as an MCP server
- Supports conversation management via tools
- JSON-RPC 2.0 wire format
- Tool definitions: `conversation_create`, `conversation_send_message`

#### `mcp-client`
- Connects to external MCP servers
- Dynamic tool discovery
- Handles tool execution and results

#### `mcp-types`
- Generated TypeScript → Rust type definitions
- Ensures protocol compatibility
- Versioned schema support

### Tool System

**Built-in Tools:**
- `local_shell`: Execute shell commands with sandboxing
- `apply_patch`: Apply code patches with verification
- `web_search`: Search the web (when enabled)
- `update_plan`: Update task planning

**MCP Tools:**
- Dynamically discovered from connected servers
- Seamless integration with built-in tools
- Extensible without code changes

## Design Patterns

### 1. Actor Model
```rust
// Each conversation runs as an independent actor
tokio::spawn(async move {
    conversation.handle_submission(submission).await
});
```

### 2. Provider Abstraction
```rust
pub trait ModelProvider {
    async fn complete(&self, prompt: Prompt) -> Result<Stream<ResponseEvent>>;
}
```

### 3. Error Handling Hierarchy
```rust
pub enum CodexErr {
    Stream(String, Option<Duration>),  // Retryable
    ConversationNotFound(Uuid),        // Not found
    Timeout,                           // Timeout  
    UsageLimitReached(error),         // Business error
}
```

### 4. Shared State Management
```rust
Arc<RwLock<HashMap<Uuid, Arc<CodexConversation>>>>
```
- Thread-safe shared state
- Read-write locks for performance
- Arc for reference counting

## Configuration System

### Config Files
- **Global**: `~/.codex/config.toml`
- **Project**: `.codex/config.toml` (in project root)
- **Instructions**: `AGENTS.md` (project-specific AI instructions)

### Configuration Hierarchy
1. Default values
2. Global config
3. Project config
4. Profile overrides
5. Command-line arguments

### Key Configuration Areas
- **Model Providers**: Custom API endpoints, auth, parameters
- **Sandbox Policies**: Execution restrictions, network access
- **Approval Policies**: Auto-approve patterns, dangerous commands
- **Shell Environment**: Environment variable policies

## Testing Strategy

### Unit Tests
- Embedded in source files via `#[cfg(test)]`
- Focus on individual component behavior
- Mock external dependencies

### Integration Tests
- Separate `tests/` directories per module
- Fixture-based testing for streams
- Mock servers for external APIs

### Test Infrastructure
- `core_test_support`: Common test utilities
- `mcp_test_support`: MCP-specific helpers
- Temporary directory management
- Hermetic test execution

## Performance Considerations

### Async/Await Everywhere
- Tokio runtime for async operations
- Proper timeout handling
- Graceful cancellation support

### Streaming Architecture
- Server-Sent Events (SSE) for responses
- Incremental updates to UI
- Backpressure handling

### Resource Management
- Connection pooling for HTTP clients
- Bounded channels to prevent memory issues
- Automatic cleanup of completed conversations

## Security Boundaries

### Command Execution
1. Policy validation (`execpolicy`)
2. Sandbox preparation
3. Environment filtering
4. Resource limits
5. Output capture and filtering

### File System Access
- Landlock restrictions on Linux
- Seatbelt policies on macOS
- Read-only and read-write path specifications
- No access to sensitive directories by default

### Network Access
- Optional complete network isolation
- Per-tool network policies
- Proxy support for corporate environments

## Extension Points

### Adding New Model Providers
1. Define in `config.toml` under `[model_providers.name]`
2. Specify `base_url`, `env_key`, optional `wire_api`
3. No code changes required

### Adding MCP Tools
1. Connect to MCP server via configuration
2. Tools automatically discovered
3. Integrated into tool selection

### Custom Sandbox Policies
1. Write `.sbpl` file (macOS) or policy rules (Linux)
2. Reference in configuration
3. Applied automatically

## Common Workflows

### Message Flow
```
User Input → Submission Queue → Codex → Model Client → AI Provider
                                   ↓
                             Tool Execution
                                   ↓
User ← Event Queue ← Codex ← Response Stream
```

### Tool Call Flow
1. Model requests tool call
2. Codex validates and prepares execution
3. Optional user approval
4. Sandbox execution
5. Result returned to model
6. Model generates final response

## Debugging and Observability

### Logging
- Structured logging via `tracing` crate
- Environment-based levels: `RUST_LOG=debug`
- Per-module configuration supported
- Logs written to `~/.codex/log/`

### Metrics
- Token usage tracking
- Latency measurements
- Error rate monitoring
- Rollout telemetry (when enabled)

## Best Practices

### When Adding Features
1. Follow existing patterns (SQ/EQ for communication)
2. Add appropriate error variants to `CodexErr`
3. Include unit tests in source file
4. Add integration tests for complex flows
5. Update protocol types if needed

### When Modifying Security
1. Never bypass sandbox without explicit user consent
2. Default to restrictive policies
3. Log all security-relevant operations
4. Consider attack vectors in design

### When Optimizing Performance
1. Profile before optimizing
2. Use async operations for I/O
3. Consider streaming vs. buffering trade-offs
4. Monitor memory usage in long-running sessions

## Anti-Patterns to Avoid

1. **Direct file system access** - Always use sandbox
2. **Synchronous blocking operations** - Use async/await
3. **Unbounded data structures** - Use bounded channels
4. **Skipping approval checks** - Respect user settings
5. **Hardcoded provider logic** - Use abstraction layer

## Architecture Decisions Record (ADR)

### ADR-001: Event-Driven Architecture
**Decision**: Use SQ/EQ pattern instead of direct function calls
**Rationale**: Enables multiple UIs, better testability, clear boundaries
**Consequences**: Slightly more complex, but much more flexible

### ADR-002: Rust Over TypeScript
**Decision**: Migrate from TypeScript to Rust for core implementation
**Rationale**: Better performance, type safety, system programming capabilities
**Consequences**: Steeper learning curve, but more robust system

### ADR-003: MCP Integration
**Decision**: Support Model Context Protocol for extensibility
**Rationale**: Industry standard, enables ecosystem growth
**Consequences**: Additional complexity, but unlimited extensibility

### ADR-004: Platform-Specific Sandboxing
**Decision**: Use native sandboxing per platform vs. containers
**Rationale**: Better performance, tighter integration, smaller footprint
**Consequences**: More platform-specific code, but better UX

## Web Search Implementation Analysis

### Current Implementation Status

Based on analysis of the codex-rs implementation compared to OpenAI's web search documentation:

#### ✅ Implemented Features

1. **Basic web search tool** (`openai_tools.rs`)
   - Registered as `web_search_preview` by default
   - Support for versioned tools via `tool_version` config

2. **User location configuration** (`WebSearchUserLocation`)
   - Country, city, region, timezone fields
   - Properly formatted as "approximate" location type

3. **Search context size** (`WebSearchContextSize`)
   - Low/medium/high settings configurable
   - Mapped to proper string values for API

4. **Force tool choice** (`force_tool_choice`)
   - Flag to prioritize web search in tool selection
   - Sends `{type: "web_search_preview"}` instead of "auto"

5. **URL citations** (`UrlCitation` struct)
   - Start/end indices for text references
   - URL and optional title fields
   - Passed through in `AgentMessageEvent`

6. **Web search events** (`WebSearchEvent`)
   - Status tracking: Started, InProgress, Completed, Failed
   - Query and domains tracking

7. **Search actions** (`WebSearchAction`)
   - Search, OpenPage, FindInPage actions defined

8. **Model-specific gates**
   - Disables web search for nano models (`!model_slug.contains("nano")`)
   - Special handling for o3/o4-mini deep research models
   - Removes user_location and context_size for deep research

#### ❌ Missing/Incomplete Features

1. **Search-specific model support**
   - No handling for `gpt-4o-search-preview` or `gpt-4o-mini-search-preview`
   - These models have special parameter requirements not addressed

2. **Context window limitation**
   - No enforcement of 128,000 token limit when web search is enabled
   - Documentation mentions this but no code enforces it

3. **Citation rendering**
   - Citations passed through but no UI implementation for:
     - Making citations "clearly visible and clickable"
     - Proper formatting of inline citations
   - TUI/CLI don't render citations specially

4. **Cost tracking**
   - No awareness of web search tool call costs
   - No billing/pricing calculations for search tokens

5. **Rate limiting awareness**
   - No specific handling for web search rate limits
   - Should match underlying model tiers

6. **Data residency/ZDR compliance**
   - No configuration for data residency requirements
   - May be needed for enterprise deployments

7. **Search result display requirements**
   - No enforcement of displaying web results with attribution
   - No UI components for rendering search results distinctly

8. **Deep Research features**
   - While OpenPage/FindInPage actions defined, no special handling
   - Missing awareness of deep research model capabilities

### Recommendations for Complete Implementation

1. **Add search-specific model detection**
   ```rust
   // In model_family.rs or similar
   let is_search_model = model_slug.contains("search-preview");
   ```

2. **Implement context window enforcement**
   ```rust
   // When web search is enabled
   let effective_context = min(model_context_window, 128_000);
   ```

3. **Add citation rendering in TUI**
   - Parse annotations in chat widget
   - Make URLs clickable (terminal permitting)
   - Show citation numbers inline

4. **Track web search costs**
   - Add usage fields for search tokens
   - Calculate costs based on model pricing

5. **Add versioned tool fallback**
   ```rust
   // Try dated version first, fallback to generic
   let tool_types = vec![
       format!("web_search_preview_{}", version),
       "web_search_preview".to_string()
   ];
   ```

6. **Implement search result display**
   - Add dedicated event type for search results
   - Format with proper attribution
   - Show domains and timestamps

## Future Considerations

### Potential Improvements
- Complete web search UI implementation with citation rendering
- Search result caching and deduplication
- Conversation persistence (currently in-memory only)
- Distributed execution for scale
- Plugin system for custom tools
- Web UI implementation
- Collaborative features

### Technical Debt
- Some error handling inconsistency
- Manual dependency injection (consider DI framework)
- Limited telemetry abstraction
- Test coverage gaps in some modules

## Glossary

- **SQ**: Submission Queue - User requests to Codex
- **EQ**: Event Queue - Codex responses to user
- **MCP**: Model Context Protocol - Standard for AI tool integration
- **SSE**: Server-Sent Events - Streaming protocol
- **Landlock**: Linux security module for sandboxing
- **Seatbelt**: macOS sandboxing technology
- **Turn**: Single request-response cycle in conversation
- **Delta**: Incremental update in streaming response