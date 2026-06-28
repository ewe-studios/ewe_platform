# How-To: Building Custom Tools

How to define, implement, and register custom tools for the agent.

---

## 1. The ToolImpl trait

Every tool implements this:

```rust
#[async_trait]
pub trait ToolImpl: Send + Sync {
    fn definition(&self) -> ToolDefinition;
    async fn execute(
        &self,
        arguments: HashMap<String, ArgType>,
    ) -> Result<ToolCallResult, ToolError>;
}
```

## 2. Defining your tool

```rust
use foundation_ai::agentic::tool_impl::*;
use foundation_ai::types::{ArgType, Args, UserModelContent, TextContent};

struct WeatherTool {
    api_key: String,
}

#[async_trait]
impl ToolImpl for WeatherTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "get_weather".into(),
            description: "Get the current weather for a location".into(),
            arguments: Args::from_value(serde_json::json!({
                "type": "object",
                "properties": {
                    "location": {
                        "type": "string",
                        "description": "City name or zip code"
                    },
                    "units": {
                        "type": "string",
                        "enum": ["celsius", "fahrenheit"],
                        "default": "celsius"
                    }
                },
                "required": ["location"]
            })),
            category: "read".into(),
        }
    }

    async fn execute(
        &self,
        arguments: HashMap<String, ArgType>,
    ) -> Result<ToolCallResult, ToolError> {
        // Extract arguments
        let location = arguments
            .get("location")
            .and_then(|v| match v {
                ArgType::Text(s) => Some(s.clone()),
                _ => None,
            })
            .ok_or_else(|| ToolError::InvalidArguments {
                tool: "get_weather".into(),
                reason: "missing 'location' argument".into(),
            })?;

        let units = arguments
            .get("units")
            .and_then(|v| match v {
                ArgType::Text(s) => Some(s.clone()),
                _ => None,
            })
            .unwrap_or_else(|| "celsius".into());

        // Call the weather API
        let weather = self.fetch_weather(&location, &units).await
            .map_err(|e| ToolError::Execution {
                tool: "get_weather".into(),
                reason: e.to_string(),
            })?;

        // Return the result
        Ok(ToolCallResult {
            content: UserModelContent::Text(TextContent {
                content: weather.to_string(),
                signature: None,
            }),
            error_detail: None,
        })
    }
}
```

## 3. Registering tools

```rust
// Create a ToolCallManager and register tools
let manager = ToolCallManager::new(session_id);
manager.register(Arc::new(WeatherTool { api_key: "key".into() }));
manager.register(Arc::new(ReadFileTool::new(vfs)));
manager.register(Arc::new(ShellTool::new(shell)));

// Build the ToolShed
let toolshed = manager.build_toolshed();

// Use in AgentSession
let session = AgentSession::builder(SessionId::new(), router)
    .with_toolshed(toolshed)
    .build()?;
```

## 4. Argument types

Tools receive arguments as `HashMap<String, ArgType>`:

```rust
enum ArgType {
    Text(String),          // String values
    Number(f64),           // Numeric values
    Bool(bool),            // Boolean values
    Array(Vec<ArgType>),   // Lists
    Object(HashMap<String, ArgType>), // Nested objects
    Null,                  // Explicit null
}
```

**Extracting arguments:**

```rust
fn extract_text(args: &HashMap<String, ArgType>, key: &str) -> Result<String, ToolError> {
    args.get(key)
        .and_then(|v| match v {
            ArgType::Text(s) => Some(s.clone()),
            _ => None,
        })
        .ok_or_else(|| ToolError::InvalidArguments {
            tool: "my_tool".into(),
            reason: format!("missing '{}' argument", key),
        })
}

fn extract_number(args: &HashMap<String, ArgType>, key: &str) -> Result<f64, ToolError> {
    args.get(key)
        .and_then(|v| match v {
            ArgType::Number(n) => Some(*n),
            _ => None,
        })
        .ok_or_else(|| ToolError::InvalidArguments {
            tool: "my_tool".into(),
            reason: format!("missing '{}' argument", key),
        })
}
```

## 5. Error handling in tools

```rust
enum ToolError {
    UnknownTool(String),           // Tool not found
    InvalidArguments { tool, reason }, // Bad arguments
    Execution { tool, reason },    // Execution failed
    DependencyCycle { tools },     // Circular dependency
    DependencyFailed { tool },     // Dependency tool failed
    Timeout(String),               // Tool took too long
}
```

**Best practices:**
- Return `InvalidArguments` for bad input (agent can fix and retry)
- Return `Execution` for runtime errors (agent can try alternative)
- Include detailed `reason` messages (helps agent understand what went wrong)

## 6. Tool categories

Categories determine how the agent uses the tool:

| Category | Behavior | Examples |
|---|---|---|
| `read` | Safe, fast, no side effects | `read_file`, `search_context` |
| `write` | Modifies state | `write_file`, `edit_file` |
| `shell` | Powerful, potentially dangerous | `run_command`, `eval` |
| `search` | Read-only, semantic | `search_file`, `grep` |
| `shed` | Discovery metatool | `list_tools` |

## 7. Tool dependencies

Tools can depend on other tools' results:

```rust
ToolCallRequest {
    id: "process-data",
    name: "process_results",
    arguments: /* ... */,
    depends_on: vec!["read-file-1".into(), "read-file-2".into()],
    execution_hint: ExecutionHint::Blocking,
}
```

The executor builds a DAG:
1. Runs `read-file-1` and `read-file-2` in parallel
2. When both complete, runs `process-data` with their results

## 8. Testing tools

```rust
#[test]
fn test_weather_tool() {
    let tool = WeatherTool { api_key: "test-key".into() };
    
    // Verify definition
    let def = tool.definition();
    assert_eq!(def.name, "get_weather");
    assert_eq!(def.category, "read");
    
    // Test execution
    let args = HashMap::from([
        ("location".into(), ArgType::Text("London".into())),
        ("units".into(), ArgType::Text("celsius".into())),
    ]);
    
    let result = futures_lite::future::block_on(tool.execute(args));
    assert!(result.is_ok());
}
```
