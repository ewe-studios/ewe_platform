use foundation_ai::backends::openai_responses_provider::*;
use foundation_ai::types::base_types::TextContent;
use foundation_ai::types::*;

#[test]
fn test_response_request_serialization() {
    let request = ResponseRequest {
        model: "o1".into(),
        input: ResponseInput::Text("Hello".into()),
        instructions: Some("Be helpful".into()),
        tools: None,
        tool_choice: None,
        max_output_tokens: Some(100),
        temperature: Some(1.0),
        top_p: None,
        stream: Some(false),
        truncate: None,
        previous_response_id: None,
    };

    let json = serde_json::to_string(&request).unwrap();
    assert!(json.contains(r#""model":"o1""#));
    assert!(json.contains(r#""input":"Hello""#));
    assert!(json.contains(r#""instructions":"Be helpful""#));
    assert!(json.contains(r#""max_output_tokens":100"#));
    assert!(json.contains(r#""stream":false"#));
}

#[test]
fn test_response_input_items() {
    let input = ResponseInput::Items(vec![ResponseInputItem::Message {
        role: "user".into(),
        content: ResponseInputContent::Text("Hello".into()),
    }]);

    let json = serde_json::to_string(&input).unwrap();
    assert!(json.contains(r#""type":"message""#));
    assert!(json.contains(r#""role":"user""#));
}

#[test]
fn test_response_deserialization() {
    let json = r#"{
        "id": "resp_abc123",
        "object": "response",
        "created_at": 1234567890,
        "model": "o1",
        "output": [{
            "type": "message",
            "id": "msg_001",
            "status": "completed",
            "role": "assistant",
            "content": [{"type": "output_text", "text": "Hello!"}]
        }],
        "status": "completed",
        "usage": {"input_tokens": 10, "output_tokens": 5, "total_tokens": 15}
    }"#;

    let response: Response = serde_json::from_str(json).unwrap();
    assert_eq!(response.id, "resp_abc123");
    assert_eq!(response.status, "completed");
    assert_eq!(response.output.len(), 1);
    assert_eq!(response.usage.as_ref().unwrap().total_tokens, 15);
}

#[test]
fn test_response_output_item_reasoning() {
    let json = r#"{
        "type": "reasoning",
        "id": "rs_001",
        "content": "Let me think about this..."
    }"#;

    let item: ResponseOutputItem = serde_json::from_str(json).unwrap();
    match item {
        ResponseOutputItem::Reasoning { content, .. } => {
            assert_eq!(content, "Let me think about this...");
        }
        _ => panic!("Expected Reasoning variant"),
    }
}

#[test]
fn test_response_output_item_function_call() {
    let json = r#"{
        "type": "function_call",
        "id": "fc_001",
        "call_id": "call_123",
        "name": "get_weather",
        "arguments": "{\"location\": \"Paris\"}",
        "status": "completed"
    }"#;

    let item: ResponseOutputItem = serde_json::from_str(json).unwrap();
    match item {
        ResponseOutputItem::FunctionCall {
            name, arguments, ..
        } => {
            assert_eq!(name, "get_weather");
            assert!(arguments.contains("Paris"));
        }
        _ => panic!("Expected FunctionCall variant"),
    }
}

#[test]
fn test_response_event_deserialization() {
    let json = r#"{
        "type": "response.output_text.delta",
        "item_id": "msg_001",
        "delta": "Hello"
    }"#;

    let event: ResponseEvent = serde_json::from_str(json).unwrap();
    match event {
        ResponseEvent::ResponseOutputTextDelta { delta, .. } => {
            assert_eq!(delta, "Hello");
        }
        _ => panic!("Expected ResponseOutputTextDelta variant"),
    }
}

#[test]
fn test_response_completed_event() {
    let json = r#"{
        "type": "response.completed",
        "response": {
            "id": "resp_abc",
            "object": "response",
            "created_at": 0,
            "model": "o1",
            "output": [],
            "status": "completed"
        }
    }"#;

    let event: ResponseEvent = serde_json::from_str(json).unwrap();
    match event {
        ResponseEvent::ResponseCompleted { response } => {
            assert_eq!(response.id, "resp_abc");
        }
        _ => panic!("Expected ResponseCompleted variant"),
    }
}

#[test]
fn test_build_response_input_from_messages() {
    let interaction = ModelInteraction {
        system_prompt: Some("Be helpful".into()),
        soul: None,
        messages: vec![Messages::User {
            id: foundation_compact::ids::new_scru128(),
            role: "user".into(),
            content: foundation_ai::types::base_types::UserModelContent::Text(TextContent {
                content: "Hello".into(),
                signature: None,
            }),
            signature: None,
        }],
        tools_shed: ToolShed::default(),
        chat_template: None,
        tool_choice: None,
    };

    let input = build_response_input(&interaction);
    match input {
        ResponseInput::Items(items) => {
            assert_eq!(items.len(), 1);
            match &items[0] {
                ResponseInputItem::Message { role, content } => {
                    assert_eq!(role, "user");
                    assert!(matches!(content, ResponseInputContent::Text(_)));
                }
                _ => panic!("Expected Message item"),
            }
        }
        _ => panic!("Expected Items input"),
    }
}

#[test]
fn test_configs_defaults() {
    let config = ResponsesConfig::default();
    assert_eq!(config.base_url, "https://api.openai.com");
    assert_eq!(config.api_version, "v1");
    assert_eq!(config.timeout_secs, 120);
    assert!(config.streaming);
}
