use foundation_ai::backends::openai_provider::*;
use foundation_ai::types::base_types::*;
use foundation_ai::types::*;

#[test]
fn test_openai_config_defaults() {
    let config = OpenAIConfig::default();
    assert_eq!(config.base_url, "https://api.openai.com");
    assert_eq!(config.api_version, "v1");
    assert_eq!(config.timeout_secs, 30);
    assert_eq!(config.max_retries, 3);
    assert!(config.proxy_url.is_none());
    assert!(config.streaming);
}

#[test]
fn test_openai_config_builder() {
    let config = OpenAIConfig::new()
        .with_base_url("http://localhost:8080")
        .with_timeout_secs(60)
        .with_streaming(false);
    assert_eq!(config.base_url, "http://localhost:8080");
    assert_eq!(config.timeout_secs, 60);
    assert!(!config.streaming);
}

#[test]
fn test_build_url() {
    let config = OpenAIConfig::new()
        .with_base_url("http://localhost:8080")
        .with_api_version("v1");
    assert_eq!(
        config.build_url("chat/completions"),
        "http://localhost:8080/v1/chat/completions"
    );
}

#[test]
fn test_model_id_to_string() {
    assert_eq!(
        model_id_to_string(&ModelId::Name("gpt-4".into(), None)),
        "gpt-4"
    );
    assert_eq!(model_id_to_string(&ModelId::Alias("4".into(), None)), "4");
}

#[test]
fn test_parse_sse_chunks() {
    let chunks_raw = vec![
        r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4","choices":[{"index":0,"delta":{"role":"assistant","content":"Hello"},"finish_reason":null}]}"#,
        r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4","choices":[{"index":0,"delta":{"content":" world"},"finish_reason":null}]}"#,
        r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}"#,
    ];

    let mut accumulated_text = String::new();
    let mut finish_reason: Option<String> = None;

    for raw in &chunks_raw {
        let chunk: ChatCompletionChunk = serde_json::from_str(raw).unwrap();
        for choice in &chunk.choices {
            if let Some(ref delta) = choice.delta {
                if let Some(ref content) = delta.content {
                    accumulated_text.push_str(content);
                }
            }
            if choice.finish_reason.is_some() {
                finish_reason = choice.finish_reason.clone();
            }
        }
    }

    assert_eq!(accumulated_text, "Hello world");
    assert_eq!(finish_reason.as_deref(), Some("stop"));

    let stop_reason = match finish_reason.as_deref() {
        Some("stop") | None => StopReason::Stop,
        Some("length") => StopReason::Length,
        Some("tool_calls") => StopReason::ToolUse,
        Some(_) => StopReason::Error,
    };
    assert_eq!(stop_reason, StopReason::Stop);
}

#[test]
fn test_chat_completion_request_serialization() {
    let request = ChatCompletionRequest {
        model: "gpt-4".into(),
        messages: vec![
            OpenAIMessage {
                role: "system".into(),
                content: Some(OpenAIMessageContent::Text("You are helpful".into())),
                tool_calls: None,
                tool_call_id: None,
                refusal: None,
            },
            OpenAIMessage {
                role: "user".into(),
                content: Some(OpenAIMessageContent::Text("Hello".into())),
                tool_calls: None,
                tool_call_id: None,
                refusal: None,
            },
        ],
        temperature: Some(0.7),
        top_p: Some(0.9),
        max_tokens: Some(100),
        stop: None,
        stream: Some(false),
        tools: None,
        tool_choice: None,
        n: Some(1),
        seed: None,
        frequency_penalty: None,
        presence_penalty: None,
        logit_bias: None,
        response_format: None,
        logprobs: None,
        top_logprobs: None,
    };

    let json = serde_json::to_string(&request).unwrap();
    assert!(json.contains("\"model\":\"gpt-4\""));
    assert!(json.contains("\"temperature\":0.7"));
    assert!(json.contains("\"stream\":false"));
}

#[test]
fn test_chat_response_deserialization() {
    let json = r#"{
        "id": "chatcmpl-123",
        "object": "chat.completion",
        "created": 1234567890,
        "model": "gpt-4",
        "choices": [{
            "index": 0,
            "message": { "role": "assistant", "content": "Hello!" },
            "finish_reason": "stop"
        }],
        "usage": { "prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15 }
    }"#;

    let response: ChatCompletionResponse = serde_json::from_str(json).unwrap();
    assert_eq!(response.id, "chatcmpl-123");
    assert_eq!(
        response.choices[0].message.as_ref().unwrap().content,
        Some(OpenAIMessageContent::Text("Hello!".into()))
    );
    assert!(response.usage.is_some());
    assert_eq!(response.usage.as_ref().unwrap().total_tokens, 15);
}

#[test]
fn test_tool_call_delta_deserialization() {
    let chunk_json = r#"{
        "id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4",
        "choices":[{
            "index":0,
            "delta":{"tool_calls":[{"index":0,"id":"call_abc","type":"function","function":{"name":"get_weather","arguments":""}}]},
            "finish_reason":null
        }]
    }"#;

    let chunk: ChatCompletionChunk = serde_json::from_str(chunk_json).unwrap();
    let delta = chunk.choices[0].delta.as_ref().unwrap();
    let tc = &delta.tool_calls.as_ref().unwrap()[0];
    assert_eq!(tc.index, 0);
    assert_eq!(tc.id.as_deref(), Some("call_abc"));
    assert_eq!(
        tc.function.as_ref().unwrap().name.as_deref(),
        Some("get_weather")
    );
}

#[test]
fn test_tool_call_delta_continuation() {
    let continuation = r#"{
        "id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4",
        "choices":[{
            "index":0,
            "delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"loc"}}]},
            "finish_reason":null
        }]
    }"#;

    let chunk: ChatCompletionChunk = serde_json::from_str(continuation).unwrap();
    let delta = chunk.choices[0].delta.as_ref().unwrap();
    let tc = &delta.tool_calls.as_ref().unwrap()[0];
    assert_eq!(tc.index, 0);
    assert!(tc.id.is_none());
    assert_eq!(
        tc.function.as_ref().unwrap().arguments.as_deref(),
        Some("{\"loc")
    );
}

#[test]
fn test_tool_call_accumulation() {
    let chunks = vec![
        r#"{"id":"c1","object":"chat.completion.chunk","created":0,"model":"gpt-4","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"name":"get_weather","arguments":""}}]},"finish_reason":null}]}"#,
        r#"{"id":"c1","object":"chat.completion.chunk","created":0,"model":"gpt-4","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"location\""}}]},"finish_reason":null}]}"#,
        r#"{"id":"c1","object":"chat.completion.chunk","created":0,"model":"gpt-4","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":": \"Paris\"}"}}]},"finish_reason":null}]}"#,
    ];

    let mut tool_calls: Vec<AccumulatedToolCall> = Vec::new();

    for raw in &chunks {
        let chunk: ChatCompletionChunk = serde_json::from_str(raw).unwrap();
        for choice in &chunk.choices {
            if let Some(ref delta) = choice.delta {
                if let Some(ref tcs) = delta.tool_calls {
                    for tc_delta in tcs {
                        let idx = tc_delta.index as usize;
                        while tool_calls.len() <= idx {
                            tool_calls.push(AccumulatedToolCall {
                                id: String::new(),
                                name: String::new(),
                                arguments: String::new(),
                            });
                        }
                        let tc = &mut tool_calls[idx];
                        if let Some(ref id) = tc_delta.id {
                            tc.id.clone_from(id);
                        }
                        if let Some(ref func) = tc_delta.function {
                            if let Some(ref name) = func.name {
                                tc.name.clone_from(name);
                            }
                            if let Some(ref args) = func.arguments {
                                tc.arguments.push_str(args);
                            }
                        }
                    }
                }
            }
        }
    }

    assert_eq!(tool_calls.len(), 1);
    assert_eq!(tool_calls[0].id, "call_1");
    assert_eq!(tool_calls[0].name, "get_weather");
    assert_eq!(tool_calls[0].arguments, r#"{"location": "Paris"}"#);
}

#[test]
fn test_json_value_to_arg_type() {
    let text = json_value_to_arg_type(&serde_json::json!("hello"));
    assert!(matches!(text, foundation_ai::types::base_types::ArgType::Text(s) if s == "hello"));

    let int = json_value_to_arg_type(&serde_json::json!(42));
    assert!(matches!(int, foundation_ai::types::base_types::ArgType::I64(42)));

    let float = json_value_to_arg_type(&serde_json::json!(3.64));
    assert!(
        matches!(float, foundation_ai::types::base_types::ArgType::Float64(f) if (f - 3.64).abs() < f64::EPSILON)
    );

    let obj = json_value_to_arg_type(&serde_json::json!({"nested": true}));
    assert!(matches!(obj, foundation_ai::types::base_types::ArgType::JSON(_)));
}

#[test]
fn test_parse_chat_response_tool_calls() {
    let response = ChatCompletionResponse {
        id: "chatcmpl-123".into(),
        object: "chat.completion".into(),
        created: 0,
        model: "gpt-4".into(),
        choices: vec![OpenAIChoice {
            index: 0,
            message: Some(OpenAIMessage {
                role: "assistant".into(),
                content: None,
                tool_calls: Some(vec![OpenAIToolCall {
                    id: "call_abc".into(),
                    tool_type: "function".into(),
                    function: OpenAIFunctionCall {
                        name: "get_weather".into(),
                        arguments: r#"{"location":"Paris"}"#.into(),
                    },
                }]),
                tool_call_id: None,
                refusal: None,
            }),
            finish_reason: Some("tool_calls".into()),
            logprobs: None,
        }],
        usage: Some(OpenAIUsage {
            prompt_tokens: 10,
            completion_tokens: 5,
            total_tokens: 15,
        }),
        system_fingerprint: None,
    };

    let model_id = ModelId::Name("gpt-4".into(), None);
    let (msg, _report) =
        parse_chat_response(&response, &model_id, &ModelUsageCosting::default()).unwrap();

    if let Messages::Assistant {
        content,
        stop_reason,
        ..
    } = &msg
    {
        assert_eq!(*stop_reason, StopReason::ToolUse);
        if let ModelOutput::ToolCall {
            id,
            name,
            arguments,
            ..
        } = content
        {
            assert_eq!(id, "call_abc");
            assert_eq!(name, "get_weather");
            let args = arguments.as_ref().expect("Should have arguments");
            assert!(args.contains_key("location"));
        } else {
            panic!("Expected ToolCall output");
        }
    } else {
        panic!("Expected Assistant message");
    }
}

#[test]
fn test_parse_chat_response_text() {
    let response = ChatCompletionResponse {
        id: "chatcmpl-456".into(),
        object: "chat.completion".into(),
        created: 0,
        model: "gpt-4".into(),
        choices: vec![OpenAIChoice {
            index: 0,
            message: Some(OpenAIMessage {
                role: "assistant".into(),
                content: Some(OpenAIMessageContent::Text("Hello!".into())),
                tool_calls: None,
                tool_call_id: None,
                refusal: None,
            }),
            finish_reason: Some("stop".into()),
            logprobs: None,
        }],
        usage: Some(OpenAIUsage {
            prompt_tokens: 5,
            completion_tokens: 2,
            total_tokens: 7,
        }),
        system_fingerprint: None,
    };

    let model_id = ModelId::Name("gpt-4".into(), None);
    let (msg, _report) =
        parse_chat_response(&response, &model_id, &ModelUsageCosting::default()).unwrap();

    if let Messages::Assistant {
        content,
        stop_reason,
        usage,
        metadata: _,
        ..
    } = &msg
    {
        assert_eq!(*stop_reason, StopReason::Stop);
        if let ModelOutput::Text(tc) = content {
            assert_eq!(tc.content, "Hello!");
        } else {
            panic!("Expected Text output");
        }
        assert!((usage.total_tokens - 7.0).abs() < f64::EPSILON);
    } else {
        panic!("Expected Assistant message");
    }
}

#[test]
fn test_parse_openai_error_format() {
    let body = r#"{"error":{"message":"Invalid API key","type":"invalid_request_error","code":"invalid_api_key"}}"#;
    let detail = parse_openai_error(body).expect("Should parse OpenAI error");
    assert_eq!(detail, "Invalid API key");
}

#[test]
fn test_parse_openai_error_plain_text_fallback() {
    let detail = parse_openai_error("Internal Server Error");
    assert!(detail.is_none());
}

#[test]
fn test_format_http_error_auth() {
    let msg = format_http_error(401, "Invalid API key");
    assert!(msg.contains("Authentication failed"), "got: {msg}");
    assert!(msg.contains("Invalid API key"), "got: {msg}");
}

#[test]
fn test_format_http_error_rate_limit() {
    let msg = format_http_error(429, "Rate limit reached");
    assert!(msg.contains("Rate limit exceeded"), "got: {msg}");
}

#[test]
fn test_format_http_error_server_error() {
    let msg = format_http_error(500, "Internal Server Error");
    assert!(msg.contains("Server error"), "got: {msg}");
    assert!(msg.contains("Internal Server Error"), "got: {msg}");
}

#[test]
fn test_is_retryable_status() {
    assert!(is_retryable_status(429));
    assert!(is_retryable_status(500));
    assert!(is_retryable_status(502));
    assert!(is_retryable_status(503));
    assert!(!is_retryable_status(400));
    assert!(!is_retryable_status(401));
    assert!(!is_retryable_status(404));
    assert!(!is_retryable_status(200));
}

#[test]
fn test_exponential_backoff() {
    assert_eq!(exponential_backoff(0), 1);
    assert_eq!(exponential_backoff(1), 2);
    assert_eq!(exponential_backoff(2), 4);
    assert_eq!(exponential_backoff(3), 8);
    assert_eq!(exponential_backoff(5), 30); // capped
    assert_eq!(exponential_backoff(10), 30); // capped
}

#[test]
fn test_response_format_serialization() {
    let fmt = OpenAIResponseFormat::Text;
    let json = serde_json::to_string(&fmt).unwrap();
    assert_eq!(json, r#"{"type":"text"}"#);

    let fmt = OpenAIResponseFormat::JsonObject;
    let json = serde_json::to_string(&fmt).unwrap();
    assert_eq!(json, r#"{"type":"json_object"}"#);

    let fmt = OpenAIResponseFormat::JsonSchema {
        json_schema: Some(OpenAIJsonSchema {
            name: "test_schema".into(),
            description: Some("A test schema".into()),
            schema: serde_json::json!({"type": "object", "properties": {"name": {"type": "string"}}}),
            strict: Some(true),
        }),
    };
    let json = serde_json::to_string(&fmt).unwrap();
    assert!(json.contains(r#""type":"json_schema""#));
    assert!(json.contains(r#""name":"test_schema""#));
    assert!(json.contains(r#""strict":true"#));
}

#[test]
fn test_tool_choice_serialization() {
    let tc = OpenAIToolChoice::Simple("auto".into());
    let json = serde_json::to_string(&tc).unwrap();
    assert_eq!(json, r#""auto""#);

    let tc = OpenAIToolChoice::Simple("none".into());
    let json = serde_json::to_string(&tc).unwrap();
    assert_eq!(json, r#""none""#);

    let tc = OpenAIToolChoice::Simple("required".into());
    let json = serde_json::to_string(&tc).unwrap();
    assert_eq!(json, r#""required""#);

    let tc = OpenAIToolChoice::Function {
        type_: "function".into(),
        function: OpenAIToolChoiceFunction {
            name: "get_weather".into(),
        },
    };
    let json = serde_json::to_string(&tc).unwrap();
    assert!(json.contains(r#""type":"function""#));
    assert!(json.contains(r#""name":"get_weather""#));
}

#[test]
fn test_build_chat_request_with_new_fields() {
    use foundation_ai::types::base_types::{
        OutputFormat, ToolChoice, ToolChoiceFunction, ToolFunctionRef,
    };

    let mut interaction = ModelInteraction {
        system_prompt: Some("You are helpful".into()),
        soul: Some("Be concise and technical".into()),
        messages: vec![foundation_ai::types::base_types::Messages::User {
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
        tool_choice: Some(ToolChoice::Auto),
    };
    let mut params = ModelParams::default();
    params.output_format = Some(OutputFormat::JsonObject);
    params.frequency_penalty = Some(0.5);
    params.presence_penalty = Some(0.3);
    let mut logit_bias = std::collections::HashMap::new();
    logit_bias.insert("50256".to_string(), -100.0);
    params.logit_bias = Some(logit_bias);

    let request = build_chat_request("gpt-4", &interaction, &params, false);

    assert!(request.response_format.is_some());
    let fmt = request.response_format.unwrap();
    assert!(serde_json::to_string(&fmt).unwrap().contains("json_object"));

    assert_eq!(request.frequency_penalty, Some(0.5));
    assert_eq!(request.presence_penalty, Some(0.3));
    assert!(request.logit_bias.is_some());
    assert!(request.tool_choice.is_some());

    // Now test forced function
    interaction.tool_choice = Some(ToolChoice::Function(ToolChoiceFunction {
        tool_type: "function".into(),
        function: ToolFunctionRef {
            name: "get_weather".into(),
        },
    }));
    let request2 = build_chat_request("gpt-4", &interaction, &params, false);
    let tc = request2.tool_choice.unwrap();
    let json = serde_json::to_string(&tc).unwrap();
    assert!(json.contains("get_weather"));
}

#[test]
fn test_multimodal_message_serialization() {
    let msg = OpenAIMessage {
        role: "user".into(),
        content: Some(OpenAIMessageContent::Parts(vec![
            OpenAIContentPart::Text {
                text: "What is this?".into(),
            },
            OpenAIContentPart::ImageUrl {
                image_url: OpenAIImageUrlObject {
                    url: "data:image/png;base64,iVBORw0KGgo".into(),
                    detail: Some("auto".into()),
                },
            },
        ])),
        tool_calls: None,
        tool_call_id: None,
        refusal: None,
    };

    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains(r#""type":"text""#));
    assert!(json.contains(r#""type":"image_url""#));
    assert!(json.contains("data:image/png;base64,"));
    assert!(json.contains(r#""detail":"auto""#));
}

#[test]
fn test_text_only_message_serialization() {
    let msg = OpenAIMessage {
        role: "user".into(),
        content: Some(OpenAIMessageContent::Text("Hello".into())),
        tool_calls: None,
        tool_call_id: None,
        refusal: None,
    };

    let json = serde_json::to_string(&msg).unwrap();
    // Text variant serializes as simple string, not object
    assert!(json.contains(r#""content":"Hello""#));
}
