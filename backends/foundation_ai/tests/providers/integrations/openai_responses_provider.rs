//! Integration tests for `ResponsesProvider` against a running llama-server.
//!
//! These tests are #[ignore]-gated and require:
//!   1. llama-server built and running: `mise run llama:server:start`
//!   2. Environment: `LLAMA_SERVER_TEST=1`

use std::sync::Arc;
use std::time::Duration;

use foundation_ai::backends::openai_responses_provider::{ResponsesConfig, ResponsesProvider};
use foundation_ai::toolbox::llama_server_harness::start_llama_server;
use foundation_ai::types::{
    Messages, Model, ModelId, ModelInteraction, ModelOutput, ModelProvider, StopReason,
    TextContent, ToolShed, UserModelContent,
};
use foundation_auth::{AuthCredential, ConfidentialText};
use foundation_core::valtron;
use foundation_core::valtron::valtron_test;
use foundation_core::valtron::Stream;
use foundation_netio::simple_http::client::native::NativeHttpClient;
use foundation_netio::simple_http::client::shared::http_client::HttpClient;
use tracing_test::traced_test;

fn setup_responses_provider() -> impl Model {
    let base_url =
        std::env::var("LLAMA_SERVER_URL").unwrap_or_else(|_| "http://127.0.0.1:8999".into());
    let api_key = std::env::var("LLAMA_SERVER_API_KEY").unwrap_or_default();

    let resolver = foundation_netio::simple_http::client::shared::SystemDnsResolver;
    let http_client: Arc<dyn HttpClient> = Arc::new(
        NativeHttpClient::with_expect_continue_timeout(resolver, Duration::from_secs(30)),
    );

    let config = ResponsesConfig::new()
        .with_base_url(base_url)
        .with_auth(AuthCredential::SecretOnly(ConfidentialText::new(api_key)));

    let provider = ResponsesProvider::with_http_client(http_client)
        .create(Some(config))
        .unwrap();

    provider
        .get_model(ModelId::Name("qwen2.5-0.5b-instruct".into(), None))
        .unwrap()
}

/// Test: generate a response via the Responses API.
#[valtron_test]
#[traced_test]
fn test_llama_server_responses_generate() {
    let llama_server_guard = start_llama_server();
    let model = setup_responses_provider();

    let interaction = ModelInteraction {
        system_prompt: Some("You are a helpful assistant.".into()),
        soul: None,
        messages: vec![Messages::User {
            id: foundation_compact::ids::new_scru128(),
            role: foundation_ai::types::MessageRole::User,
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
            assert!(!tc.content.is_empty());
        } else {
            panic!("Expected Text output");
        }
        assert!(
            matches!(stop_reason, StopReason::Stop | StopReason::Length),
            "Expected Stop or Length, got {stop_reason:?}"
        );
    }
}

/// Test: streaming via the Responses API.
#[valtron_test]
#[traced_test]
fn test_llama_server_responses_stream() {
    let llama_server_guard = start_llama_server();
    let model = setup_responses_provider();

    let interaction = ModelInteraction {
        system_prompt: None,
        soul: None,
        messages: vec![Messages::User {
            id: foundation_compact::ids::new_scru128(),
            role: foundation_ai::types::MessageRole::User,
            content: UserModelContent::Text(TextContent {
                content: "Count from 1 to 3.".into(),
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
