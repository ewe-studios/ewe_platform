# Decision 15: Tool Registration and Discovery via ToolShed

**Status:** Accepted  
**Date:** 2026-06-12  
**Updated:** 2026-06-13  
**Context:** Specification 36 — Agentic API for foundation_ai

## Problem

The agent needs tools to interact with the world. Tools must be:
- Described to the LLM in its function calling format
- Discoverable — the agent can ask "is there a tool for X?" without knowing all tools upfront
- Registerable at session creation and dynamically at runtime
- Stored with semantic descriptions for vector-based discovery

## Decision

Tool registration follows a clear pipeline from implementation to LLM prompt, with the `ToolImpl` trait defining what implementers need to provide.

### Tool Pipeline

```
ToolImpl (implementation)
    ↓ registered with
ToolCallManager (registry)
    ↓ generates
ToolShed (always present, even if zero tools — shed meta-tool is always available)
    ↓ converted by ToolFormatter
ModelInteraction.tools_shed (sent to LLM)
    ↓ LLM responds with
ToolRequest (intent: tool name + arguments + depends_on + execution_hint)
    ↓ executed by
ToolCallManager (looks up ToolImpl by name, executes)
    ↓ returns
ToolResult (content or error, sent back to LLM via Message API)
```

### ToolImpl Trait

All tool implementations implement this trait:

```rust
pub trait ToolImpl: Send + Sync {
    /// Tool definition for LLM function calling format
    fn definition(&self) -> ToolDefinition;
    
    /// Execute the tool with parsed arguments
    fn execute(&self, arguments: HashMap<String, ArgType>) 
        -> impl Future<Output = Result<ToolCallResult, ToolError>> + Send;
}

pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub arguments: Args,  // JSON Schema from foundation_jsonschema
}

pub struct ToolCallResult {
    pub content: UserModelContent,
    pub error_detail: Option<String>,
}
```

### ToolCallManager Registry

The ToolCallManager maintains a registry of tool implementations and builds the ToolShed from them:

```rust
pub struct ToolCallManager {
    tools: HashMap<String, Arc<dyn ToolImpl>>,
    vector_store: Arc<dyn VectorStore>,  // for shed tool search
    message_store: Arc<MessageInner>,    // for persisting tool call results
    // ... other fields
}

impl ToolCallManager {
    /// Register a tool implementation
    pub fn register(&mut self, tool: Arc<dyn ToolImpl>) {
        let name = tool.definition().name.clone();
        self.tools.insert(name, tool);
        // Also store description in vector store for shed search
        self.vector_store.insert(&name, tool.definition().embedding(), metadata);
    }
    
    /// Execute a tool call by name (look up + execute)
    fn execute_tool(&self, call: &ToolCallRequest) -> Result<ToolCallResult, ToolError> {
        let tool = self.tools.get(&call.name)
            .ok_or_else(|| ToolError::UnknownTool(call.name.clone()))?;
        tool.execute(call.arguments.clone())
    }
    
    /// Build the ToolShed from registered tools
    /// Always includes the shed meta-tool
    fn build_toolshed(&self) -> ToolShed {
        ToolShed {
            shed: self.build_shed_tool(),        // always present
            memory: self.build_memory_tool(),    // if memory feature enabled
            delegate: self.build_delegate_tool(), // if delegation feature enabled
            read: self.tools.get("read").map(|t| t.definition().into()),
            edit: self.tools.get("edit").map(|t| t.definition().into()),
            write: self.tools.get("write").map(|t| t.definition().into()),
            search: self.tools.get("search").map(|t| t.definition().into()),
            bash: self.tools.get("bash").map(|t| t.definition().into()),
            // No `others` field — shed tool covers dynamic discovery
        }
    }
}
```

### ToolShed — Always Present

The ToolShed is **always** included in `ModelInteraction.tools_shed`, even when zero tools are registered. This ensures the `shed` meta-tool is always available for tool discovery.

### The `shed` Meta-Tool

The `shed` tool is always present in the ToolShed. It searches the tool description vector store and returns matching tool summaries:

```rust
pub struct ShedQuery {
    pub description: String,  // "I need a tool to parse YAML"
    pub limit: usize,
}

pub struct ShedResult {
    pub tools: Vec<ToolSummary>,
}

pub struct ToolSummary {
    pub name: String,
    pub description: String,
    pub category: String,
}
```

The `shed` tool:
1. Takes a natural language query from the LLM
2. Searches the tool description vector store for matches
3. Returns tool names, descriptions, and schemas

### ToolFormatter Integration

Tool schema conversion is handled by `ToolFormatter` in foundation_ai:

```rust
pub trait ToolFormatter: Default + Send + Sync {
    fn format_tools(&self, tools: &[Tool]) -> Result<serde_json::Value, ...>;
    fn tool_calling_instructions(&self) -> Option<String>;
    fn extract_tool_calls(&self, response: &str) -> Result<ExtractResult, ...>;
    fn format_tool_response(&self, result: &Messages) -> Result<serde_json::Value, ...>;
}
```

Each provider (Anthropic, OpenAI, etc.) implements `ToolFormatter` to convert internal `Tool` definitions to its API's format.

### External Tools (MCP, HTTP, CLI)

External tools implement `ToolImpl` and register with the ToolCallManager:

- **MCP servers**: Wrap MCP tool definitions as `ToolImpl`
- **HTTP endpoints**: Describe HTTP call semantics as a `ToolImpl`
- **CLI commands**: Wrap shell commands as `ToolImpl`

The agent doesn't need to know the difference — all tools implement `ToolImpl`.

### Tool Versioning

Tools present their interface via their JSON Schema. If a tool's interface changes, the new schema is registered. Versioning is the tool's own concern.

> **RESOLVED (2026-06-15, F01 + F09 + F10):** `ToolShed.others` is **removed**. Zero-tool sessions → **`Option<ToolShed>` at the `ModelInteraction` level**. `ToolImpl::execute` is **sync** (async isn't object-safe; valtron owns concurrency). `ToolShed` gains **`search_files`** (fff) and generalizes `bash` → **`shell`** (bash on linux/macOS, PowerShell on Windows).

## Rationale

**Why ToolImpl trait?**
- Clear contract: `definition()` + `execute() -> Result<ToolCallResult, ToolError>`
- Easy to implement for new tools
- ToolCallManager can execute any tool via the trait

**Why ToolShed always present?**
- `shed` meta-tool is always available for discovery
- Even zero-tool sessions can discover tools dynamically
- Consistent interface for LLM — ToolShed is always there

**Why no `others` field in ToolShed?**
- `shed` tool covers dynamic discovery — no need for a catch-all
- External tools discovered via `shed` search, not hardcoded in struct

## Alternatives Considered

### Flat tool list in ModelInteraction
- **Pros:** Simpler
- **Cons:** All tool definitions in context — wastes tokens, limits tool count
- **Rejected because:** Agent may have hundreds of tools — can't fit all in context

### Separate tool registry outside ToolShed
- **Pros:** Clearer separation
- **Cons:** More complexity, ToolShed already provides the structure
- **Rejected because:** ToolShed + `shed` tool is sufficient
