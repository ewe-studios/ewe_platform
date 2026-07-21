//! OpenRouter (OpenAI-compatible) — exercises the `OpenAIProvider` send + SSE
//! parse paths against a real service. Self-skips without `OPENROUTER_API_KEY`.

use std::sync::Arc;
use std::time::Duration;

use foundation_ai::backends::openai_provider::{OpenAIConfig, OpenAIProvider};
use foundation_ai::types::{
    MessageRole, Messages, Model, ModelId, ModelInteraction, ModelOutput, ModelParams,
    ModelProvider, TextContent, ToolShed, UserModelContent,
};
use foundation_auth::{AuthCredential, ConfidentialText};
use foundation_core::valtron::{valtron_test, Stream};
use foundation_netio::http::NativeHttpClient;
use foundation_netio::shared::client::http_client::HttpClient;

/// A cheap, widely-available OpenRouter model with tool support.
const MODEL: &str = "openai/gpt-4o-mini";

fn provider_or_skip() -> Option<impl Model> {
    let key = std::env::var("OPENROUTER_API_KEY").ok()?;
    if key.is_empty() {
        return None;
    }
    let resolver = foundation_netio::shared::client::SystemDnsResolver;
    let http: Arc<dyn HttpClient> = Arc::new(NativeHttpClient::with_expect_continue_timeout(
        resolver,
        Duration::from_secs(60),
    ));
    let config = OpenAIConfig::new()
        .with_base_url("https://openrouter.ai/api/v1".to_string())
        .with_auth(AuthCredential::SecretOnly(ConfidentialText::new(key)));
    let provider = OpenAIProvider::with_http_client(http)
        .create(Some(config))
        .expect("provider builds");
    Some(
        provider
            .get_model(ModelId::Name(MODEL.to_string(), None))
            .expect("model resolves"),
    )
}

fn interaction(text: &str) -> ModelInteraction {
    ModelInteraction {
        system_prompt: Some("You are a terse assistant.".to_string()),
        soul: None,
        messages: vec![Messages::User {
            id: foundation_compact::ids::new_scru128(),
            role: MessageRole::User,
            content: UserModelContent::Text(TextContent {
                content: text.to_string(),
                signature: None,
            }),
            signature: None,
        }],
        tools_shed: ToolShed::default(),
        chat_template: None,
        tool_choice: None,
    }
}

fn params() -> ModelParams {
    ModelParams {
        max_tokens: 24,
        temperature: 0.0,
        ..Default::default()
    }
}

#[valtron_test]
fn openrouter_generate_returns_text() {
    let Some(model) = provider_or_skip() else {
        eprintln!("[skip] OPENROUTER_API_KEY not set");
        return;
    };
    let out = model
        .generate(interaction("Reply with the single word: pong."), Some(params()))
        .expect("openrouter generate should succeed");
    let text: String = out
        .iter()
        .filter_map(|m| match m {
            Messages::Assistant {
                content: ModelOutput::Text(t),
                ..
            } => Some(t.content.clone()),
            _ => None,
        })
        .collect();
    assert!(!text.trim().is_empty(), "openrouter must return text: {out:?}");
    println!("openrouter generate: {text:?}");
}

#[valtron_test]
fn openrouter_stream_advances() {
    let Some(model) = provider_or_skip() else {
        eprintln!("[skip] OPENROUTER_API_KEY not set");
        return;
    };
    let stream = model
        .stream(interaction("Count: one two three."), Some(params()))
        .expect("stream should be created");
    let mut text_tokens = 0;
    for item in stream {
        if let Stream::Next(Messages::Assistant {
            content: ModelOutput::Text(_),
            ..
        }) = item
        {
            text_tokens += 1;
        }
        if text_tokens >= 2 {
            break;
        }
    }
    assert!(text_tokens >= 1, "openrouter stream must produce tokens");
}
