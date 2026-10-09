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

Every `AgentSession` owns a `ToolCallManager`. Give it tools on the builder,
or register them on the session later; the loop rebuilds the `ToolShed` from
the registry before every generation, so later registration takes effect on
the next request.

```rust
let agent = AgentSession::<Doc, Mem>::builder(session_id, router)
    .with_tool(Arc::new(BashTool::new()))
    .with_tools(ToolPreset::files(Arc::clone(&fs)).as_child_tools())
    .build()?;

// …or after build:
agent.tool_manager().register(Arc::new(MyTool));
```

`with_toolshed(..)` only declares tools that `build()` must find registered
(a preflight check); the model is offered whatever is registered either way.

A standalone manager (tests, custom loops):

```rust
let manager = ToolCallManager::new(session_id);
manager.register(Arc::new(ReadTool::new(fs)));

let request = ToolCallRequest {
    id: "call-1".into(),
    name: "read".into(),
    arguments: HashMap::from([("path".into(), ArgType::Text("/src/main.rs".into()))]),
    depends_on: vec![],
    execution_hint: ExecutionHint::Unspecified,
};
let result = futures_lite::future::block_on(manager.execute_one(&request));
```

`execute_one` looks the tool up, validates the arguments against the tool's
JSON Schema (`validate_arguments`; for a `MultiCommands` tool, against the
schema of the command named by `command`), then runs it with panic
containment: a panicking tool becomes `ToolError::Execution`. A schema
violation is `ToolError::InvalidArguments` and the tool does not run.

Other methods: `deregister`, `get`, `get_def`, `names`, `build_toolshed`,
`build_workflow`, `execute_with_retry`, `set_retry_config` / `retry_config`.

### Using ToolPreset (harness)

`harness::ToolPreset` bundles common tools for quick registration:

```rust
use foundation_ai::harness::ToolPreset;

// Single-tool presets:
ToolPreset::files(fs)        // → read, write, edit
ToolPreset::shell()          // → bash
ToolPreset::memory(h)        // → memory add/remove/replace (MultiCommands)
                             //   h = Arc::new(agent.memory_hierarchy().clone())
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

Each tool declares its own shape (single or multi command) via
`ToolImpl::definition()`. A registered tool named `shed` becomes the `shed`
field; when none is registered, a default `shed` definition is added. When
there are no other tools, `shed` is `None` — advertising discovery to a
tool-less model made it call a phantom `shed()`.

---

## 4. Execution order (the dependency DAG)

Tool calls can declare dependencies:

```rust
ToolCallRequest {
    id: "call-3".into(),
    name: "process_results".into(),
    depends_on: vec!["call-1".into(), "call-2".into()],
    ..
}
```

`ToolCallManager::build_workflow` groups calls into stages by dependency
depth. A stage is `Sequential { fail_fast: true }` if any call in it has
`ExecutionHint::Sequential`, otherwise `Parallel { fail_mode: CollectAll }`.
Cycles and unknown dependency ids are `ToolError::InvalidArguments`; the loop
reports them as a `FailedAction`.

The agent loop flattens the stages and runs the calls **one at a time** in that
order, each through `execute_with_retry`. The parallel / sequential stage
distinction is computed but not yet used for concurrency.

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

Set it per tool with `manager.set_retry_config("name", cfg)`. `ToolError::kind()`
never returns `Network` today (only `Timeout`, `Execution`,
`InvalidArguments`), so in practice only timeouts are retried.

---

## 6. Built-in tools

| Tool | Module | Type |
|------|--------|------|
| `memory add/remove/replace` | `agentic::tools::memory` | MultiCommands (F14/F19) |
| `agent start/check/result/pause/resume/stop` | `agentic::tools::agent` | MultiCommands (F15) |
| `read`, `write`, `edit` | `agentic::tools::files` | SingleCommand × 3 |
| `bash` | `agentic::tools::files` | SingleCommand |
| `search_file` (filesystem), `search_context` (session recall) | `agentic::tools::search` | SingleCommand × 2 (native only) |
| `shed` | `agentic::tools::shed` | SingleCommand (metatool; needs a `ToolDiscovery` with an embedder + vector store) |

`register_file_tools(manager, fs)` and `register_shell_tool(manager)` in
`agentic::tools::files` register the file and shell tools without a preset.

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
