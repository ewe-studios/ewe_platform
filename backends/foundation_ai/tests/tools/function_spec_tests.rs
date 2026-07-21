//! Tests for `Tool::function_spec()` — the bridge from `ToolDefinition` to
//! `ToolFunctionSpec` that providers consume. Covers returns field propagation
//! for both SingleCommand and MultiCommands shapes (F19).

use foundation_ai::types::{Args, Tool, ToolDefinition};
use foundation_jsonschema::ValidationOptions;

fn make_opts() -> ValidationOptions {
    foundation_jsonschema::scheme::object()
        .required("query", foundation_jsonschema::scheme::string().min_len(1))
        .build()
}

fn make_tool() -> Tool {
    Tool::SingleCommand(ToolDefinition {
        name: "search".into(),
        category: "search".into(),
        description: "Search the web".into(),
        arguments: Args::new(make_opts()),
        returns: None,
    })
}

fn make_tool_with_returns(schema: serde_json::Value) -> Tool {
    Tool::SingleCommand(ToolDefinition {
        name: "search".into(),
        category: "search".into(),
        description: "Search the web".into(),
        arguments: Args::new(make_opts()),
        returns: Some(Args::new(ValidationOptions::with_schema(schema))),
    })
}

#[test]
fn single_no_returns() {
    let spec = make_tool().function_spec();
    assert_eq!(spec.name, "search");
    assert_eq!(spec.description, "Search the web");
    assert_eq!(spec.parameters["type"], "object");
    assert!(spec.returns.is_none());
}

#[test]
fn single_with_returns() {
    let tool = make_tool_with_returns(serde_json::json!({
        "type": "object",
        "properties": {"url": {"type": "string"}},
        "required": ["url"]
    }));
    let spec = tool.function_spec();
    assert!(spec.returns.is_some());
    let ret = spec.returns.unwrap();
    assert_eq!(ret["type"], "object");
    assert!(ret["properties"]["url"].is_object());
}

#[test]
fn multi_commands_no_returns() {
    let tool = Tool::MultiCommands(
        "multi".into(),
        vec![
            ToolDefinition {
                name: "a".into(),
                category: "m".into(),
                description: "A".into(),
                arguments: Args::new(make_opts()),
                returns: None,
            },
            ToolDefinition {
                name: "b".into(),
                category: "m".into(),
                description: "B".into(),
                arguments: Args::new(make_opts()),
                returns: None,
            },
        ],
    );
    let spec = tool.function_spec();
    assert!(spec.returns.is_none());
}

#[test]
fn multi_commands_collects_first_returns() {
    let tool = Tool::MultiCommands(
        "agent".into(),
        vec![
            ToolDefinition {
                name: "start".into(),
                category: "agent".into(),
                description: "Start a sub-agent".into(),
                arguments: Args::new(ValidationOptions::with_schema(serde_json::json!({
                    "type": "object",
                    "properties": {"task": {"type": "string"}},
                    "required": ["task"]
                }))),
                returns: None,
            },
            ToolDefinition {
                name: "check".into(),
                category: "agent".into(),
                description: "Check status".into(),
                arguments: Args::new(ValidationOptions::with_schema(serde_json::json!({
                    "type": "object",
                    "properties": {"id": {"type": "string"}},
                    "required": ["id"]
                }))),
                returns: Some(Args::new(ValidationOptions::with_schema(serde_json::json!({
                    "type": "object",
                    "properties": {"state": {"type": "string"}}
                })))),
            },
        ],
    );
    let spec = tool.function_spec();
    assert_eq!(spec.name, "agent");
    assert!(spec.description.contains("start"));
    assert!(spec.description.contains("check"));

    // oneOf with 2 branches, each pinning command
    let branches = spec.parameters["oneOf"].as_array().unwrap();
    assert_eq!(branches.len(), 2);
    assert_eq!(branches[0]["properties"]["command"]["const"], "start");
    assert_eq!(branches[1]["properties"]["command"]["const"], "check");

    // First non-None returns wins (start had None, check had one)
    assert!(spec.returns.is_some());
    assert_eq!(
        spec.returns.unwrap()["properties"]["state"]["type"],
        "string"
    );
}

#[test]
fn multi_commands_takes_first_non_none_returns() {
    // If the first command has returns, it should be used, not the second.
    let tool = Tool::MultiCommands(
        "multi".into(),
        vec![
            ToolDefinition {
                name: "first".into(),
                category: "m".into(),
                description: "First".into(),
                arguments: Args::new(make_opts()),
                returns: Some(Args::new(ValidationOptions::with_schema(
                    serde_json::json!({"type": "string"}),
                ))),
            },
            ToolDefinition {
                name: "second".into(),
                category: "m".into(),
                description: "Second".into(),
                arguments: Args::new(make_opts()),
                returns: Some(Args::new(ValidationOptions::with_schema(
                    serde_json::json!({"type": "integer"}),
                ))),
            },
        ],
    );
    let spec = tool.function_spec();
    assert!(spec.returns.is_some());
    assert_eq!(spec.returns.unwrap()["type"], "string");
}
