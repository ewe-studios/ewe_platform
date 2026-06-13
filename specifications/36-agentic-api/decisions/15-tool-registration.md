# Decision 15: Tool Registration and Discovery via ToolShed

**Status:** Proposed  
**Date:** 2026-06-12  
**Context:** Specification 36 — Agentic API for foundation_ai

## Problem

The agent needs tools to interact with the world. Tools must be:
- Described to the LLM in its function calling format
- Discoverable — the agent can ask "is there a tool for X?" without knowing all tools upfront
- Registerable at session creation and dynamically at runtime
- Stored with semantic descriptions for vector-based discovery

## Decision

Tool registration and discovery is handled entirely by the **ToolShed**, which is already defined in `foundation_ai::types` and wired into `ModelInteraction`.

### ToolShed Structure

```rust
pub struct ToolShed {
    pub shed: Tool,          // meta-tool: "find me a tool for X"
    pub memory: Option<MemoryTool>,  // add, replace, remove memory facts
    pub delegate: Option<DelegationTool>,  // start, check, get delegation
    pub read: Tool,          // read file content
    pub edit: Tool,          // edit file content
    pub write: Tool,         // write file content
    pub search: Tool,        // search file content (fff integration)
    pub bash: Option<Tool>,  // execute shell commands
    // No `others` field — shed tool covers dynamic discovery
}
```

### The `shed` Meta-Tool

The `shed` tool is the key to dynamic tool discovery. It allows the agent to ask:

> "Is there a tool that can parse YAML files?"
> "Do you have a tool for interacting with AWS S3?"

The `shed` tool:
1. Takes a natural language query
2. Searches the **tool description vector store** for matching tools
3. Returns matching tool names, descriptions, and schemas

```rust
// shed tool arguments
pub struct ShedQuery {
    pub description: String,  // "I need a tool to parse YAML"
    pub limit: usize,         // max results to return
}

// shed tool response
pub struct ShedResult {
    pub tools: Vec<ToolSummary>,
}

pub struct ToolSummary {
    pub name: String,
    pub description: String,
    pub category: String,
    // Tool schema available on request via tool name
}
```

### Tool Description Vector Store

All tool descriptions are stored in the vector store under a **toolshed namespace**:

```
VectorStore (toolshed namespace)
├── "read" → "Read file content from the filesystem"
├── "edit" → "Edit file content with search-and-replace"
├── "write" → "Write content to a file, creating if needed"
├── "search" → "Search file content using fff fast grep"
├── "bash" → "Execute shell commands in a sandboxed environment"
├── "aws_s3_upload" → "Upload files to AWS S3 bucket"
├── "yaml_parser" → "Parse YAML files into structured data"
└── ... (any number of tools)
```

When the agent calls `shed("I need to parse YAML")`, the vector store returns `yaml_parser` as a match.

### Tool Registration Flow

```
Session creation
├── ToolShed provided (core tools: read, edit, write, search, bash, shed)
│   └── Tool descriptions stored in vector store (toolshed namespace)
│
├── ModelInteraction created with ToolShed
│   └── Tool definitions converted to provider format via ToolFormatter
│       ├── Anthropic: { name, description, input_schema }
│       ├── OpenAI: { type: "function", function: { name, description, parameters } }
│       └── etc.
│
├── Agent starts with core tools available
│
└── When agent needs unknown tool:
    ├── Agent calls `shed("I need to do X")`
    ├── Vector store searches for matching tool descriptions
    ├── If found: agent gets tool name + description, requests full schema
    └── If not found: agent reports no tool available
```

### Tool Execution

The ToolCallManager receives tool call requests from the LLM and executes them:

```rust
impl ToolCallManager {
    pub fn execute(&self, call: ToolCallRequest) -> ToolCallResult {
        match call.name.as_str() {
            "read" => self.tools.read.execute(call.arguments),
            "edit" => self.tools.edit.execute(call.arguments),
            "write" => self.tools.write.execute(call.arguments),
            "search" => self.tools.search.execute(call.arguments),  // fff integration
            "bash" => self.tools.bash.as_ref()?.execute(call.arguments),
            "shed" => self.shed_search(call.arguments),              // vector store search
            _ => Err(ToolError::UnknownTool(call.name)),
        }
    }
    
    /// shed_search: query vector store for tool descriptions
    fn shed_search(&self, args: ShedQuery) -> Result<ShedResult> {
        let matches = self.vector_store.query(&args.description, args.limit);
        Ok(ShedResult {
            tools: matches.into_iter().map(|m| m.metadata.tool_summary()).collect(),
        })
    }
}
```

### ToolFormatter Integration

Tool schema conversion is already handled by `ToolFormatter` in foundation_ai:

```rust
pub trait ToolFormatter: Default + Send + Sync {
    fn format_tools(&self, tools: &[Tool]) -> Result<serde_json::Value, ...>;
    fn tool_calling_instructions(&self) -> Option<String>;
    fn extract_tool_calls(&self, response: &str) -> Result<ExtractResult, ...>;
    fn format_tool_response(&self, result: &Messages) -> Result<serde_json::Value, ...>;
}
```

Each provider (Anthropic, OpenAI, etc.) implements `ToolFormatter` to convert internal `Tool` definitions to its API's format. No additional conversion layer is needed.

### External Tools (MCP, HTTP, CLI)

External tools are registered in the ToolShed like any other tool:

- **MCP servers**: MCP tool definitions are converted to `Tool` and added to ToolShed. The `shed` tool's vector store includes MCP tool descriptions.
- **HTTP endpoints**: Described as tools with HTTP call semantics. Added to ToolShed.
- **CLI commands**: Wrapped as tools with argument parsing. Added to ToolShed.

The agent doesn't need to know the difference — all tools look the same through ToolShed.

### Tool Versioning

Tools present their interface via their JSON Schema. If a tool's interface changes, the new schema is registered in the ToolShed. Versioning is the tool's own concern — the ToolShed stores whatever schema the tool provides.

### Tool Discovery via Vector Store

The vector store query for tool discovery uses the same `VectorStore` trait as message semantic recall, but in a separate namespace:

```rust
impl VectorStore {
    /// Query tool descriptions in the toolshed namespace
    pub fn query_tools(&self, query: &str, top_k: usize) -> Vec<ToolSummary> {
        self.query_in_namespace("toolshed", query, top_k)
    }
}
```

This allows the same vector search infrastructure to serve both:
- **Message recall** — "what did I say about auth?"
- **Tool discovery** — "is there a tool for auth?"

## Rationale

**Why ToolShed instead of flat tool list?**
- The `shed` meta-tool saves context — agent doesn't need all tool definitions upfront
- Vector-stored descriptions enable semantic tool discovery
- Structured categories (read, edit, write, search, bash, memory, delegate) cover common operations

**Why no `others` field?**
- `shed` tool covers dynamic discovery — no need for a catch-all field
- Keeps ToolShed focused on core operations
- External tools discovered via `shed` search, not hardcoded in struct

**Why store tool descriptions in vector store?**
- Semantic matching — "parse YAML" finds `yaml_parser` even without exact keyword match
- Fast lookup — vector search is O(log n) with index
- Reuses existing VectorStore infrastructure

## Alternatives Considered

### Flat tool list in ModelInteraction
- **Pros:** Simpler
- **Cons:** All tool definitions in context — wastes tokens, limits number of tools
- **Rejected because:** Agent may have hundreds of tools — can't fit all in context

### Separate tool registry outside ToolShed
- **Pros:** Clearer separation
- **Cons:** More complexity, ToolShed already provides the structure
- **Rejected because:** ToolShed + `shed` tool is sufficient for all use cases

### Tool versioning in ToolShed
- **Pros:** Track tool interface changes
- **Cons:** Adds complexity, tool owns its interface
- **Rejected because:** Tool presents its schema — if it changes, new schema replaces old
