//! Integration tests for `AnthropicMessagesProvider` with real HTTP servers.
//!
//! - `TestHttpServer` (request-response) — verifies non-streaming and SSE response parsing
//! - llama-server integration tests (#[ignore]-gated) — verifies against real llama-server
//!   with Anthropic Messages API compatibility

use foundation_ai::backends::anthropic_messages_provider::{
    AnthropicConfig, AnthropicMessagesProvider,
};
use foundation_ai::types::{
    Args, CostStatus, ImageContent, Messages, MimeType, Model, ModelId, ModelInteraction, ModelOutput, ModelParams, ModelProvider,
    ModelProviders, ModelUsageCosting, StopReason, TextContent, Tool, ToolShed, UsageCosting, UsageReport, UserModelContent,
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
        soul: None,
        messages: vec![Messages::User {
            role: String::from("user"),
            content: UserModelContent::Text(TextContent {
                content: prompt.to_string(),
                signature: None,
            }),
            signature: None,
        }],
        tools_shed: None,
        chat_template: None,
        tool_choice: None,
    }
}

fn setup_provider_and_model(server: &TestHttpServer) -> impl Model + use<'_> {
    let addr = server_addr(server);
    let resolver = StaticSocketAddr::new(addr);
    let config = AnthropicConfig::new()
        .with_base_url(server.base_url())
        .with_auth(AuthCredential::SecretOnly(ConfidentialText::new(
            "test-key".to_string(),
        )));

    let provider = AnthropicMessagesProvider::with_resolver_and_config(resolver, config.clone())
        .create(Some(config))
        .unwrap();

    provider
        .get_model(ModelId::Name("claude-3-5-sonnet".into(), None))
        .unwrap()
}

#[test]
#[tracing_test::traced_test]
fn test_provider_generate() {
    let _guard = valtron::initialize_pool(42, Some(4));

    let chat_response = br#"{
        "id": "msg_abc123",
        "type": "message",
        "role": "assistant",
        "content": [{"type": "text", "text": "Hello from mock!"}],
        "model": "claude-3-5-sonnet-20241022",
        "stop_reason": "end_turn",
        "stop_sequence": null,
        "usage": {"input_tokens": 10, "output_tokens": 5, "cache_creation_input_tokens": 0, "cache_read_input_tokens": 0}
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
        assert!((usage.total_tokens - 15.0).abs() < f64::EPSILON);
    } else {
        panic!("Expected Assistant message");
    }
}

/// Verify SSE streaming yields incremental text via named events.
#[test]
#[tracing_test::traced_test]
fn test_provider_streaming_text() {
    let _guard = valtron::initialize_pool(42, Some(4));

    let sse_body = b"event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_123\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[],\"model\":\"claude-3-5-sonnet\",\"stop_reason\":null,\"stop_sequence\":null,\"usage\":{\"input_tokens\":10,\"output_tokens\":0,\"cache_creation_input_tokens\":0,\"cache_read_input_tokens\":0}}}\n\n\
event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n\
event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}\n\n\
event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\" world\"}}\n\n\
event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n\
event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"input_tokens\":10,\"output_tokens\":2,\"cache_creation_input_tokens\":0,\"cache_read_input_tokens\":0}}\n\n\
event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";

    let server = TestHttpServer::with_response(move |_req| sse_response(sse_body));
    let model = setup_provider_and_model(&server);

    let mut stream = model.stream(make_interaction("Hi"), None).unwrap();

    let mut messages: Vec<Messages> = Vec::new();
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

    // Last message should have the full accumulated text
    let last = messages.last().unwrap();
    if let Messages::Assistant {
        content,
        stop_reason,
        ..
    } = last
    {
        if let ModelOutput::Text(tc) = content {
            assert_eq!(tc.content, "Hello world");
        } else {
            panic!("Expected Text output in final message");
        }
        assert_eq!(*stop_reason, StopReason::Stop);
    } else {
        panic!("Expected Assistant message");
    }
}

/// Verify SSE streaming accumulates tool call deltas into a final ToolCall message.
#[test]
#[tracing_test::traced_test]
fn test_provider_streaming_tool_calls() {
    let _guard = valtron::initialize_pool(42, Some(4));

    let sse_body = b"event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_123\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[],\"model\":\"claude-3-5-sonnet\",\"stop_reason\":null,\"stop_sequence\":null,\"usage\":{\"input_tokens\":50,\"output_tokens\":0,\"cache_creation_input_tokens\":0,\"cache_read_input_tokens\":0}}}\n\n\
event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"tool_use\",\"id\":\"tool_abc\",\"name\":\"get_weather\",\"input\":{}}}\n\n\
event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"location\\\": \\\"Paris\\\"}\"}}\n\n\
event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n\
event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"tool_use\"},\"usage\":{\"input_tokens\":50,\"output_tokens\":10,\"cache_creation_input_tokens\":0,\"cache_read_input_tokens\":0}}\n\n\
event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";

    let server = TestHttpServer::with_response(move |_req| sse_response(sse_body));
    let model = setup_provider_and_model(&server);

    let mut stream = model
        .stream(make_interaction("Weather in Paris?"), None)
        .unwrap();

    let mut messages: Vec<Messages> = Vec::new();
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
            assert_eq!(id, "tool_abc");
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

/// Verify extended thinking response parsing.
#[test]
#[tracing_test::traced_test]
fn test_provider_generate_with_thinking() {
    let _guard = valtron::initialize_pool(42, Some(4));

    let chat_response = br#"{
        "id": "msg_abc123",
        "type": "message",
        "role": "assistant",
        "content": [
            {"type": "thinking", "thinking": "Let me think about this...", "signature": "sig_abc"},
            {"type": "text", "text": "The answer is 42."}
        ],
        "model": "claude-3-7-sonnet-20250219",
        "stop_reason": "end_turn",
        "stop_sequence": null,
        "usage": {"input_tokens": 20, "output_tokens": 50, "cache_creation_input_tokens": 0, "cache_read_input_tokens": 0}
    }"#;

    let server = TestHttpServer::with_response(move |_req| json_response(chat_response));
    let model = setup_provider_and_model(&server);

    let result = model.generate(make_interaction("What is 6 times 7?"), None).unwrap();
    assert_eq!(result.len(), 2);

    // First message: thinking block
    if let Messages::Assistant { content, .. } = &result[0] {
        if let ModelOutput::ThinkingContent { thinking, signature } = content {
            assert_eq!(thinking, "Let me think about this...");
            assert_eq!(signature.as_deref(), Some("sig_abc"));
        } else {
            panic!("Expected ThinkingContent, got {content:?}");
        }
    } else {
        panic!("Expected Assistant message");
    }

    // Second message: text answer
    if let Messages::Assistant { content, .. } = &result[1] {
        if let ModelOutput::Text(tc) = content {
            assert_eq!(tc.content, "The answer is 42.");
        } else {
            panic!("Expected Text output, got {content:?}");
        }
    } else {
        panic!("Expected Assistant message");
    }
}

/// Verify multimodal request with image content block.
#[test]
#[tracing_test::traced_test]
fn test_provider_generate_multimodal() {
    let _guard = valtron::initialize_pool(42, Some(4));

    let chat_response = br#"{
        "id": "msg_abc123",
        "type": "message",
        "role": "assistant",
        "content": [{"type": "text", "text": "I see a cat in this image."}],
        "model": "claude-3-5-sonnet-20241022",
        "stop_reason": "end_turn",
        "stop_sequence": null,
        "usage": {"input_tokens": 100, "output_tokens": 10, "cache_creation_input_tokens": 0, "cache_read_input_tokens": 0}
    }"#;

    let server = TestHttpServer::with_response(move |_req| json_response(chat_response));
    let model = setup_provider_and_model(&server);

    use foundation_ai::types::{ImageContent, MimeType};

    let interaction = ModelInteraction {
        system_prompt: None,
        soul: None,
        messages: vec![Messages::User {
            role: "user".into(),
            content: UserModelContent::Image(ImageContent {
                b64: "base64data".into(),
                mime_type: MimeType::ImagePng,
            }),
            signature: None,
        }],
        tools_shed: None,
        chat_template: None,
        tool_choice: None,
    };

    let result = model.generate(interaction, None).unwrap();
    assert_eq!(result.len(), 1);

    if let Messages::Assistant { content, .. } = &result[0] {
        if let ModelOutput::Text(tc) = content {
            assert_eq!(tc.content, "I see a cat in this image.");
        } else {
            panic!("Expected Text output");
        }
    } else {
        panic!("Expected Assistant message");
    }
}

// ============================================================================
// llama-server integration tests (#[ignore]-gated)
// ============================================================================
//
// llama-server exposes an Anthropic-compatible endpoint at /v1/messages.
// These tests verify the provider works against a real llama-server instance.
//
// Requirements:
//   1. llama-server built and running: `mise run llama:server:start`
//   2. Environment: `LLAMA_SERVER_TEST=1`

fn setup_llama_server_provider() -> impl Model {
    let base_url =
        std::env::var("LLAMA_SERVER_URL").unwrap_or_else(|_| "http://127.0.0.1:8999".into());

    // Matches the llama-server --api-key flag in mise configuration.
    let api_key = "test-api-key-123";

    let config = AnthropicConfig::new()
        .with_base_url(base_url)
        .with_messages_endpoint("/v1/messages")
        .with_auth(AuthCredential::SecretOnly(ConfidentialText::new(api_key.to_string())));

    let provider = AnthropicMessagesProvider::new().create(Some(config)).unwrap();

    provider
        .get_model(ModelId::Name("claude-3-5-sonnet".into(), None))
        .unwrap()
}

/// Test: generate a short response against the real llama-server.
#[test]
#[tracing_test::traced_test]
#[ignore]
fn test_llama_server_anthropic_generate() {
    let _guard = valtron::initialize_pool(42, Some(4));
    let model = setup_llama_server_provider();

    let interaction = ModelInteraction {
        system_prompt: Some("You are a helpful assistant.".into()),
        soul: None,
        messages: vec![Messages::User {
            role: "user".into(),
            content: UserModelContent::Text(TextContent {
                content: "Say hello in one word.".into(),
                signature: None,
            }),
            signature: None,
        }],
        tools_shed: None,
        chat_template: None,
        tool_choice: None,
    };

    let result = model.generate(interaction, None).unwrap();
    assert!(!result.is_empty());

    if let Messages::Assistant {
        content,
        stop_reason,
        ..
    } = &result[0]
    {
        if let ModelOutput::Text(tc) = content {
            assert!(!tc.content.is_empty(), "Response should not be empty");
        } else {
            panic!("Expected Text output, got {content:?}");
        }
        assert!(
            matches!(stop_reason, StopReason::Stop | StopReason::Length),
            "Expected Stop or Length, got {stop_reason:?}"
        );
    } else {
        panic!("Expected Assistant message");
    }
}

/// Test: streaming text generation against the real llama-server.
#[test]
#[tracing_test::traced_test]
#[ignore]
fn test_llama_server_anthropic_streaming() {
    let _guard = valtron::initialize_pool(42, Some(4));
    let model = setup_llama_server_provider();

    let interaction = ModelInteraction {
        system_prompt: None,
        soul: None,
        messages: vec![Messages::User {
            role: "user".into(),
            content: UserModelContent::Text(TextContent {
                content: "Count from 1 to 3, one number per line.".into(),
                signature: None,
            }),
            signature: None,
        }],
        tools_shed: None,
        chat_template: None,
        tool_choice: None,
    };

    let mut stream = model.stream(interaction, None).unwrap();
    let mut total_chars = 0;
    for item in &mut stream {
        if let Stream::Next(msg) = item {
            if let Messages::Assistant { content, .. } = &msg {
                if let ModelOutput::Text(tc) = content {
                    total_chars += tc.content.len();
                }
            }
        }
    }

    assert!(total_chars > 0, "Should have received streamed tokens");
}

/// Test: multi-turn conversation with conversation history.
#[test]
#[tracing_test::traced_test]
#[ignore]
fn test_llama_server_anthropic_multi_turn() {
    let _guard = valtron::initialize_pool(42, Some(4));
    let model = setup_llama_server_provider();

    let interaction = ModelInteraction {
        system_prompt: None,
        soul: None,
        messages: vec![
            Messages::User {
                role: "user".into(),
                content: UserModelContent::Text(TextContent {
                    content: "What is the capital of France?".into(),
                    signature: None,
                }),
                signature: None,
            },
            Messages::Assistant {
                model: ModelId::Name("claude-3-5-sonnet".into(), None),
                timestamp: std::time::SystemTime::now(),
                content: ModelOutput::Text(TextContent {
                    content: "The capital of France is Paris.".into(),
                    signature: None,
                }),
                stop_reason: StopReason::Stop,
                usage: UsageReport {
                    input: 0.0,
                    output: 0.0,
                    cache_read: 0.0,
                    cache_write: 0.0,
                    total_tokens: 0.0,
                    cost: UsageCosting {
                        currency: String::new(),
                        input: 0.0,
                        output: 0.0,
                        cache_read: 0.0,
                        cache_write: 0.0,
                        total_tokens: 0.0,
                        status: CostStatus::Estimated,
                    },
                },
                provider: ModelProviders::ANTHROPIC,
                error_detail: None,
                signature: None,
                metadata: None,
            },
            Messages::User {
                role: "user".into(),
                content: UserModelContent::Text(TextContent {
                    content: "What about Spain?".into(),
                    signature: None,
                }),
                signature: None,
            },
        ],
        tools_shed: None,
        chat_template: None,
        tool_choice: None,
    };

    let result = model.generate(interaction, None).unwrap();
    assert!(!result.is_empty());

    if let Messages::Assistant { content, .. } = &result[0] {
        if let ModelOutput::Text(tc) = content {
            assert!(
                tc.content.to_lowercase().contains("madrid"),
                "Expected Madrid in response, got: {}",
                tc.content
            );
        } else {
            panic!("Expected Text output");
        }
    }
}

/// Test: max_tokens constraint truncates output.
#[test]
#[tracing_test::traced_test]
#[ignore]
fn test_llama_server_anthropic_max_tokens() {
    let _guard = valtron::initialize_pool(42, Some(4));
    let model = setup_llama_server_provider();

    let interaction = ModelInteraction {
        system_prompt: None,
        soul: None,
        messages: vec![Messages::User {
            role: "user".into(),
            content: UserModelContent::Text(TextContent {
                content: "Write a long essay about the history of computing.".into(),
                signature: None,
            }),
            signature: None,
        }],
        tools_shed: None,
        chat_template: None,
        tool_choice: None,
    };

    let mut params = ModelParams::default();
    params.max_tokens = 20;

    let result = model.generate(interaction, Some(params)).unwrap();
    assert!(!result.is_empty());

    if let Messages::Assistant { stop_reason, .. } = &result[0] {
        assert_eq!(
            *stop_reason,
            StopReason::Length,
            "Expected Length stop with max_tokens=20, got {stop_reason:?}"
        );
    }
}

/// Test: provider can resolve and connect to the running llama-server.
#[test]
#[tracing_test::traced_test]
#[ignore]
fn test_llama_server_anthropic_resolve_model() {
    let _guard = valtron::initialize_pool(42, Some(4));
    let model = setup_llama_server_provider();

    let interaction = ModelInteraction {
        system_prompt: None,
        soul: None,
        messages: vec![Messages::User {
            role: "user".into(),
            content: UserModelContent::Text(TextContent {
                content: "Hi".into(),
                signature: None,
            }),
            signature: None,
        }],
        tools_shed: None,
        chat_template: None,
        tool_choice: None,
    };

    let result = model.generate(interaction, None);
    assert!(
        result.is_ok(),
        "Expected provider to connect and respond, got error: {:?}",
        result.err()
    );
}

// ============================================================================
// Unit tests
// ============================================================================

use foundation_ai::backends::anthropic_messages_provider::{
    AnthropicContentBlock, AnthropicDelta, AnthropicImageSource,
    AnthropicMessage, AnthropicRole, AnthropicSystemBlock, AnthropicSystemContent,
    AnthropicTool, AnthropicToolChoice, AnthropicUsage,
    MessagesRequest, MessagesResponse, StreamEvent,
    build_anthropic_request, empty_usage_report, exponential_backoff, format_http_error,
    is_retryable_status, map_stop_reason, parse_anthropic_error, parse_response,
};
use std::time::SystemTime;

// --- Config tests ---

#[test]
fn test_anthropic_config_defaults() {
    let config = AnthropicConfig::default();
    assert_eq!(config.base_url, "https://api.anthropic.com");
    assert_eq!(config.api_version, "2023-06-01");
    assert_eq!(config.timeout_secs, 120);
    assert_eq!(config.max_retries, 3);
    assert!(config.proxy_url.is_none());
    assert!(config.streaming);
}

#[test]
fn test_anthropic_config_builder() {
    let config = AnthropicConfig::new()
        .with_base_url("http://localhost:8080")
        .with_api_version("2024-01-01")
        .with_timeout_secs(60)
        .with_streaming(false);
    assert_eq!(config.base_url, "http://localhost:8080");
    assert_eq!(config.api_version, "2024-01-01");
    assert_eq!(config.timeout_secs, 60);
    assert!(!config.streaming);
}

// --- Request/Response serialization tests ---

#[test]
fn test_messages_request_serialization() {
    let request = MessagesRequest {
        model: "claude-3-5-sonnet-20241022".into(),
        system: Some(AnthropicSystemContent::Text("Be helpful".into())),
        messages: vec![AnthropicMessage {
            role: AnthropicRole::User,
            content: vec![AnthropicContentBlock::Text {
                text: "Hello".into(),
            }],
        }],
        max_tokens: 1024,
        temperature: Some(0.7),
        top_p: Some(0.9),
        top_k: None,
        stream: Some(false),
        stop_sequences: None,
        tools: None,
        tool_choice: None,
        thinking: None,
    };

    let json = serde_json::to_string(&request).unwrap();
    assert!(json.contains(r#""model":"claude-3-5-sonnet-20241022""#));
    assert!(json.contains(r#""max_tokens":1024"#));
    assert!(json.contains(r#""temperature":0.7"#));
    assert!(json.contains(r#""stream":false"#));
    assert!(json.contains(r#""text":"Hello""#));
}

#[test]
fn test_system_content_blocks() {
    let request = MessagesRequest {
        model: "claude-3-5-sonnet-20241022".into(),
        system: Some(AnthropicSystemContent::Blocks(vec![
            AnthropicSystemBlock::Text {
                text: "You are an expert".into(),
            },
        ])),
        messages: vec![],
        max_tokens: 1024,
        temperature: None,
        top_p: None,
        top_k: None,
        stream: Some(false),
        stop_sequences: None,
        tools: None,
        tool_choice: None,
        thinking: None,
    };

    let json = serde_json::to_string(&request).unwrap();
    assert!(json.contains(r#""type":"text""#));
    assert!(json.contains("You are an expert"));
}

#[test]
fn test_message_content_block_serialization() {
    let msg = AnthropicMessage {
        role: AnthropicRole::User,
        content: vec![AnthropicContentBlock::Text {
            text: "Hello".into(),
        }],
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains(r#""role":"user""#));
    assert!(json.contains(r#""type":"text""#));
    assert!(json.contains(r#""text":"Hello""#));
}

#[test]
fn test_image_content_block() {
    let msg = AnthropicMessage {
        role: AnthropicRole::User,
        content: vec![AnthropicContentBlock::Image {
            source: AnthropicImageSource {
                source_type: "base64".into(),
                media_type: "image/png".into(),
                data: "iVBORw0KGgo".into(),
            },
        }],
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains(r#""type":"image""#));
    assert!(json.contains(r#""media_type":"image/png""#));
    assert!(json.contains("iVBORw0KGgo"));
}

#[test]
fn test_tool_use_content_block() {
    let msg = AnthropicMessage {
        role: AnthropicRole::Assistant,
        content: vec![AnthropicContentBlock::ToolUse {
            id: "tool_123".into(),
            name: "get_weather".into(),
            input: serde_json::json!({"location": "Paris"}),
        }],
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains(r#""type":"tool_use""#));
    assert!(json.contains(r#""name":"get_weather""#));
    assert!(json.contains("Paris"));
}

#[test]
fn test_tool_result_content_block() {
    let msg = AnthropicMessage {
        role: AnthropicRole::User,
        content: vec![AnthropicContentBlock::ToolResult {
            tool_use_id: "tool_123".into(),
            content: "Sunny, 25°C".into(),
            is_error: None,
        }],
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains(r#""type":"tool_result""#));
    assert!(json.contains(r#""tool_use_id":"tool_123""#));
    assert!(json.contains("Sunny"));
}

#[test]
fn test_anthropic_tool_choice_serialization() {
    let tc = AnthropicToolChoice::Auto;
    let json = serde_json::to_string(&tc).unwrap();
    assert_eq!(json, r#"{"type":"auto"}"#);

    let tc = AnthropicToolChoice::Any;
    let json = serde_json::to_string(&tc).unwrap();
    assert_eq!(json, r#"{"type":"any"}"#);

    let tc = AnthropicToolChoice::Tool {
        name: "get_weather".into(),
    };
    let json = serde_json::to_string(&tc).unwrap();
    assert!(json.contains(r#""type":"tool""#));
    assert!(json.contains(r#""name":"get_weather""#));
}

#[test]
fn test_anthropic_tool_definition() {
    let tool = AnthropicTool {
        name: "get_weather".into(),
        description: "Get weather for a location".into(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "location": {"type": "string"}
            }
        }),
    };
    let json = serde_json::to_string(&tool).unwrap();
    assert!(json.contains(r#""name":"get_weather""#));
    assert!(json.contains("Get weather"));
    assert!(json.contains("location"));
}

#[test]
fn test_messages_response_deserialization() {
    let json = r#"{
        "id": "msg_abc123",
        "type": "message",
        "role": "assistant",
        "content": [{"type": "text", "text": "Hello!"}],
        "model": "claude-3-5-sonnet-20241022",
        "stop_reason": "end_turn",
        "stop_sequence": null,
        "usage": {"input_tokens": 10, "output_tokens": 5, "cache_creation_input_tokens": 0, "cache_read_input_tokens": 0}
    }"#;

    let response: MessagesResponse = serde_json::from_str(json).unwrap();
    assert_eq!(response.id, "msg_abc123");
    assert_eq!(response.role, "assistant");
    assert_eq!(response.stop_reason.as_deref(), Some("end_turn"));
    assert_eq!(response.usage.input_tokens, 10);
    assert_eq!(response.usage.output_tokens, 5);
}

#[test]
fn test_response_with_tool_use() {
    let json = r#"{
        "id": "msg_abc123",
        "type": "message",
        "role": "assistant",
        "content": [{"type": "tool_use", "id": "tool_1", "name": "get_weather", "input": {"location": "Paris"}}],
        "model": "claude-3-5-sonnet-20241022",
        "stop_reason": "tool_use",
        "stop_sequence": null,
        "usage": {"input_tokens": 50, "output_tokens": 20, "cache_creation_input_tokens": 0, "cache_read_input_tokens": 0}
    }"#;

    let response: MessagesResponse = serde_json::from_str(json).unwrap();
    assert_eq!(response.stop_reason.as_deref(), Some("tool_use"));
    match &response.content[0] {
        AnthropicContentBlock::ToolUse { id, name, input } => {
            assert_eq!(id, "tool_1");
            assert_eq!(name, "get_weather");
            assert!(input.get("location").is_some());
        }
        _ => panic!("Expected ToolUse content block"),
    }
}

// --- SSE event deserialization tests ---

#[test]
fn test_stream_event_deserialization() {
    let json = r#"{
        "type": "message_start",
        "message": {
            "id": "msg_abc",
            "type": "message",
            "role": "assistant",
            "content": [],
            "model": "claude-3-5-sonnet-20241022",
            "stop_reason": null,
            "stop_sequence": null,
            "usage": {"input_tokens": 10, "output_tokens": 0, "cache_creation_input_tokens": 0, "cache_read_input_tokens": 0}
        }
    }"#;

    let event: StreamEvent = serde_json::from_str(json).unwrap();
    match event {
        StreamEvent::MessageStart { message } => {
            assert_eq!(message.id, "msg_abc");
        }
        _ => panic!("Expected MessageStart"),
    }
}

#[test]
fn test_content_block_delta_deserialization() {
    let json = r#"{
        "type": "content_block_delta",
        "index": 0,
        "delta": {"type": "text_delta", "text": "Hello"}
    }"#;

    let event: StreamEvent = serde_json::from_str(json).unwrap();
    match event {
        StreamEvent::ContentBlockDelta { delta, .. } => match delta {
            AnthropicDelta::TextDelta { text } => {
                assert_eq!(text, "Hello");
            }
            _ => panic!("Expected TextDelta"),
        },
        _ => panic!("Expected ContentBlockDelta"),
    }
}

#[test]
fn test_message_delta_deserialization() {
    let json = r#"{
        "type": "message_delta",
        "delta": {"stop_reason": "end_turn"},
        "usage": {"input_tokens": 10, "output_tokens": 15, "cache_creation_input_tokens": 0, "cache_read_input_tokens": 0}
    }"#;

    let event: StreamEvent = serde_json::from_str(json).unwrap();
    match event {
        StreamEvent::MessageDelta { delta, usage } => {
            assert_eq!(delta.stop_reason.as_deref(), Some("end_turn"));
            assert_eq!(usage.output_tokens, 15);
        }
        _ => panic!("Expected MessageDelta"),
    }
}

// --- parse_response tests ---

#[test]
fn test_parse_response_text() {
    let response = MessagesResponse {
        id: "msg_123".into(),
        response_type: "message".into(),
        role: "assistant".into(),
        content: vec![AnthropicContentBlock::Text {
            text: "Hello!".into(),
        }],
        model: "claude-3-5-sonnet-20241022".into(),
        stop_reason: Some("end_turn".into()),
        stop_sequence: None,
        usage: AnthropicUsage {
            input_tokens: 10,
            output_tokens: 5,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        },
    };

    let model_id = ModelId::Name("claude-3-5-sonnet".into(), None);
    let (msgs, _report) = parse_response(&response, &model_id, &ModelUsageCosting::default()).unwrap();

    assert_eq!(msgs.len(), 1);
    match &msgs[0] {
        Messages::Assistant {
            content,
            stop_reason,
            provider,
            ..
        } => {
            assert_eq!(stop_reason, &StopReason::Stop);
            assert_eq!(provider, &ModelProviders::ANTHROPIC);
            if let ModelOutput::Text(tc) = content {
                assert_eq!(tc.content, "Hello!");
            } else {
                panic!("Expected Text output");
            }
        }
        _ => panic!("Expected Assistant message"),
    }
}

#[test]
fn test_parse_response_tool_use() {
    let response = MessagesResponse {
        id: "msg_123".into(),
        response_type: "message".into(),
        role: "assistant".into(),
        content: vec![AnthropicContentBlock::ToolUse {
            id: "tool_1".into(),
            name: "get_weather".into(),
            input: serde_json::json!({"location": "Paris"}),
        }],
        model: "claude-3-5-sonnet-20241022".into(),
        stop_reason: Some("tool_use".into()),
        stop_sequence: None,
        usage: AnthropicUsage {
            input_tokens: 50,
            output_tokens: 20,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        },
    };

    let model_id = ModelId::Name("claude-3-5-sonnet".into(), None);
    let (msgs, _report) = parse_response(&response, &model_id, &ModelUsageCosting::default()).unwrap();

    assert_eq!(msgs.len(), 1);
    match &msgs[0] {
        Messages::Assistant {
            content,
            stop_reason,
            ..
        } => {
            assert_eq!(stop_reason, &StopReason::ToolUse);
            if let ModelOutput::ToolCall { id, name, .. } = content {
                assert_eq!(id, "tool_1");
                assert_eq!(name, "get_weather");
            } else {
                panic!("Expected ToolCall output");
            }
        }
        _ => panic!("Expected Assistant message"),
    }
}

#[test]
fn test_parse_response_thinking() {
    let response = MessagesResponse {
        id: "msg_123".into(),
        response_type: "message".into(),
        role: "assistant".into(),
        content: vec![
            AnthropicContentBlock::Thinking {
                thinking: "Let me think...".into(),
                signature: "sig_abc".into(),
            },
            AnthropicContentBlock::Text {
                text: "The answer is 42.".into(),
            },
        ],
        model: "claude-3-7-sonnet-20250219".into(),
        stop_reason: Some("end_turn".into()),
        stop_sequence: None,
        usage: AnthropicUsage {
            input_tokens: 20,
            output_tokens: 10,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        },
    };

    let model_id = ModelId::Name("claude-3-7-sonnet".into(), None);
    let (msgs, _report) = parse_response(&response, &model_id, &ModelUsageCosting::default()).unwrap();

    assert_eq!(msgs.len(), 2);

    // First message: thinking block
    match &msgs[0] {
        Messages::Assistant { content, .. } => {
            if let ModelOutput::ThinkingContent { thinking, signature } = content {
                assert_eq!(thinking, "Let me think...");
                assert_eq!(signature.as_deref(), Some("sig_abc"));
            } else {
                panic!("Expected ThinkingContent");
            }
        }
        _ => panic!("Expected Assistant message with ThinkingContent"),
    }

    // Second message: text block
    match &msgs[1] {
        Messages::Assistant { content, .. } => {
            if let ModelOutput::Text(tc) = content {
                assert_eq!(tc.content, "The answer is 42.");
            } else {
                panic!("Expected Text output");
            }
        }
        _ => panic!("Expected Assistant message"),
    }
}

// --- build_anthropic_request tests ---

#[test]
fn test_build_anthropic_request_basic() {
    let interaction = ModelInteraction {
        system_prompt: Some("You are helpful".into()),
        soul: None,
        messages: vec![Messages::User {
            role: "user".into(),
            content: UserModelContent::Text(TextContent {
                content: "Hello".into(),
                signature: None,
            }),
            signature: None,
        }],
        tools_shed: None,
        chat_template: None,
        tool_choice: None,
    };
    let params = ModelParams::default();

    let request = build_anthropic_request("claude-3-5-sonnet", &interaction, &params, false);

    assert_eq!(request.model, "claude-3-5-sonnet");
    assert!(request.system.is_some());
    assert_eq!(request.messages.len(), 1);
    assert!(matches!(request.messages[0].role, AnthropicRole::User));
    assert_eq!(request.max_tokens, 2048);
    assert_eq!(request.stream, Some(false));
}

#[test]
fn test_build_anthropic_request_with_tools() {
    let test_tool = Tool {
        id: "tool_1".into(),
        name: "get_weather".into(),
        description: "Get weather info".into(),
        arguments: Some(Args::from_value(serde_json::json!({
            "type": "object",
            "properties": {
                "location": { "type": "string" }
            },
            "required": ["location"],
        }))),
        returns: None,
    };
    let interaction = ModelInteraction {
        system_prompt: None,
        soul: None,
        tools_shed: Some(ToolShed {
            shed: test_tool.clone(),
            memory: None,
            delegate: None,
            read: test_tool.clone(),
            edit: test_tool.clone(),
            write: test_tool.clone(),
            search: test_tool.clone(),
            bash: None,
            others: None,
        }),
        messages: vec![],
        chat_template: None,
        tool_choice: None,
    };
    let params = ModelParams::default();

    let request = build_anthropic_request("claude-3-5-sonnet", &interaction, &params, false);

    assert!(request.tools.is_some());
    let tools = request.tools.unwrap();
    assert_eq!(tools.len(), 5); // shed + read + edit + write + search
    assert_eq!(tools[0].name, "get_weather");
    assert_eq!(tools[0].description, "Get weather info");
}

#[test]
fn test_build_anthropic_request_image() {
    let interaction = ModelInteraction {
        system_prompt: None,
        soul: None,
        messages: vec![Messages::User {
            role: "user".into(),
            content: UserModelContent::Image(ImageContent {
                b64: "base64data".into(),
                mime_type: MimeType::ImagePng,
            }),
            signature: None,
        }],
        tools_shed: None,
        chat_template: None,
        tool_choice: None,
    };
    let params = ModelParams::default();

    let request = build_anthropic_request("claude-3-5-sonnet", &interaction, &params, false);

    assert_eq!(request.messages.len(), 1);
    match &request.messages[0].content[0] {
        AnthropicContentBlock::Image { source } => {
            assert_eq!(source.media_type, "image/png");
            assert_eq!(source.data, "base64data");
        }
        _ => panic!("Expected Image content block"),
    }
}

#[test]
fn test_build_anthropic_request_tool_result() {
    let interaction = ModelInteraction {
        system_prompt: None,
        soul: None,
        messages: vec![
            Messages::Assistant {
                model: ModelId::Name("test".into(), None),
                timestamp: SystemTime::now(),
                usage: empty_usage_report(),
                content: ModelOutput::ToolCall {
                    id: "tool_1".into(),
                    name: "get_weather".into(),
                    arguments: None,
                    signature: None,
                },
                stop_reason: StopReason::ToolUse,
                provider: ModelProviders::ANTHROPIC,
                error_detail: None,
                signature: None,
                metadata: None,
            },
            Messages::ToolResult {
                id: "tool_1".into(),
                name: "get_weather".into(),
                timestamp: SystemTime::now(),
                details: None,
                content: UserModelContent::Text(TextContent {
                    content: "Sunny".into(),
                    signature: None,
                }),
                error_detail: None,
                signature: None,
            },
        ],
        tools_shed: None,
        chat_template: None,
        tool_choice: None,
    };
    let params = ModelParams::default();

    let request = build_anthropic_request("claude-3-5-sonnet", &interaction, &params, false);

    assert_eq!(request.messages.len(), 2);
    assert!(matches!(
        request.messages[0].content[0],
        AnthropicContentBlock::ToolUse { .. }
    ));
    assert!(matches!(
        request.messages[1].content[0],
        AnthropicContentBlock::ToolResult { .. }
    ));
}

// --- Utility function tests ---

#[test]
fn test_anthropic_error_parsing() {
    let body = r#"{"error":{"type":"invalid_request_error","message":"Invalid API key"}}"#;
    let detail = parse_anthropic_error(body).expect("Should parse Anthropic error");
    assert!(detail.contains("invalid_request_error"));
    assert!(detail.contains("Invalid API key"));
}

#[test]
fn test_anthropic_error_parsing_no_type() {
    let body = r#"{"error":{"message":"Something went wrong"}}"#;
    let detail = parse_anthropic_error(body).expect("Should parse Anthropic error");
    assert_eq!(detail, "Something went wrong");
}

#[test]
fn test_anthropic_error_plain_text_fallback() {
    let detail = parse_anthropic_error("Internal Server Error");
    assert!(detail.is_none());
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
    assert_eq!(exponential_backoff(5), 30);
    assert_eq!(exponential_backoff(10), 30);
}

#[test]
fn test_format_http_errors() {
    assert!(format_http_error(401, "bad").contains("Authentication"));
    assert!(format_http_error(403, "bad").contains("Permission"));
    assert!(format_http_error(429, "bad").contains("Rate limit"));
    assert!(format_http_error(500, "bad").contains("Server error"));
}

#[test]
fn test_stop_reason_mapping() {
    assert_eq!(map_stop_reason(&Some("end_turn".into())), StopReason::Stop);
    assert_eq!(
        map_stop_reason(&Some("stop_sequence".into())),
        StopReason::Stop
    );
    assert_eq!(
        map_stop_reason(&Some("max_tokens".into())),
        StopReason::Length
    );
    assert_eq!(
        map_stop_reason(&Some("tool_use".into())),
        StopReason::ToolUse
    );
    assert_eq!(
        map_stop_reason(&Some("unknown_reason".into())),
        StopReason::Message("unknown_reason".into())
    );
    assert_eq!(map_stop_reason(&None), StopReason::Stop);
}
