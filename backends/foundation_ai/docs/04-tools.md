# Fundamentals 04 — ToolImpl, ToolShed, ToolPreset, and the execution DAG

How tools are defined, registered, discovered, and executed with dependency
ordering. (Updated for F19 unified tool model.)

---

## 1. ToolImpl trait

Every tool implements this:

```rust
#[async_trait]
pub trait ToolImpl: Send + Sync {
    fn definition(&self) -> Tool;    // Tool::SingleCommand or Tool::MultiCommands
    async fn execute(&self, arguments: HashMap<String, ArgType>)
        -> Result<ToolCallResult, ToolError>;
}
```

**`ToolDefinition`** (each command's descriptor):
```rust
pub struct ToolDefinition {
    pub name: String,              // "read_file" or "start" (for MultiCommands)
    pub category: String,          // "read", "write", "shell", "agent", …
    pub description: String,       // Human-readable description shown to the LLM
    pub arguments: Args,           // JSON Schema for arguments
    pub returns: Option<Args>,     // JSON Schema for the tool's result (→ strict mode)
}
```

**`Tool`** (the shape presented to providers):
```rust
pub enum Tool {
    SingleCommand(ToolDefinition),                    // e.g. "bash"
    MultiCommands(String, Vec<ToolDefinition>),       // e.g. "agent" → [start, check, …]
}
```

**`ToolCallResult`**:
```rust
pub struct ToolCallResult {
    pub content: UserModelContent,   // The tool's output
    pub error_detail: Option<String>,
}
```

### Tool::function_spec() — the bridge to providers

`function_spec()` converts a `Tool` into a `ToolFunctionSpec` that each provider
renders into its wire format:

```rust
pub struct ToolFunctionSpec {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,      // JSON Schema for arguments
    pub returns: Option<serde_json::Value>,  // JSON Schema for the result
}
```

Provider mapping:

| Provider | `name` / `description` / `parameters` | `returns` |
|----------|------|---------|
| OpenAI Chat | `function.name/description/parameters` | `strict: true` on the function object |
| OpenAI Responses | `ResponseTool.name/description/parameters` | `strict: Some(true)` |
| Anthropic | `name/description/input_schema` | ignored (no output-schema slot) |
| Text-based (llama.cpp/Candle) | `name/description/parameters` | appended to description as `Returns: {schema}` |

---

## 2. ToolCallManager — registration and execution

```rust
let manager = ToolCallManager::new(session_id);
manager.register(Arc::new(ReadFileTool::new(vfs)));
manager.register(Arc::new(ShellTool::new()));

// Execute a single tool call
let request = ToolCallRequest {
    id: "call-1",
    name: "read_file",
    arguments: HashMap::from([("path".into(), ArgType::Text("/src/main.rs".into()))]),
    depends_on: vec![],
    execution_hint: ExecutionHint::Unspecified,
};
let result = manager.execute_one(&request).await?;
```

### Using ToolPreset (harness)

`harness::ToolPreset` bundles common tools for quick registration:

```rust
use foundation_ai::harness::ToolPreset;

// Single-tool presets:
ToolPreset::files(fs)        // → read, write, edit
ToolPreset::shell()          // → bash
ToolPreset::memory(h)        // → memory add/remove/replace (MultiCommands)
ToolPreset::agent(...)       // → agent start/check/result/… (MultiCommands)
ToolPreset::shed(d)          // → tool discovery metatool

// Compose via merge() or +:
let preset = ToolPreset::files(fs)
    .merge(ToolPreset::shell())
    .merge(ToolPreset::memory(hierarchy));

// Two modes of use:
preset.register_all(session.tool_manager());     // register on existing manager
let child_tools = preset.as_child_tools();       // for AgentTool's child_tools
let mgr = preset.into_manager(session_id);       // fresh ToolCallManager

// Composite presets:
ToolPreset::minimal_sub_agent(fs)                // files + shell
ToolPreset::standard(fs, hierarchy, discovery)   // files + shell + memory + shed
```

---

## 3. ToolShed — what the model sees

`ToolCallManager::build_toolshed()` collects registered tools:

```rust
pub struct ToolShed {
    pub shed: Option<Tool>,   // Discovery metatool (auto-added when ≥1 real tool)
    pub tools: Vec<Tool>,     // All other registered tools
}
```

No more per-category slots — each tool declares its own shape (single or multi
command) via `ToolImpl::definition()`. The shed metatool is included only when
there is at least one real tool to discover.

---

## 4. Execution DAG

Tool calls can declare dependencies:

```rust
ToolCallRequest {
    id: "call-3",
    name: "process_results",
    depends_on: vec!["call-1".into(), "call-2".into()],
    ...
}
```

The executor:
1. Groups calls by dependency depth into stages
2. Runs each stage in parallel or sequentially (per `ExecutionHint`)
3. Cycles detected → error

---

## 5. Retry configuration

```rust
pub struct ToolRetryConfig {
    pub max_retries: u32,              // default: 3
    pub initial_backoff: Duration,     // default: 1s
    pub backoff_multiplier: f64,       // default: 2.0
    pub max_backoff: Duration,         // default: 30s
    pub retry_on: Vec<ToolErrorKind>,  // default: Timeout, Network
}
```

---

## 6. Built-in tools

| Tool | Module | Type |
|------|--------|------|
| `memory add/remove/replace` | `agentic::tools::memory` | MultiCommands (F14/F19) |
| `agent start/check/result/pause/resume/stop` | `agentic::tools::agent` | MultiCommands (F15) |
| `read`, `write`, `edit` | `agentic::tools::files` | SingleCommand × 3 |
| `bash` | `agentic::tools::files` | SingleCommand |
| `search_file`, `search_content` | `agentic::tools::search` | SingleCommand × 2 |
| `shed` | `agentic::tools::shed` | SingleCommand (metatool) |

---

## 7. Building a custom tool

```rust
use std::sync::Arc;
use std::collections::HashMap;
use async_trait::async_trait;
use foundation_ai::agentic::tool_impl::{
    ToolImpl, ToolCallResult, ToolDefinition, ToolError, ToolCallManager,
};
use foundation_ai::types::{ArgType, Args, Tool, TextContent, UserModelContent};

struct GreetTool;

#[async_trait]
impl ToolImpl for GreetTool {
    fn definition(&self) -> Tool {
        Tool::SingleCommand(ToolDefinition {
            name: "greet".into(),
            category: "custom".into(),
            description: "Greet someone by name.".into(),
            arguments: Args::new(
                foundation_jsonschema::scheme::object()
                    .required("name", foundation_jsonschema::scheme::string().min_len(1))
                    .build(),
            ),
            returns: Some(Args::new(
                foundation_jsonschema::scheme::object()
                    .required("greeting", foundation_jsonschema::scheme::string())
                    .build(),
            )),
        })
    }

    async fn execute(
        &self,
        arguments: HashMap<String, ArgType>,
    ) -> Result<ToolCallResult, ToolError> {
        let name = match arguments.get("name") {
            Some(ArgType::Text(s)) => s.clone(),
            _ => return Err(ToolError::InvalidArguments {
                tool: "greet".into(),
                reason: "missing 'name'".into(),
            }),
        };
        Ok(ToolCallResult {
            content: UserModelContent::Text(TextContent {
                content: format!("Hello, {name}!"),
                signature: None,
            }),
            error_detail: None,
        })
    }
}

// Register:
let manager = ToolCallManager::new(session_id);
manager.register(Arc::new(GreetTool));
```

---

## 8. Tool error types

```rust
pub enum ToolError {
    UnknownTool(String),
    InvalidArguments { tool: String, reason: String },
    Execution { tool: String, reason: String },
    Timeout { tool: String },
    Cancelled(String),
}
```
