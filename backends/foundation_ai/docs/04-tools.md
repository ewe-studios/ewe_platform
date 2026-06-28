# Fundamentals 04 — ToolImpl, ToolShed, and the execution DAG

How tools are defined, registered, discovered, and executed with dependency
ordering.

---

## 1. ToolImpl trait

Every tool implements this:

```rust
#[async_trait]
pub trait ToolImpl: Send + Sync {
    fn definition(&self) -> ToolDefinition;
    async fn execute(&self, arguments: HashMap<String, ArgType>)
        -> Result<ToolCallResult, ToolError>;
}
```

**`ToolDefinition`**:
```rust
pub struct ToolDefinition {
    pub name: String,           // "read_file"
    pub description: String,    // "Read the contents of a file"
    pub arguments: Args,        // JSON Schema for arguments
    pub category: String,       // "read", "write", "shell", "search"
}
```

**`ToolCallResult`**:
```rust
pub struct ToolCallResult {
    pub content: UserModelContent,  // The tool's output
    pub error_detail: Option<String>,
}
```

## 2. ToolCallManager — registration and execution

```rust
let manager = ToolCallManager::new(session_id);
manager.register(Arc::new(ReadFileTool::new(vfs)));
manager.register(Arc::new(ShellTool::new(shell)));

// Execute a single tool call
let request = ToolCallRequest {
    id: "call-1",
    name: "read_file",
    arguments: HashMap::from([("path".into(), ArgType::Text("/src/main.rs".into()))]),
    depends_on: vec![],
    execution_hint: ExecutionHint::SideEffectFree,
};
let result = manager.execute_one(&request).await?;
```

## 3. ToolShed — tool discovery

`build_toolshed()` categorizes registered tools by category:

```rust
pub struct ToolShed {
    pub shed: Option<ToolDefinition>,      // Discovery metatool
    pub read: Option<ToolDefinition>,      // Read tools
    pub write: Option<ToolDefinition>,     // Write tools
    pub shell: Option<ToolDefinition>,     // Shell tools
    pub search: Option<ToolDefinition>,    // Search tools
    pub search_files: Option<ToolDefinition>, // File search
}
```

The `shed` tool is a metatool that lists all available tools — the agent can
discover what it can do at runtime.

## 4. Execution DAG

Tool calls can declare dependencies:

```rust
ToolCallRequest {
    id: "call-3",
    name: "process_results",
    depends_on: vec!["call-1".into(), "call-2".into()], // waits for both
    ...
}
```

The executor builds a DAG and:
1. Runs all tools with no dependencies in parallel
2. When a tool completes, runs tools that depend on it
3. Cycles detected → error

## 5. Retry configuration

Tools can be configured with retry behavior:

```rust
pub struct ToolRetryConfig {
    pub max_retries: u32,
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
    pub backoff_multiplier: f64,
}
```

Default: 3 retries, 100ms initial, 5s max, 2× multiplier.

## 6. Built-in tools

### search_context
Searches the agent's knowledge graph and memory for relevant context.
Modes: Semantic, Memory, Graph, Hybrid.

### search_file
Searches files via grep (content) or find (path). Supports regex patterns.

### shed (metatool)
Lists all available tools and their descriptions. Used by the agent to
discover capabilities.

## 7. Tool error types

```rust
pub enum ToolError {
    UnknownTool(String),           // Tool not registered
    InvalidArguments { tool, reason }, // Schema validation failed
    Execution { tool, reason },    // Tool execution failed
    DependencyCycle { tools },     // Circular dependency detected
    DependencyFailed { tool },     // A dependency tool failed
    Timeout(String),               // Tool took too long
}
```

Each maps to an `AgenticError::ToolCall` for the error policy to handle.
