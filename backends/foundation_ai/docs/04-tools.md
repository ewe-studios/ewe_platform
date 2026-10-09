# Fundamentals 04 — ToolImpl, ToolShed, `shed`, ToolPreset, and the execution DAG

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
ToolCallResult::text("done")         // plain-text result
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

## 2. ToolShed — the session's tools

A session's tools are given in one place: a `ToolShed` passed to
`with_toolshed`. The shed holds tool *constructors*, one per name.
`AgentSession::build()` calls each constructor with the session's parts and
the result is the session's `ToolCallManager` — so every tool is registered by
construction, and there are no per-tool methods on the session builder.

```rust
use foundation_ai::agentic::{tool_fn, AgentSession, ToolShed};
use foundation_ai::harness::ToolPreset;

let tools = ToolShed::new()
    .tool(GreetTool)                                  // a ready-made ToolImpl
    .tools(ToolPreset::files(Arc::clone(&fs)))        // a preset
    .tools(ToolPreset::shell())
    .tools(ToolPreset::search_context())              // built from the session
    .tool(tool_fn("notes", |s| {                      // your own session-dependent tool
        Arc::new(NotesTool::new(Arc::clone(&s.memory))) as Arc<dyn ToolImpl>
    }));

let agent = AgentSession::builder(router)
    .with_model("my-model")
    .with_toolshed(tools)
    .build()?;
```

- `ToolShed::tool(..)` takes anything `Into<Box<dyn ToolConstructor>>`: a
  `ToolImpl` value, an `Arc<T>` / `Arc<dyn ToolImpl>`, or a constructor from
  `tool_fn`. `tools(..)` takes any iterator of constructors (a `ToolPreset`, a
  `Vec<Box<dyn ToolConstructor>>`).
- `SessionParts` is what a constructor gets: the `session_id`, the session's
  `context` (`Arc<dyn ContextSearch>` — recall over its stores and embedder),
  its `memory` (`Arc<dyn MemoryAccess>`) and its `embedder`. These are the same
  `Arc`s the session holds.
- `build()` fails with `ToolShedError::DuplicateTool(name)` when a name was
  added twice, `ReservedName("shed")` for a tool named `shed`, and
  `NameMismatch` when a constructor builds a tool under a different name. The
  error converts into `AgenticError`.
- `ToolShed::get(name)` / `contains(name)` are O(1) lookups; `names()` lists
  the tools.

`agent.tool_manager().register(Arc::new(MyTool))` after `build()` still works
for adding a tool to a running session; it is not the normal path.

## 3. What the model sees — `shed` first

The model is offered exactly one tool up front, the built-in `shed`
meta-tool, however many tools the session has. `shed` is how it learns what
else it can call:

- `shed { description, limit }` searches the registered tools and returns each
  hit's name, description, category and argument schema (`ShedResult`). With
  an embedder on the session (`with_embedder`), it searches by embedding
  through `ToolDiscovery`; otherwise — and to fill the rest of `limit` — it
  matches the query's words against tool names, descriptions and categories.
- Every tool `shed` returns becomes **active**, and active tools are declared
  on every later request. Native tool calling (Anthropic, OpenAI) only lets a
  model call tools declared in the request, so a tool must be declared before
  it can be called.
- A call to a tool that isn't active gets a tool error
  (`ToolError::NotActive`) telling the model to look it up with `shed` first;
  the tool does not run.
- A session with no tools declares nothing (not even `shed`).

```rust
// What the loop declares on each request:
let declared: ToolDeclarations = agent.tool_manager().offered_tools();
// = shed + agent.tool_manager().active_definitions()

agent.tool_manager().is_active("read");            // has shed returned it?
agent.tool_manager().activate(&["read".into()]);   // pre-activate (custom loops, tests)
agent.tool_manager().all_declarations();           // every registered tool, for inspection
```

`ToolDeclarations` (in `types`) is the provider-facing list — definitions only:

```rust
pub struct ToolDeclarations {
    pub shed: Option<Tool>,   // the `shed` meta-tool
    pub tools: Vec<Tool>,     // the active tools
}
```

## 4. ToolCallManager — the registry

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

`execute_one` runs `shed` itself; for any other name it looks the tool up,
validates the arguments against the tool's JSON Schema (`validate_arguments`;
for a `MultiCommands` tool, against the schema of the command named by
`command`), then runs it with panic containment: a panicking tool becomes
`ToolError::Execution`. A schema violation is `ToolError::InvalidArguments`
and the tool does not run. (The "is it active?" check is the loop's, via
`check_offered`, so a standalone manager can run any registered tool.)

Other methods: `deregister`, `get`, `get_def`, `names`, `offered_tools`,
`active_definitions`, `activate`, `is_active`, `check_offered`,
`search_tools`, `enable_discovery`, `all_declarations`, `build_workflow`,
`execute_with_retry`, `set_retry_config` / `retry_config`.

### Using ToolPreset (harness)

`harness::ToolPreset` bundles common tools. A preset is a list of tool
constructors, so it goes straight into `ToolShed::tools(..)`:

```rust
use foundation_ai::harness::ToolPreset;

// Ready-made presets:
ToolPreset::files(fs)        // → read, write, edit
ToolPreset::shell()          // → bash
ToolPreset::memory(h)        // → memory add/remove/replace over a given hierarchy
ToolPreset::agent(...)       // → agent start/check/result/… (MultiCommands)

// Built from the session inside build():
ToolPreset::search_context() // → search_context over the session's stores + embedder
ToolPreset::session_memory() // → memory over the session's own hierarchy

// Compose via merge() or +:
let preset = ToolPreset::files(fs).merge(ToolPreset::shell());

// Normal use:
let tools = ToolShed::new().tools(preset);

// Ready-made presets only (Err(ToolShedError::NeedsSession) otherwise):
preset.register_all(session.tool_manager())?;    // add to a running session
let child_tools = preset.as_child_tools()?;       // for AgentTool's child_tools
let mgr = preset.into_manager(session_id)?;       // fresh ToolCallManager

// Composite presets:
ToolPreset::minimal_sub_agent(fs)                // files + shell
ToolPreset::standard(fs)                         // files + shell + session memory
```

---

## 5. Execution order (the dependency DAG)

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

## 6. Retry configuration

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

## 7. Built-in tools

| Tool | Module | Type |
|------|--------|------|
| `memory add/remove/replace` | `agentic::tools::memory` | MultiCommands (F14/F19) |
| `agent start/check/result/pause/resume/stop` | `agentic::tools::agent` | MultiCommands (F15) |
| `read`, `write`, `edit` | `agentic::tools::files` | SingleCommand × 3 |
| `bash` | `agentic::tools::files` | SingleCommand |
| `search_file` (filesystem), `search_context` (session recall) | `agentic::tools::search` | SingleCommand × 2 (native only) |
| `shed` | built into every `ToolCallManager` (`agentic::tools::shed` holds `ToolDiscovery`) | SingleCommand meta-tool; embedding search when the session has an embedder, name/description match otherwise |

`register_file_tools(manager, fs)` and `register_shell_tool(manager)` in
`agentic::tools::files` register the file and shell tools without a preset.

---

## 8. Building a custom tool

```rust
use std::collections::HashMap;
use async_trait::async_trait;
use foundation_ai::agentic::tool_impl::{ToolImpl, ToolCallResult, ToolDefinition, ToolError};
use foundation_ai::agentic::{AgentSession, FnTool, ToolArgs, ToolShed};
use foundation_ai::types::{ArgType, Args, Tool};

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
        let name = ToolArgs::new("greet", &arguments).str("name")?;   // InvalidArguments if missing
        Ok(ToolCallResult::text(format!("Hello, {name}!")))
    }
}

// The same tool as a closure:
let greet = FnTool::new(
    "greet",
    "Greet someone by name.",
    Args::new(foundation_jsonschema::scheme::object()
        .required("name", foundation_jsonschema::scheme::string().min_len(1))
        .build()),
    |args: ToolArgs<'_>| {
        let name = args.str("name").map(str::to_owned);
        async move { Ok(ToolCallResult::text(format!("Hello, {}!", name?))) }
    },
);

// Give it to a session:
let agent = AgentSession::builder(router)
    .with_model("my-model")
    .with_toolshed(ToolShed::new().tool(GreetTool))
    .build()?;
```

`ToolArgs` reads arguments whichever way the backend spelled them (`str`,
`i64`, `usize`, `f64`, `bool`, their `opt_*` forms, `opt_value`, and
`parse::<T>()` for a whole struct); Doc 10 §4 has the details.

---

## 9. Tool error types

```rust
pub enum ToolError {
    UnknownTool(String),
    InvalidArguments { tool: String, reason: String },
    Execution { tool: String, reason: String },
    Timeout { tool: String },
    Cancelled(String),
    NotActive(String),   // registered, but `shed` hasn't returned it yet
}
```
