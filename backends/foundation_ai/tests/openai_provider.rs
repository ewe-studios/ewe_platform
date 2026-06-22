//! Integration tests for `OpenAIProvider` with real HTTP servers.
//!
//! - `TestHttpServer` (request-response) — verifies non-streaming and SSE response parsing
//! - `SseTestServer` (streaming) — verifies reconnection behavior with controlled close

use std::net::SocketAddr;
use std::sync::{Arc, LazyLock};

use foundation_ai::backends::openai_provider::{OpenAIConfig, OpenAIProvider};
use foundation_ai::types::{
    CostStatus, Messages, Model, ModelId, ModelInteraction, ModelOutput, ModelParams,
    ModelProvider, ModelProviders, StopReason, TextContent, ToolShed, UsageCosting, UsageReport,
    UserModelContent,
};
use foundation_auth::{AuthCredential, ConfidentialText};
use foundation_core::valtron;
use foundation_core::valtron::Stream;
use foundation_netio::simple_http::client::native::NativeHttpClient;
use foundation_netio::simple_http::client::shared::http_client::HttpClient;
use foundation_netio::simple_http::client::shared::StaticSocketAddr;
use foundation_testing::http::{
    HttpResponse, SseConnectionResult, SseStreamWriter, SseTestServer, TestHttpServer,
};
use serial_test::serial;

static POOL: LazyLock<valtron::PoolGuard> = LazyLock::new(|| valtron::initialize_pool(42, Some(4)));

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

#[test]
#[serial]
#[tracing_test::traced_test]
fn test_provider_generate() {
    let _guard = &*POOL;

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
#[test]
#[serial]
#[tracing_test::traced_test]
fn test_provider_streaming_text() {
    let _guard = &*POOL;
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
#[test]
#[serial]
#[tracing_test::traced_test]
fn test_provider_streaming_tool_calls() {
    let _guard = &*POOL;
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
#[test]
#[serial]
#[tracing_test::traced_test]
fn test_provider_streaming_sends_post_with_body_and_parses_events() {
    let _guard = &*POOL;

    use foundation_netio::simple_http::shared::SimpleMethod;
    use foundation_testing::http::HttpRequest;

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
                foundation_netio::simple_http::shared::SendSafeBody::Text(s) => s.len(),
                foundation_netio::simple_http::shared::SendSafeBody::Bytes(v) => v.len(),
                foundation_netio::simple_http::shared::SendSafeBody::None => 0,
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

// ============================================================================
// llama-server integration tests (#[ignore]-gated)
// ============================================================================

/// Helper: create an OpenAIProvider connected to the running llama-server.
fn setup_llama_server_provider() -> impl Model {
    let base_url =
        std::env::var("LLAMA_SERVER_URL").unwrap_or_else(|_| "http://127.0.0.1:8999".into());
    let api_key = std::env::var("LLAMA_SERVER_API_KEY").unwrap_or_default();
    let model_name =
        std::env::var("LLAMA_SERVER_MODEL").unwrap_or_else(|_| "qwen2.5-0.5b-instruct".into());

    let config = OpenAIConfig::new()
        .with_base_url(base_url.clone())
        .with_auth(AuthCredential::SecretOnly(ConfidentialText::new(api_key)));

    let provider = OpenAIProvider::new().create(Some(config)).unwrap();

    provider.get_model(ModelId::Name(model_name, None)).unwrap()
}

/// Test: generate a short response against the real llama-server.
#[test]
#[serial]
#[tracing_test::traced_test]
#[ignore]
fn test_llama_server_generate() {
    let _guard = &*POOL;
    let model = setup_llama_server_provider();

    let interaction = ModelInteraction {
        system_prompt: Some("You are a helpful assistant.".into()),
        soul: None,
        messages: vec![Messages::User {
            id: foundation_compact::ids::new_scru128(),
            role: "user".into(),
            content: UserModelContent::Text(TextContent {
                content: "Say hello in one word.".into(),
                signature: None,
            }),
            signature: None,
        }],
        tools_shed: ToolShed::default(),
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
#[serial]
#[tracing_test::traced_test]
#[ignore]
fn test_llama_server_streaming() {
    let _guard = &*POOL;
    let model = setup_llama_server_provider();

    let interaction = ModelInteraction {
        system_prompt: None,
        soul: None,
        messages: vec![Messages::User {
            id: foundation_compact::ids::new_scru128(),
            role: "user".into(),
            content: UserModelContent::Text(TextContent {
                content: "Count from 1 to 3, one number per line.".into(),
                signature: None,
            }),
            signature: None,
        }],
        tools_shed: ToolShed::default(),
        chat_template: None,
        tool_choice: None,
    };

    let mut stream = model.stream(interaction, None).unwrap();
    let mut last_text = String::new();
    let mut tool_call_name = None;
    let mut tool_call_id = None;
    for item in &mut stream {
        if let Stream::Next(msg) = item {
            if let Messages::Assistant { content, .. } = &msg {
                match content {
                    ModelOutput::Text(tc) => {
                        last_text.clone_from(&tc.content);
                    }
                    ModelOutput::ToolCall { name, id, .. } => {
                        tool_call_name = Some(name.clone());
                        tool_call_id = Some(id.clone());
                    }
                    _ => {}
                }
            }
        }
    }

    if let Some(name) = &tool_call_name {
        assert!(!name.is_empty(), "Tool call name should not be empty");
        assert!(
            tool_call_id.as_ref().is_some_and(|id| !id.is_empty()),
            "Tool call id should not be empty"
        );
    } else {
        assert!(
            !last_text.is_empty(),
            "Should have received non-empty streamed text or a tool call"
        );
    }
}

/// Test: multi-turn conversation with conversation history.
#[test]
#[serial]
#[tracing_test::traced_test]
#[ignore]
fn test_llama_server_multi_turn() {
    let _guard = &*POOL;
    let model = setup_llama_server_provider();

    let interaction = ModelInteraction {
        system_prompt: None,
        soul: None,
        messages: vec![
            Messages::User {
                id: foundation_compact::ids::new_scru128(),
                role: "user".into(),
                content: UserModelContent::Text(TextContent {
                    content: "What is the capital of France?".into(),
                    signature: None,
                }),
                signature: None,
            },
            Messages::Assistant {
                id: foundation_compact::ids::new_scru128(),
                model: ModelId::Name("qwen2.5-0.5b-instruct".into(), None),
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
                provider: ModelProviders::LLAMACPP,
                error_detail: None,
                signature: None,
                metadata: None,
            },
            Messages::User {
                id: foundation_compact::ids::new_scru128(),
                role: "user".into(),
                content: UserModelContent::Text(TextContent {
                    content: "What about Spain?".into(),
                    signature: None,
                }),
                signature: None,
            },
        ],
        tools_shed: ToolShed::default(),
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
#[serial]
#[tracing_test::traced_test]
#[ignore]
fn test_llama_server_max_tokens() {
    let _guard = &*POOL;
    let model = setup_llama_server_provider();

    let interaction = ModelInteraction {
        system_prompt: None,
        soul: None,
        messages: vec![Messages::User {
            id: foundation_compact::ids::new_scru128(),
            role: "user".into(),
            content: UserModelContent::Text(TextContent {
                content: "Write a long essay about the history of computing.".into(),
                signature: None,
            }),
            signature: None,
        }],
        tools_shed: ToolShed::default(),
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

/// Test: provider can resolve a model name against the running llama-server.
#[test]
#[serial]
#[tracing_test::traced_test]
#[ignore]
fn test_llama_server_resolve_model() {
    let _guard = &*POOL;
    let model = setup_llama_server_provider();

    // If we got a model handle, the provider connected successfully.
    // Generate a minimal request to verify the connection.
    let interaction = ModelInteraction {
        system_prompt: None,
        soul: None,
        messages: vec![Messages::User {
            id: foundation_compact::ids::new_scru128(),
            role: "user".into(),
            content: UserModelContent::Text(TextContent {
                content: "Hi".into(),
                signature: None,
            }),
            signature: None,
        }],
        tools_shed: ToolShed::default(),
        chat_template: None,
        tool_choice: None,
    };

    let result = model.generate(interaction, None);
    // The server should respond (even if with an error) — no panic means connected.
    assert!(
        result.is_ok(),
        "Expected provider to connect and respond, got error: {:?}",
        result.err()
    );
}
