//! Integration tests for `OpenAIProvider` with real HTTP servers.
//!
//! - `TestHttpServer` (request-response) — verifies non-streaming and SSE response parsing
//! - llama-server integration tests (#[ignore]-gated) — verifies against real llama-server
//!   with Anthropic Messages API compatibility

use std::sync::Arc;
use std::time::Duration;

use foundation_ai::backends::openai_provider::{OpenAIConfig, OpenAIProvider};
use foundation_ai::toolbox::llama_server_harness::start_llama_server;
use foundation_ai::types::{
    CostStatus, Messages, Model, ModelId, ModelInteraction, ModelOutput, ModelParams,
    ModelProvider, ModelProviders, StopReason, TextContent, ToolShed, UsageCosting, UsageReport,
    UserModelContent,
};
use foundation_auth::{AuthCredential, ConfidentialText};
use foundation_core::valtron::{valtron_test, Stream};
use foundation_netio::http::NativeHttpClient;
use foundation_netio::shared::client::http_client::HttpClient;

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

    let resolver = foundation_netio::shared::client::SystemDnsResolver;
    let http_client: Arc<dyn HttpClient> = Arc::new(
        NativeHttpClient::with_expect_continue_timeout(resolver, Duration::from_secs(30)),
    );

    let config = OpenAIConfig::new()
        .with_base_url(base_url.clone())
        .with_auth(AuthCredential::SecretOnly(ConfidentialText::new(api_key)));

    let provider = OpenAIProvider::with_http_client(http_client)
        .create(Some(config))
        .unwrap();

    provider.get_model(ModelId::Name(model_name, None)).unwrap()
}

/// Test: generate a short response against the real llama-server.
#[valtron_test]
#[tracing_test::traced_test]
fn test_llama_server_generate() {
    let _llama_server_guard = start_llama_server();
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
#[valtron_test]
#[tracing_test::traced_test]
fn test_llama_server_streaming() {
    let _llama_server_guard = start_llama_server();
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
#[valtron_test]
#[tracing_test::traced_test]
fn test_llama_server_multi_turn() {
    let _llama_server_guard = start_llama_server();
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
#[valtron_test]
#[tracing_test::traced_test]
fn test_llama_server_max_tokens() {
    let _llama_server_guard = start_llama_server();
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
#[valtron_test]
#[tracing_test::traced_test]
fn test_llama_server_resolve_model() {
    let _llama_server_guard = start_llama_server();
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
