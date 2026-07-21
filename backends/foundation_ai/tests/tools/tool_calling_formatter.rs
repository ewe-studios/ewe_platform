//! Unit tests for `ToolFormatter` implementations.
//!
//! Tests AnthropicFormatter, OpenAIFormatter, and TextBasedFormatter
//! with sample provider responses.

use foundation_ai::backends::anthropic_messages_provider::AnthropicFormatter;
use foundation_ai::backends::openai_provider::OpenAIFormatter;
use foundation_ai::types::{
    ArgType, Args, Messages, ModelOutput, TextBasedFormatter, TextContent, ToolCallingError,
    UserModelContent,
};

use foundation_ai::types::{Tool, ToolDefinition, ToolFormatter};
use foundation_jsonschema::{scheme, ValidationOptions};
use std::time::SystemTime;

// ============================================================================
// Helpers
// ============================================================================

fn make_tool(name: &str, description: &str) -> Tool {
    let opts: ValidationOptions = scheme::object()
        .required("query", scheme::string().min_len(1))
        .build();
    Tool::SingleCommand(ToolDefinition {
        name: name.to_string(),
        category: "search".into(),
        description: description.to_string(),
        arguments: Args::new(opts),
        returns: None,
    })
}

// ============================================================================
// AnthropicFormatter
// ============================================================================

#[test]
fn anthropic_format_tools() {
    let formatter = AnthropicFormatter;
    let tools = vec![make_tool("search", "Search the web")];

    let result = formatter.format_tools(&tools).unwrap();
    let arr = result.as_array().unwrap();
    assert_eq!(arr.len(), 1);

    let tool = &arr[0];
    assert_eq!(tool["name"], "search");
    assert_eq!(tool["description"], "Search the web");
    assert_eq!(tool["input_schema"]["type"], "object");
    assert!(tool["input_schema"]["properties"].get("query").is_some());
}

#[test]
fn anthropic_format_tools_empty() {
    let formatter = AnthropicFormatter;
    let result = formatter.format_tools(&[]).unwrap();
    let arr = result.as_array().unwrap();
    assert!(arr.is_empty());
}

#[test]
fn anthropic_tool_calling_instructions_none() {
    let formatter = AnthropicFormatter;
    assert!(formatter.tool_calling_instructions().is_none());
}

#[test]
fn anthropic_extract_tool_calls_tool_use() {
    let formatter = AnthropicFormatter;
    let response = r#"{
        "content": [
            {"type": "text", "text": "Let me search for you."},
            {"type": "tool_use", "id": "tool_abc123", "name": "search", "input": {"query": "rust"}}
        ]
    }"#;

    let result = formatter.extract_tool_calls(response).unwrap();
    assert!(result.has_tool_calls);
    assert_eq!(result.calls.len(), 1);

    let call = &result.calls[0];
    match call {
        ModelOutput::ToolCall { id, name, .. } => {
            assert_eq!(id, "tool_abc123");
            assert_eq!(name, "search");
        }
        _ => panic!("expected ToolCall"),
    }

    assert_eq!(
        result.remaining_text,
        Some("Let me search for you.".to_string())
    );
}

#[test]
fn anthropic_extract_tool_calls_multiple() {
    let formatter = AnthropicFormatter;
    let response = r#"{
        "content": [
            {"type": "tool_use", "id": "t1", "name": "get_weather", "input": {"location": "Paris"}},
            {"type": "tool_use", "id": "t2", "name": "get_weather", "input": {"location": "London"}}
        ]
    }"#;

    let result = formatter.extract_tool_calls(response).unwrap();
    assert_eq!(result.calls.len(), 2);
    assert!(result.has_tool_calls);
    assert!(result.remaining_text.is_none());
}

#[test]
fn anthropic_extract_no_tool_calls() {
    let formatter = AnthropicFormatter;
    let response = r#"{
        "content": [
            {"type": "text", "text": "Hello! How can I help?"}
        ]
    }"#;

    let result = formatter.extract_tool_calls(response).unwrap();
    assert!(!result.has_tool_calls);
    assert!(result.calls.is_empty());
    assert_eq!(
        result.remaining_text,
        Some("Hello! How can I help?".to_string())
    );
}

#[test]
fn anthropic_format_tool_response() {
    let formatter = AnthropicFormatter;
    let result = Messages::ToolResult {
        id: foundation_compact::ids::new_scru128(),
        tool_call_id: "tool_abc123".to_string(),
        name: "search".to_string(),
        timestamp: SystemTime::now(),
        details: None,
        content: UserModelContent::Text(TextContent {
            content: "Found 3 results".to_string(),
            signature: None,
        }),
        error_detail: None,
        signature: None,
    };

    let formatted = formatter.format_tool_response(&result).unwrap();
    assert_eq!(formatted["type"], "tool_result");
    assert_eq!(formatted["tool_use_id"], "tool_abc123");
    assert_eq!(formatted["is_error"], false);

    let content_arr = formatted["content"].as_array().unwrap();
    assert_eq!(content_arr.len(), 1);
    assert_eq!(content_arr[0]["text"], "Found 3 results");
}

#[test]
fn anthropic_format_tool_response_error() {
    let formatter = AnthropicFormatter;
    let result = Messages::ToolResult {
        id: foundation_compact::ids::new_scru128(),
        tool_call_id: "search".to_string(),
        name: "search".to_string(),
        timestamp: SystemTime::now(),
        details: None,
        content: UserModelContent::Text(TextContent {
            content: "Connection refused".to_string(),
            signature: None,
        }),
        error_detail: Some("timeout".to_string()),
        signature: None,
    };

    let formatted = formatter.format_tool_response(&result).unwrap();
    assert_eq!(formatted["is_error"], true);
}

#[test]
fn anthropic_format_tool_response_wrong_variant() {
    let formatter = AnthropicFormatter;
    let wrong = Messages::User {
        id: foundation_compact::ids::new_scru128(),
        role: foundation_ai::types::MessageRole::User,
        content: UserModelContent::Text(TextContent {
            content: "hello".to_string(),
            signature: None,
        }),
        signature: None,
    };

    let err = formatter.format_tool_response(&wrong).unwrap_err();
    // ErrorTrace<ToolCallingError::Response>
    assert!(err
        .current_context()
        .to_string()
        .contains("expected Messages::ToolResult"));
}

#[test]
fn anthropic_extract_invalid_json() {
    let formatter = AnthropicFormatter;
    let err = formatter.extract_tool_calls("not json at all").unwrap_err();
    assert!(err
        .current_context()
        .to_string()
        .contains("failed to extract tool calls"));
}

// ============================================================================
// OpenAIFormatter
// ============================================================================

#[test]
fn openai_format_tools() {
    let formatter = OpenAIFormatter;
    let tools = vec![make_tool("search", "Search the web")];

    let result = formatter.format_tools(&tools).unwrap();
    let arr = result.as_array().unwrap();
    assert_eq!(arr.len(), 1);

    let tool = &arr[0];
    assert_eq!(tool["type"], "function");
    assert_eq!(tool["function"]["name"], "search");
    assert_eq!(tool["function"]["description"], "Search the web");
    assert_eq!(tool["function"]["parameters"]["type"], "object");
}

#[test]
fn openai_tool_calling_instructions_none() {
    let formatter = OpenAIFormatter;
    assert!(formatter.tool_calling_instructions().is_none());
}

#[test]
fn openai_extract_tool_calls() {
    let formatter = OpenAIFormatter;
    let response = r#"{
        "choices": [{
            "message": {
                "content": "Let me check.",
                "tool_calls": [
                    {
                        "id": "call_xyz",
                        "type": "function",
                        "function": {
                            "name": "search",
                            "arguments": "{\"query\":\"rust\"}"
                        }
                    }
                ]
            }
        }]
    }"#;

    let result = formatter.extract_tool_calls(response).unwrap();
    assert!(result.has_tool_calls);
    assert_eq!(result.calls.len(), 1);

    let call = &result.calls[0];
    match call {
        ModelOutput::ToolCall { id, name, .. } => {
            assert_eq!(id, "call_xyz");
            assert_eq!(name, "search");
        }
        _ => panic!("expected ToolCall"),
    }

    assert_eq!(result.remaining_text, Some("Let me check.".to_string()));
}

#[test]
fn openai_extract_no_tool_calls() {
    let formatter = OpenAIFormatter;
    let response = r#"{
        "choices": [{
            "message": {
                "content": "Hello!",
                "tool_calls": null
            }
        }]
    }"#;

    let result = formatter.extract_tool_calls(response).unwrap();
    assert!(!result.has_tool_calls);
    assert!(result.calls.is_empty());
    assert_eq!(result.remaining_text, Some("Hello!".to_string()));
}

#[test]
fn openai_format_tool_response() {
    let formatter = OpenAIFormatter;
    let result = Messages::ToolResult {
        id: foundation_compact::ids::new_scru128(),
        tool_call_id: "call_xyz".to_string(),
        name: "search".to_string(),
        timestamp: SystemTime::now(),
        details: None,
        content: UserModelContent::Text(TextContent {
            content: "Found 3 results".to_string(),
            signature: None,
        }),
        error_detail: None,
        signature: None,
    };

    let formatted = formatter.format_tool_response(&result).unwrap();
    assert_eq!(formatted["role"], "tool");
    assert_eq!(formatted["tool_call_id"], "call_xyz");
    assert_eq!(formatted["content"], "Found 3 results");
}

#[test]
fn openai_format_tool_response_wrong_variant() {
    let formatter = OpenAIFormatter;
    let wrong = Messages::User {
        id: foundation_compact::ids::new_scru128(),
        role: foundation_ai::types::MessageRole::User,
        content: UserModelContent::Text(TextContent {
            content: "hello".to_string(),
            signature: None,
        }),
        signature: None,
    };

    let err = formatter.format_tool_response(&wrong).unwrap_err();
    assert!(err
        .current_context()
        .to_string()
        .contains("expected Messages::ToolResult"));
}

// ============================================================================
// TextBasedFormatter
// ============================================================================

#[test]
fn text_based_format_tools() {
    let formatter = TextBasedFormatter;
    let tools = vec![make_tool("search", "Search the web")];

    let result = formatter.format_tools(&tools).unwrap();
    let arr = result.as_array().unwrap();
    assert_eq!(arr.len(), 1);

    let tool = &arr[0];
    assert_eq!(tool["name"], "search");
    assert_eq!(tool["description"], "Search the web");
    assert_eq!(tool["parameters"]["type"], "object");
}

#[test]
fn text_based_tool_calling_instructions() {
    let formatter = TextBasedFormatter;
    let instructions = formatter.tool_calling_instructions().unwrap();
    assert!(instructions.contains("<ToolCall>"));
    assert!(instructions.contains("</ToolCall>"));
    assert!(instructions.contains("name"));
    assert!(instructions.contains("arguments"));
}

#[test]
fn text_based_extract_single_tool_call() {
    let formatter = TextBasedFormatter;
    let response = r#"I'll search for you.
<ToolCall>{"name": "search", "arguments": {"query": "rust"}}</ToolCall>
Done."#;

    let result = formatter.extract_tool_calls(response).unwrap();
    assert!(result.has_tool_calls);
    assert_eq!(result.calls.len(), 1);

    let call = &result.calls[0];
    match call {
        ModelOutput::ToolCall {
            id: _,
            name,
            arguments: args,
            ..
        } => {
            assert_eq!(name, "search");
            let args = args.as_ref().unwrap();
            // The "arguments" field in the JSON is a nested object
            let inner_args = args
                .get("arguments")
                .and_then(|v| match v {
                    ArgType::JSONMap(m) => Some(m),
                    _ => None,
                })
                .unwrap();
            assert_eq!(inner_args["query"], ArgType::Text("rust".to_string()));
        }
        _ => panic!("expected ToolCall"),
    }

    assert_eq!(
        result.remaining_text,
        Some("I'll search for you.\nDone.".to_string())
    );
}

#[test]
fn text_based_extract_multiple_tool_calls() {
    let formatter = TextBasedFormatter;
    let response = r#"I'll check the weather.
<ToolCall>{"name": "get_weather", "arguments": {"location": "Paris"}}</ToolCall>
<ToolCall>{"name": "get_weather", "arguments": {"location": "London"}}</ToolCall>
Let me also check Tokyo.
<ToolCall>{"name": "get_weather", "arguments": {"location": "Tokyo"}}</ToolCall>"#;

    let result = formatter.extract_tool_calls(response).unwrap();
    assert!(result.has_tool_calls);
    assert_eq!(result.calls.len(), 3);

    assert_eq!(
        result.remaining_text,
        Some("I'll check the weather.\nLet me also check Tokyo.".to_string())
    );
}

#[test]
fn text_based_extract_no_tool_calls() {
    let formatter = TextBasedFormatter;
    let response = "Hello! How can I help you today?";

    let result = formatter.extract_tool_calls(response).unwrap();
    assert!(!result.has_tool_calls);
    assert!(result.calls.is_empty());
    assert_eq!(result.remaining_text, Some(response.to_string()));
}

#[test]
fn text_based_extract_malformed_json_kept_as_text() {
    let formatter = TextBasedFormatter;
    let response = r#"<ToolCall>{not valid json}</ToolCall>
Some text after."#;

    let result = formatter.extract_tool_calls(response).unwrap();
    // Malformed JSON should not be extracted as a tool call
    assert!(!result.has_tool_calls);
    assert!(result.calls.is_empty());
    // The malformed tag + text should be in remaining
    assert!(result.remaining_text.is_some());
}

#[test]
fn text_based_extract_whitespace_inside_tags() {
    let formatter = TextBasedFormatter;
    let response = r#"<ToolCall>  {"name": "search", "arguments": {"query": "rust"}}  </ToolCall>"#;

    let result = formatter.extract_tool_calls(response).unwrap();
    assert!(result.has_tool_calls);
    assert_eq!(result.calls.len(), 1);
}

#[test]
fn text_based_format_tool_response() {
    let formatter = TextBasedFormatter;
    let result = Messages::ToolResult {
        id: foundation_compact::ids::new_scru128(),
        tool_call_id: "tool_1".to_string(),
        name: "search".to_string(),
        timestamp: SystemTime::now(),
        details: None,
        content: UserModelContent::Text(TextContent {
            content: "Found 3 results".to_string(),
            signature: None,
        }),
        error_detail: None,
        signature: None,
    };

    let formatted = formatter.format_tool_response(&result).unwrap();
    assert_eq!(formatted["role"], "tool");
    assert_eq!(formatted["name"], "search");
    assert!(formatted["content"].as_str().unwrap().contains("search"));
    assert!(formatted["content"]
        .as_str()
        .unwrap()
        .contains("Found 3 results"));
}

#[test]
fn text_based_format_tool_response_error_note() {
    let formatter = TextBasedFormatter;
    let result = Messages::ToolResult {
        id: foundation_compact::ids::new_scru128(),
        tool_call_id: "tool_1".to_string(),
        name: "search".to_string(),
        timestamp: SystemTime::now(),
        details: None,
        content: UserModelContent::Text(TextContent {
            content: "Connection refused".to_string(),
            signature: None,
        }),
        error_detail: Some("timeout".to_string()),
        signature: None,
    };

    let formatted = formatter.format_tool_response(&result).unwrap();
    let content = formatted["content"].as_str().unwrap();
    assert!(content.contains("(error)"));
}

#[test]
fn text_based_format_tool_response_wrong_variant() {
    let formatter = TextBasedFormatter;
    let wrong = Messages::User {
        id: foundation_compact::ids::new_scru128(),
        role: foundation_ai::types::MessageRole::User,
        content: UserModelContent::Text(TextContent {
            content: "hello".to_string(),
            signature: None,
        }),
        signature: None,
    };

    let err = formatter.format_tool_response(&wrong).unwrap_err();
    assert!(err
        .current_context()
        .to_string()
        .contains("expected Messages::ToolResult"));
}

// ============================================================================
// Error type tests
// ============================================================================

#[test]
fn tool_calling_error_display() {
    let extract_err = ToolCallingError::Extract {
        reason: "invalid JSON".to_string(),
    };
    assert!(extract_err
        .to_string()
        .contains("failed to extract tool calls"));
    assert!(extract_err.to_string().contains("invalid JSON"));

    let format_err = ToolCallingError::Format {
        tool_name: "search".to_string(),
        reason: "missing name".to_string(),
    };
    assert!(format_err
        .to_string()
        .contains("failed to format tool 'search'"));

    let response_err = ToolCallingError::Response {
        tool_name: "weather".to_string(),
        reason: "bad input".to_string(),
    };
    assert!(response_err
        .to_string()
        .contains("failed to format result for 'weather'"));
}

// ============================================================================
// function_spec — returns field propagation
// ============================================================================

fn make_tool_with_returns(name: &str, description: &str, returns_schema: serde_json::Value) -> Tool {
    let opts: ValidationOptions = scheme::object()
        .required("query", scheme::string().min_len(1))
        .build();
    let returns_opts = ValidationOptions::with_schema(returns_schema);
    Tool::SingleCommand(ToolDefinition {
        name: name.to_string(),
        category: "search".into(),
        description: description.to_string(),
        arguments: Args::new(opts),
        returns: Some(Args::new(returns_opts)),
    })
}

#[test]
fn function_spec_returns_propagated() {
    let tool = make_tool_with_returns(
        "search",
        "Search the web",
        serde_json::json!({"type": "object", "properties": {"url": {"type": "string"}}, "required": ["url"]}),
    );
    let spec = tool.function_spec();
    assert_eq!(spec.name, "search");
    assert!(spec.returns.is_some());
    let ret = spec.returns.unwrap();
    assert_eq!(ret["type"], "object");
    assert!(ret["properties"].get("url").is_some());
}

#[test]
fn function_spec_returns_none_when_absent() {
    let tool = make_tool("search", "Search");
    let spec = tool.function_spec();
    assert!(spec.returns.is_none());
}

#[test]
fn function_spec_multi_commands_collects_first_returns() {
    let returns_a = ValidationOptions::with_schema(serde_json::json!({"type": "string"}));
    let returns_b = ValidationOptions::with_schema(serde_json::json!({"type": "integer"}));
    let tool = Tool::MultiCommands(
        "multi".into(),
        vec![
            ToolDefinition {
                name: "cmd_a".into(), category: "multi".into(),
                description: "A".into(),
                arguments: Args::new(ValidationOptions::with_schema(serde_json::json!({"type": "object"}))),
                returns: Some(Args::new(returns_a)),
            },
            ToolDefinition {
                name: "cmd_b".into(), category: "multi".into(),
                description: "B".into(),
                arguments: Args::new(ValidationOptions::with_schema(serde_json::json!({"type": "object"}))),
                returns: Some(Args::new(returns_b)),
            },
        ],
    );
    let spec = tool.function_spec();
    assert_eq!(spec.name, "multi");
    // First non-None returns wins
    assert!(spec.returns.is_some());
    assert_eq!(spec.returns.unwrap()["type"], "string");
}

// ============================================================================
// OpenAIFormatter — strict / returns
// ============================================================================

#[test]
fn openai_format_tools_emits_strict_when_returns_present() {
    let formatter = OpenAIFormatter;
    let tool = make_tool_with_returns(
        "search",
        "Search",
        serde_json::json!({"type": "object", "properties": {"url": {"type": "string"}}}),
    );
    let result = formatter.format_tools(&[tool]).unwrap();
    let arr = result.as_array().unwrap();
    let func = &arr[0]["function"];
    assert_eq!(func["strict"], true, "strict must be true when returns is present");
}

#[test]
fn openai_format_tools_no_strict_when_returns_absent() {
    let formatter = OpenAIFormatter;
    let tool = make_tool("search", "Search");
    let result = formatter.format_tools(&[tool]).unwrap();
    let arr = result.as_array().unwrap();
    let func = &arr[0]["function"];
    // strict should be absent (not false, not null)
    assert!(func.get("strict").is_none(), "strict should be absent: {func}");
}

// ============================================================================
// AnthropicFormatter — ignores returns
// ============================================================================

#[test]
fn anthropic_format_tools_ignores_returns() {
    let formatter = AnthropicFormatter;
    let tool = make_tool_with_returns(
        "search",
        "Search",
        serde_json::json!({"type": "object", "properties": {"url": {"type": "string"}}}),
    );
    let result = formatter.format_tools(&[tool]).unwrap();
    let arr = result.as_array().unwrap();
    let obj = &arr[0];
    // Anthropic has no output_schema / strict field
    assert!(obj.get("output_schema").is_none());
    assert!(obj.get("strict").is_none());
    assert_eq!(obj["name"], "search");
}

// ============================================================================
// TextBasedFormatter — Returns: embedded in description
// ============================================================================

#[test]
fn text_based_format_tools_embeds_returns_in_description() {
    let formatter = TextBasedFormatter;
    let tool = make_tool_with_returns(
        "search",
        "Search the web",
        serde_json::json!({"type": "object", "properties": {"url": {"type": "string"}}, "required": ["url"]}),
    );
    let result = formatter.format_tools(&[tool]).unwrap();
    let arr = result.as_array().unwrap();
    let desc = arr[0]["description"].as_str().unwrap();
    assert!(desc.contains("Search the web"));
    assert!(desc.contains("Returns:"), "expected Returns: in description, got: {desc}");
    assert!(desc.contains("required"), "expected schema in description, got: {desc}");
    assert!(desc.contains("url"), "expected 'url' prop in description, got: {desc}");
}

#[test]
fn text_based_format_tools_no_returns_in_description_when_absent() {
    let formatter = TextBasedFormatter;
    let tool = make_tool("search", "Search the web");
    let result = formatter.format_tools(&[tool]).unwrap();
    let arr = result.as_array().unwrap();
    let desc = arr[0]["description"].as_str().unwrap();
    assert_eq!(desc, "Search the web");
    assert!(!desc.contains("Returns:"), "should not have Returns: {desc}");
}

#[test]
fn errstack_trace_propagation() {
    let formatter = TextBasedFormatter;
    // Plain text with no tool calls — returns Ok with empty results
    let result = formatter.extract_tool_calls("totally invalid").unwrap();
    assert!(!result.has_tool_calls);
    assert!(result.calls.is_empty());
    assert_eq!(result.remaining_text, Some("totally invalid".to_string()));
}
