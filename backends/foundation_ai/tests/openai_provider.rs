//! Integration tests for `OpenAIProvider` with a mock HTTP server.
//!
//! Tests the full provider lifecycle: config, model creation, streaming,
//! and tool call handling against a local `TestHttpServer`.

use foundation_ai::backends::openai_provider::{OpenAIConfig, OpenAIProvider};
use foundation_ai::types::{
    Model, ModelId, ModelInteraction, ModelOutput, ModelProvider, StopReason, TextContent,
    UserModelContent,
};
use foundation_auth::{AuthCredential, ConfidentialText};
use foundation_core::valtron;
use foundation_core::valtron::Stream;
use foundation_core::wire::simple_http::client::StaticSocketAddr;
use foundation_testing::http::{HttpResponse, TestHttpServer};
use std::net::SocketAddr;

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
        messages: vec![foundation_ai::types::Messages::User {
            role: String::from("user"),
            content: UserModelContent::Text(TextContent {
                content: prompt.to_string(),
                signature: None,
            }),
            signature: None,
        }],
        tools: vec![],
        chat_template: None,
    }
}

fn setup_provider_and_model(server: &TestHttpServer) -> impl Model + use<'_> {
    let addr = server_addr(server);
    let resolver = StaticSocketAddr::new(addr);
    let config = OpenAIConfig::new().with_base_url(server.base_url());

    let provider = OpenAIProvider::with_resolver_and_config(resolver, config)
        .create(
            Some(OpenAIConfig::new().with_base_url(server.base_url())),
            Some(AuthCredential::SecretOnly(ConfidentialText::new(
                "test-key".to_string(),
            ))),
        )
        .unwrap();

    provider
        .get_model(ModelId::Name("gpt-4".into(), None))
        .unwrap()
}

#[test]
#[tracing_test::traced_test]
fn test_provider_generate() {
    let _guard = valtron::initialize_pool(42, Some(4));

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

    if let foundation_ai::types::Messages::Assistant {
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
#[test]
fn test_provider_streaming_text() {
    let _guard = valtron::initialize_pool(42, Some(4));
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
    if let foundation_ai::types::Messages::Assistant {
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
#[test]
fn test_provider_streaming_tool_calls() {
    let _guard = valtron::initialize_pool(42, Some(4));
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
    if let foundation_ai::types::Messages::Assistant {
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
