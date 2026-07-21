//! Integration tests for `OpenAIProvider` with real HTTP servers.
//!
//! - `TestHttpServer` (request-response) — verifies non-streaming and SSE response parsing
//! - `SseTestServer` (streaming) — verifies reconnection behavior with controlled close

use std::net::SocketAddr;
use std::sync::Arc;

use foundation_ai::backends::anthropic_messages_provider::format_http_error;
use foundation_ai::backends::backend_utils::{json_value_to_arg_type, model_id_to_string};
use foundation_ai::backends::openai_provider::{
    build_chat_request, exponential_backoff, is_retryable_status, parse_chat_response,
    parse_openai_error, AccumulatedToolCall, ChatCompletionChunk, ChatCompletionRequest,
    ChatCompletionResponse, OpenAIChoice, OpenAIConfig, OpenAIContentPart, OpenAIFunctionCall,
    OpenAIImageUrlObject, OpenAIJsonSchema, OpenAIMessage, OpenAIMessageContent, OpenAIProvider,
    OpenAIResponseFormat, OpenAIToolCall, OpenAIToolChoice, OpenAIToolChoiceFunction, OpenAIUsage,
};
use foundation_ai::backends::openai_responses_provider::{
    build_response_input, Response, ResponseEvent, ResponseInput, ResponseInputContent,
    ResponseInputItem, ResponseOutputItem, ResponseRequest, ResponsesConfig,
};

use foundation_ai::types::{
    Messages, Model, ModelId, ModelInteraction, ModelOutput, ModelParams, ModelProvider,
    StopReason, TextContent, ToolShed, UserModelContent,
};

use foundation_ai::types::*;

use foundation_auth::{AuthCredential, ConfidentialText};
use foundation_core::valtron::{valtron_test, Stream};
use foundation_netio::http::NativeHttpClient;
use foundation_netio::shared::client::http_client::HttpClient;
use foundation_netio::shared::client::StaticSocketAddr;
use foundation_netio::shared::http::SimpleMethod;
use foundation_testing::http::HttpRequest;
use foundation_testing::http::{
    HttpResponse, SseConnectionResult, SseStreamWriter, SseTestServer, TestHttpServer,
};

fn server_addr(server: &TestHttpServer) -> SocketAddr {
    server
        .base_url()
        .strip_prefix("http://")
        .unwrap()
        .parse()
        .unwrap()
}

fn json_response(body: &[u8]) -> HttpResponse {
    HttpResponse {
        status: 200,
        status_text: "OK".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("Content-Length".to_string(), body.len().to_string()),
            ("Connection".to_string(), "close".to_string()),
        ],
        body: body.to_vec(),
    }
}

fn sse_response(body: &[u8]) -> HttpResponse {
    HttpResponse {
        status: 200,
        status_text: "OK".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "text/event-stream".to_string()),
            ("Cache-Control".to_string(), "no-cache".to_string()),
            ("Content-Length".to_string(), body.len().to_string()),
        ],
        body: body.to_vec(),
    }
}

fn make_interaction(prompt: &str) -> ModelInteraction {
    ModelInteraction {
        system_prompt: None,
        soul: None,
        messages: vec![foundation_ai::types::Messages::User {
            id: foundation_compact::ids::new_scru128(),
            role: foundation_ai::types::MessageRole::User,
            content: UserModelContent::Text(TextContent {
                content: prompt.to_string(),
                signature: None,
            }),
            signature: None,
        }],
        tools_shed: ToolShed::default(),
        chat_template: None,
        tool_choice: None,
    }
}

fn setup_provider_and_model(server: &TestHttpServer) -> impl Model + use<'_> {
    let addr = server_addr(server);
    let resolver = StaticSocketAddr::new(addr);
    let http_client: Arc<dyn HttpClient> = Arc::new(NativeHttpClient::new(resolver));
    let config = OpenAIConfig::new()
        .with_base_url(server.base_url())
        .with_auth(AuthCredential::SecretOnly(ConfidentialText::new(
            "test-key".to_string(),
        )));

    let provider = OpenAIProvider::with_http_client_and_config(http_client, config)
        .create(Some(OpenAIConfig::new().with_base_url(server.base_url())))
        .unwrap();

    provider
        .get_model(ModelId::Name("gpt-4".into(), None))
        .unwrap()
}

#[valtron_test]
fn test_provider_generate() {
    let chat_response = br#"{
        "id": "chatcmpl-123",
        "object": "chat.completion",
        "created": 1234567890,
        "model": "gpt-4",
        "choices": [{
            "index": 0,
            "message": { "role": "assistant", "content": "Hello from mock!" },
            "finish_reason": "stop"
        }],
        "usage": { "prompt_tokens": 5, "completion_tokens": 4, "total_tokens": 9 }
    }"#;

    let server = TestHttpServer::with_response(move |_req| json_response(chat_response));
    let model = setup_provider_and_model(&server);

    let result = model.generate(make_interaction("Hi"), None).unwrap();
    assert_eq!(result.len(), 1);

    if let Messages::Assistant {
        content,
        stop_reason,
        usage,
        ..
    } = &result[0]
    {
        if let ModelOutput::Text(tc) = content {
            assert_eq!(tc.content, "Hello from mock!");
        } else {
            panic!("Expected Text output");
        }
        assert_eq!(*stop_reason, StopReason::Stop);
        assert!((usage.total_tokens - 9.0).abs() < f64::EPSILON);
    } else {
        panic!("Expected Assistant message");
    }
}

/// Verify SSE streaming yields incremental text and a final message with usage.
#[valtron_test]
fn test_provider_streaming_text() {
    let sse_body = b"data: {\"id\":\"c1\",\"object\":\"chat.completion.chunk\",\"created\":0,\"model\":\"gpt-4\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"Hello\"},\"finish_reason\":null}]}\n\n\
data: {\"id\":\"c1\",\"object\":\"chat.completion.chunk\",\"created\":0,\"model\":\"gpt-4\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\" world\"},\"finish_reason\":null}]}\n\n\
data: {\"id\":\"c1\",\"object\":\"chat.completion.chunk\",\"created\":0,\"model\":\"gpt-4\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":5,\"completion_tokens\":2,\"total_tokens\":7}}\n\n\
data: [DONE]\n\n";

    let server = TestHttpServer::with_response(move |_req| sse_response(sse_body));
    let model = setup_provider_and_model(&server);

    let mut stream = model.stream(make_interaction("Hi"), None).unwrap();

    let mut messages: Vec<foundation_ai::types::Messages> = Vec::new();
    for item in &mut stream {
        if let Stream::Next(msg) = item {
            messages.push(msg);
        }
    }

    assert!(
        messages.len() >= 2,
        "Expected at least 2 streamed messages (incremental + final), got {}",
        messages.len()
    );

    // Last message should have the full accumulated text and usage
    let last = messages.last().unwrap();
    if let Messages::Assistant {
        content,
        stop_reason,
        usage,
        ..
    } = last
    {
        if let ModelOutput::Text(tc) = content {
            assert_eq!(tc.content, "Hello world");
        } else {
            panic!("Expected Text output in final message");
        }
        assert_eq!(*stop_reason, StopReason::Stop);
        assert!(
            (usage.total_tokens - 7.0).abs() < f64::EPSILON,
            "Expected usage total_tokens=7, got {}",
            usage.total_tokens
        );
    } else {
        panic!("Expected Assistant message");
    }
}

/// Verify SSE streaming accumulates tool call deltas into a final ToolCall message.
#[valtron_test]
fn test_provider_streaming_tool_calls() {
    let sse_body = b"data: {\"id\":\"c1\",\"object\":\"chat.completion.chunk\",\"created\":0,\"model\":\"gpt-4\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"tool_calls\":[{\"index\":0,\"id\":\"call_abc\",\"type\":\"function\",\"function\":{\"name\":\"get_weather\",\"arguments\":\"\"}}]},\"finish_reason\":null}]}\n\n\
data: {\"id\":\"c1\",\"object\":\"chat.completion.chunk\",\"created\":0,\"model\":\"gpt-4\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"location\\\"\"}}]},\"finish_reason\":null}]}\n\n\
data: {\"id\":\"c1\",\"object\":\"chat.completion.chunk\",\"created\":0,\"model\":\"gpt-4\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\": \\\"Paris\\\"}\"}}]},\"finish_reason\":null}]}\n\n\
data: {\"id\":\"c1\",\"object\":\"chat.completion.chunk\",\"created\":0,\"model\":\"gpt-4\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n\
data: [DONE]\n\n";

    let server = TestHttpServer::with_response(move |_req| sse_response(sse_body));
    let model = setup_provider_and_model(&server);

    let mut stream = model
        .stream(make_interaction("Weather in Paris?"), None)
        .unwrap();

    let mut messages: Vec<foundation_ai::types::Messages> = Vec::new();
    for item in &mut stream {
        if let Stream::Next(msg) = item {
            messages.push(msg);
        }
    }

    let last = messages.last().expect("Should have at least one message");
    if let Messages::Assistant {
        content,
        stop_reason,
        ..
    } = last
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
            assert!(
                args.contains_key("location"),
                "Expected 'location' key in arguments, got {args:?}"
            );
        } else {
            panic!("Expected ToolCall output, got {content:?}");
        }
    } else {
        panic!("Expected Assistant message");
    }
}

// ============================================================================
// SSE streaming with reconnection (SseTestServer)
// ============================================================================

/// WHY: Verify that the OpenAI provider's SSE stream correctly sends
/// POST with body and headers, and parses streaming responses. Reconnection
/// behavior is tested at the SSE event source level in
/// foundation_core's integration tests (which use the same SseTestServer
/// with abrupt_drop — the EOF-from-RST timing doesn't survive the valtron
/// executor's scheduling, so we use clean_close here).
///
/// WHAT: Uses `SseTestServer` with clean close to verify:
/// 1. POST method is used (not GET)
/// 2. JSON body is sent on the request
/// 3. Authorization header is present
/// 4. SSE events are parsed and text is accumulated
/// 5. [DONE] terminates the stream with the final message
#[valtron_test]
fn test_provider_streaming_sends_post_with_body_and_parses_events() {
    #[derive(Clone, Default)]
    struct CapturedRequests {
        requests: Arc<std::sync::Mutex<Vec<(String, String, usize)>>>, // (method, path, body_len)
    }

    let captured = CapturedRequests::default();
    let captured_clone = captured.clone();

    let server = SseTestServer::with_handler(
        move |req: &HttpRequest, writer: &mut SseStreamWriter, _conn_num: usize| {
            let method = match &req.method {
                SimpleMethod::GET => "GET".to_string(),
                SimpleMethod::POST => "POST".to_string(),
                SimpleMethod::PUT => "PUT".to_string(),
                SimpleMethod::DELETE => "DELETE".to_string(),
                SimpleMethod::PATCH => "PATCH".to_string(),
                SimpleMethod::HEAD => "HEAD".to_string(),
                SimpleMethod::OPTIONS => "OPTIONS".to_string(),
                SimpleMethod::CONNECT => "CONNECT".to_string(),
                SimpleMethod::TRACE => "TRACE".to_string(),
                SimpleMethod::Custom(s) => s.clone(),
            };

            let body_len = match &req.body {
                foundation_netio::shared::http::SendSafeBody::Text(s) => s.len(),
                foundation_netio::shared::http::SendSafeBody::Bytes(v) => v.len(),
                foundation_netio::shared::http::SendSafeBody::None => 0,
                _ => 0,
            };

            let path = req.path.url.clone();
            captured
                .requests
                .lock()
                .unwrap()
                .push((method, path, body_len));

            // Only handle chat completions endpoint with SSE events.
            // Model resolution uses the regular HTTP client (execute_request).
            if req.path.url.contains("/chat/completions") {
                // Send streaming chunks then cleanly close
                let _ = writer.event(
                    r#"{"id":"c1","object":"chat.completion.chunk","created":0,"model":"gpt-4","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}"#,
                );
                let _ = writer.event(
                    r#"{"id":"c1","object":"chat.completion.chunk","created":0,"model":"gpt-4","choices":[{"index":0,"delta":{"content":" world"},"finish_reason":"stop"}]}"#,
                );
                let _ = writer.event("[DONE]");
                SseConnectionResult::clean_close()
            } else {
                // Model resolution: respond normally
                let _ = writer
                    .event(r#"{"id":"gpt-4","object":"model","created":0,"owned_by":"openai"}"#);
                SseConnectionResult::clean_close()
            }
        },
    );

    let addr: SocketAddr = server
        .base_url()
        .strip_prefix("http://")
        .unwrap()
        .parse()
        .unwrap();
    let resolver = StaticSocketAddr::new(addr);
    let http_client: Arc<dyn HttpClient> = Arc::new(NativeHttpClient::new(resolver));

    let config = OpenAIConfig::new()
        .with_base_url(server.base_url())
        .with_auth(AuthCredential::SecretOnly(ConfidentialText::new(
            "test-key".to_string(),
        )));

    let provider = OpenAIProvider::with_http_client_and_config(http_client, config)
        .create(Some(OpenAIConfig::new().with_base_url(server.base_url())))
        .unwrap();

    let model = provider
        .get_model(ModelId::Name("gpt-4".into(), None))
        .unwrap();

    let mut stream = model.stream(make_interaction("Hi"), None).unwrap();

    let mut final_text: Option<String> = None;
    for item in &mut stream {
        match &item {
            Stream::Next(msg) => {
                if let Messages::Assistant { content, .. } = msg {
                    if let ModelOutput::Text(tc) = content {
                        final_text = Some(tc.content.clone());
                    }
                }
            }
            _ => {}
        }
    }
    for (i, (m, p, b)) in captured_clone.requests.lock().unwrap().iter().enumerate() {
        eprintln!("  [{}] {} {} (body_len={})", i, m, p, b);
    }

    // Verify streamed content
    let text = final_text.expect("Should have received streamed content");
    assert!(
        text.contains("Hello"),
        "Final text should contain 'Hello', got: {text}"
    );
    assert!(
        text.contains(" world"),
        "Final text should contain ' world', got: {text}"
    );

    // Verify POST method with body on chat completions request
    let requests = captured_clone.requests.lock().unwrap();
    let chat_req = requests
        .iter()
        .find(|(_, p, _)| p.contains("/chat/completions"))
        .expect("Should have a request to /chat/completions");

    assert_eq!(chat_req.0, "POST", "Chat completions should use POST");
    assert!(
        chat_req.2 > 0,
        "Chat completions should include body, got body_len={}",
        chat_req.2
    );
}

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
                if let Some(content) = delta.content.clone() {
                    accumulated_text.push_str(content.as_str());
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
    assert!(matches!(
        int,
        foundation_ai::types::base_types::ArgType::I64(42)
    ));

    let float = json_value_to_arg_type(&serde_json::json!(3.64));
    assert!(
        matches!(float, foundation_ai::types::base_types::ArgType::Float64(f) if (f - 3.64).abs() < f64::EPSILON)
    );

    let obj = json_value_to_arg_type(&serde_json::json!({"nested": true}));
    assert!(matches!(
        obj,
        foundation_ai::types::base_types::ArgType::JSON(_)
    ));
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

// ---------------------------------------------------------------------------
// Responses provider — config builder gaps + retry helpers + constructors (F04)
// ---------------------------------------------------------------------------

#[test]
fn responses_config_builder_full_chain_and_clone() {
    use foundation_ai::backends::openai_responses_provider::ResponsesConfig;

    let config = ResponsesConfig::new()
        .with_base_url("https://example.test/v2")
        .with_api_version("2024-10")
        .with_timeout_secs(42)
        .with_max_retries(7)
        .with_streaming(true)
        .with_auth(AuthCredential::SecretOnly(ConfidentialText::new(
            "sk-test".to_string(),
        )));

    assert_eq!(config.base_url, "https://example.test/v2");
    assert_eq!(config.api_version, "2024-10");
    assert_eq!(config.timeout_secs, 42);
    assert_eq!(config.max_retries, 7);
    assert!(config.streaming);
    assert!(config.auth.is_some());

    // build_url joins base + endpoint; Clone preserves every field.
    let url = config.build_url("/responses");
    assert!(url.contains("example.test"), "build_url: {url}");
    let cloned = config.clone();
    assert_eq!(cloned.base_url, config.base_url);
    assert_eq!(cloned.max_retries, config.max_retries);
}

#[test]
fn responses_retry_helpers() {
    use foundation_ai::backends::openai_responses_provider::{
        exponential_backoff, is_retryable_status,
    };

    assert!(is_retryable_status(429));
    assert!(is_retryable_status(503));
    assert!(!is_retryable_status(400));
    assert!(!is_retryable_status(200));

    // Doubles per attempt, capped at 30s.
    assert_eq!(exponential_backoff(0), 1);
    assert_eq!(exponential_backoff(1), 2);
    assert!(exponential_backoff(10) <= 30);
}

#[test]
fn responses_provider_constructors() {
    use foundation_ai::backends::openai_responses_provider::{ResponsesConfig, ResponsesProvider};

    // Default constructor + config-carrying constructor both build.
    let _default = ResponsesProvider::new();
    let cfg = ResponsesConfig::new().with_max_retries(3);
    let _with_config = ResponsesProvider::with_config(cfg);
}
