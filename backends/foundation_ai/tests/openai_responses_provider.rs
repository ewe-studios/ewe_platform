//! Integration tests for `ResponsesProvider` against a running llama-server.
//!
//! These tests are #[ignore]-gated and require:
//!   1. llama-server built and running: `mise run llama:server:start`
//!   2. Environment: `LLAMA_SERVER_TEST=1`

use foundation_ai::backends::openai_responses_provider::{ResponsesConfig, ResponsesProvider};
use foundation_ai::types::{
    Messages, Model, ModelId, ModelInteraction, ModelOutput, ModelProvider, StopReason,
    TextContent, UserModelContent,
};
use foundation_auth::{AuthCredential, ConfidentialText};
use foundation_core::valtron;
use foundation_core::valtron::Stream;
use tracing_test::traced_test;

fn setup_responses_provider() -> impl Model {
    let base_url =
        std::env::var("LLAMA_SERVER_URL").unwrap_or_else(|_| "http://127.0.0.1:8999".into());
    let api_key = std::env::var("LLAMA_SERVER_API_KEY").unwrap_or_default();

    let config = ResponsesConfig::new()
        .with_base_url(base_url)
        .with_auth(AuthCredential::SecretOnly(ConfidentialText::new(api_key)));

    let provider = ResponsesProvider::new().create(Some(config)).unwrap();

    provider
        .get_model(ModelId::Name("qwen2.5-0.5b-instruct".into(), None))
        .unwrap()
}

/// Test: generate a response via the Responses API.
#[test]
#[traced_test]
#[ignore]
fn test_llama_server_responses_generate() {
    let _guard = valtron::initialize_pool(42, Some(4));
    let model = setup_responses_provider();

    let interaction = ModelInteraction {
        system_prompt: Some("You are a helpful assistant.".into()),
        messages: vec![Messages::User {
            role: "user".into(),
            content: UserModelContent::Text(TextContent {
                content: "Say hello in one word.".into(),
                signature: None,
            }),
            signature: None,
        }],
        tools: vec![],
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
#[test]
#[traced_test]
#[ignore]
fn test_llama_server_responses_stream() {
    let _guard = valtron::initialize_pool(42, Some(4));
    let model = setup_responses_provider();

    let interaction = ModelInteraction {
        system_prompt: None,
        messages: vec![Messages::User {
            role: "user".into(),
            content: UserModelContent::Text(TextContent {
                content: "Count from 1 to 3.".into(),
                signature: None,
            }),
            signature: None,
        }],
        tools: vec![],
        chat_template: None,
        tool_choice: None,
    };

    let mut stream = model.stream(interaction, None).unwrap();
    let mut received_any = false;
    for item in &mut stream {
        if let Stream::Next(_) = item {
            received_any = true;
        }
    }
    assert!(received_any, "Should have received streaming events");
}
