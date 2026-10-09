# How-To: Building Custom Tools

Define a tool, read its arguments safely, register it, and test it. Doc 04 is
the reference for the tool runtime.

---

## 1. The `ToolImpl` trait

```rust
#[async_trait]
pub trait ToolImpl: Send + Sync {
    fn definition(&self) -> Tool;   // Tool::SingleCommand(..) or Tool::MultiCommands(..)
    async fn execute(&self, arguments: HashMap<String, ArgType>)
        -> Result<ToolCallResult, ToolError>;
}
```

`execute` is async; cancellation drops the future, so clean up with `Drop`
(kill child processes, close connections).

## 2. A single-command tool

```rust
use std::collections::HashMap;
use async_trait::async_trait;
use foundation_ai::agentic::tool_impl::{ToolCallResult, ToolDefinition, ToolError, ToolImpl};
use foundation_ai::types::{ArgType, Args, TextContent, Tool, UserModelContent};
use foundation_jsonschema::scheme;

struct WeatherTool { api_key: String }

#[async_trait]
impl ToolImpl for WeatherTool {
    fn definition(&self) -> Tool {
        Tool::SingleCommand(ToolDefinition {
            name: "get_weather".into(),
            category: "read".into(),
            description: "Get the current weather for a city.".into(),
            arguments: Args::new(
                scheme::object()
                    .required("location", scheme::string().min_len(1).description("City name"))
                    .build(),
            ),
            returns: None,
        })
    }

    async fn execute(&self, arguments: HashMap<String, ArgType>) -> Result<ToolCallResult, ToolError> {
        let location = text_arg(&arguments, "location")?;
        let report = self.fetch(&location).await.map_err(|e| ToolError::Execution {
            tool: "get_weather".into(),
            reason: e.to_string(),
        })?;
        Ok(ToolCallResult {
            content: UserModelContent::Text(TextContent { content: report, signature: None }),
            error_detail: None,
        })
    }
}
```

`Args::from_value(json!({...}))` works too if you'd rather write the schema as
JSON. `returns: Some(Args::new(..))` declares a result schema; OpenAI backends
then mark the function `strict`.

## 3. A multi-command tool

Group related commands under one name; the model picks one with a `command`
argument (`Tool::function_spec` builds a `oneOf` schema for you):

```rust
fn definition(&self) -> Tool {
    Tool::MultiCommands("todo".into(), vec![
        ToolDefinition { name: "add".into(),  category: "todo".into(),
                         description: "Add an item".into(),
                         arguments: Args::new(scheme::object().required("item", scheme::string()).build()),
                         returns: None },
        ToolDefinition { name: "list".into(), category: "todo".into(),
                         description: "List items".into(),
                         arguments: Args::empty(), returns: None },
    ])
}

async fn execute(&self, args: HashMap<String, ArgType>) -> Result<ToolCallResult, ToolError> {
    match text_arg(&args, "command")?.as_str() {
        "add" => self.add(&args),
        "list" => self.list(),
        other => Err(ToolError::InvalidArguments {
            tool: "todo".into(),
            reason: format!("unknown command '{other}'"),
        }),
    }
}
```

The built-in `memory` and `agent` tools follow this pattern.

## 4. Reading arguments

Before `execute` runs, the arguments are checked against your schema
(`arguments`, or the chosen command's schema for a multi-command tool). A
violation goes back to the model as an `InvalidArguments` result and your tool
is not called, so a `required` string is guaranteed present by the time you
read it.

Every backend maps the model's JSON the same way:

| JSON value | `ArgType` |
|---|---|
| string | `Text(s)` |
| integer | `I64(n)` |
| other number | `Float64(x)` |
| boolean, null, array, object | `JSON(text)` — e.g. `JSON("true")`, `JSON("[1,2]")` |

Helpers in `agentic::tool_impl`:

```rust
use foundation_ai::agentic::tool_impl::{arg_bool, arg_usize};

let verbose = arg_bool(&args, "verbose").unwrap_or(false);   // JSON("true") or Text("true")
let limit = arg_usize(&args, "limit").unwrap_or(10);         // I64, unsigned variants, or "10"

fn text_arg(args: &HashMap<String, ArgType>, key: &str) -> Result<String, ToolError> {
    match args.get(key) {
        Some(ArgType::Text(s)) => Ok(s.clone()),
        _ => Err(ToolError::InvalidArguments {
            tool: "my_tool".into(),
            reason: format!("missing or non-string '{key}'"),
        }),
    }
}
```

For structured values, parse the JSON text:
`serde_json::from_str::<MyType>(raw)` on `ArgType::JSON(raw)`, or
`arg.to_json_value()` for any variant.

## 5. Errors

```rust
pub enum ToolError {
    UnknownTool(String),
    InvalidArguments { tool: String, reason: String },
    Execution { tool: String, reason: String },
    Timeout { tool: String },
    Cancelled(String),
}
```

An `Err` is not fatal to the turn: the loop turns it into a
`Messages::ToolResult` (with `error_detail`) and the model sees it. So:

- `InvalidArguments` for bad input — say what was wrong, the model can retry.
- `Execution` for runtime failures — include enough detail to pick another
  approach.
- `Timeout` is the only kind retried by the default `ToolRetryConfig`.

A panic inside `execute` is caught and reported as `Execution`.

## 6. Categories

`category` is a free-form tag used for grouping and by tool discovery
(`shed`). The built-ins use `read`, `write`, `edit`, `shell`, `search`,
`search_files`, `memory`, `agent` and `discovery`. It doesn't change how the
loop runs the tool.

## 7. Registering

Tools go in the session's `ToolShed`:

```rust
let agent = AgentSession::builder(router)
    .with_toolshed(ToolShed::new().tool(WeatherTool { api_key }))
    .build()?;
```

A tool that needs the session (its context, memory or id) is added as a
constructor, which `build()` calls with the session's parts:

```rust
use foundation_ai::agentic::tool_fn;

let tools = ToolShed::new().tool(tool_fn("notes", |s| {
    Arc::new(NotesTool::new(Arc::clone(&s.memory))) as Arc<dyn ToolImpl>
}));
```

The model finds the tool through the `shed` meta-tool — write a `description`
that says what the tool is for, since that is what `shed` searches. Bundles:
`ToolPreset::from_tools(vec![..])`, passed to `ToolShed::tools(..)`.

## 8. Dependencies between calls

A model may send several calls with `depends_on` ids. The manager orders them
(Doc 04 §4) and runs them one after another. A tool doesn't see its
dependencies' results directly; the model reads them on the next round.

## 9. Testing

```rust
#[test]
fn weather_tool_reads_location() {
    let tool = WeatherTool { api_key: "test".into() };

    let Tool::SingleCommand(def) = tool.definition() else { panic!("expected single command") };
    assert_eq!(def.name, "get_weather");

    let args = HashMap::from([("location".to_string(), ArgType::Text("London".into()))]);
    let result = futures_lite::future::block_on(tool.execute(args));
    assert!(result.is_ok());
}
```

For whole-turn tests, `agentic::testing` (feature `testing`) has
`MockModelProvider` (scripted model replies, e.g. `mock_tool_call(..)`) and
`MockTool`.
