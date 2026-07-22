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
///
/// Not `openai/gpt-4o-mini`: routing to OpenAI through OpenRouter returns
/// `403 "prohibited due to a violation of provider Terms Of Service"` unless the
/// account has the matching data policy enabled, so it fails for reasons that
/// have nothing to do with this crate. Mistral Nemo is served directly, is
/// roughly half the price, and advertises `tools` support.
const MODEL: &str = "mistralai/mistral-nemo";

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
        // Base URL must NOT include the version segment: `build_url` composes
        // `{base_url}/{api_version}/{endpoint}` and `api_version` defaults to
        // "v1". Passing ".../api/v1" here produced ".../api/v1/v1/chat/completions",
        // which OpenRouter answers with a 404 HTML page.
        //
        // `OPENROUTER_BASE_URL` redirects these tests at a local listener so the
        // raw request bytes can be inspected — the streaming failure is a
        // wire-level difference from what `curl` sends, not a payload problem.
        .with_base_url(
            std::env::var("OPENROUTER_BASE_URL")
                .unwrap_or_else(|_| "https://openrouter.ai/api".to_string()),
        )
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
    // Bounded: a stream that never yields text must fail on the assertion
    // rather than spin until the HTTP read timeout. Non-`Next` items are normal
    // scheduling churn, so the cap is generous.
    const MAX_ITEMS: usize = 5_000;

    let mut text_tokens = 0;
    let mut seen: Vec<String> = Vec::new();
    for (i, item) in stream.enumerate() {
        if seen.len() < 12 {
            let mut d = format!("{item:?}");
            d.truncate(160);
            seen.push(d);
        }
        match item {
            Stream::Next(Messages::Assistant {
                content: ModelOutput::Text(_),
                ..
            }) => text_tokens += 1,
            Stream::Next(_) => {}
            // A network stream reports back-pressure instead of blocking. The
            // local-model tests can spin because tokens land synchronously;
            // here, spinning never gives the transport a chance to progress, so
            // the scheduling states have to be honoured.
            Stream::Delayed(d) => std::thread::sleep(d),
            _ => std::thread::sleep(Duration::from_millis(10)),
        }
        if text_tokens >= 2 || i >= MAX_ITEMS {
            break;
        }
    }
    // Report what actually arrived: "must produce tokens" alone gives no way to
    // tell a transport failure from a shape mismatch in the decoded message.
    assert!(
        text_tokens >= 1,
        "openrouter stream must produce text tokens; got {text_tokens}.\nFirst items observed:\n{}",
        seen.join("\n")
    );
}
