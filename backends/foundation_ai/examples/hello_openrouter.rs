//! Example: Send a "hello" prompt via OpenRouter and validate the response.
//!
//! OpenRouter proxies 200+ models through a single OpenAI-compatible API.
//! This example demonstrates:
//!   - Setting up OpenRouter with the OpenAI provider + custom base_url
//!   - Calling a model by its OpenRouter id (e.g. `google/gemma-4-26b-a4b-it:free`)
//!   - Validating the response contains actual generated content
//!
//! Run with:
//! ```bash
//! OPENROUTER_API_KEY=sk-or-... cargo run -p foundation_ai \
//!   --example hello_openrouter --features agentic
//! ```

use foundation_ai::agentic::KvMemoryStore;
use foundation_ai::backends::openai_provider::{OpenAIConfig, OpenAIProvider};
use foundation_ai::harness::{RouterMix, RouterPreset};
use foundation_ai::agentic::AgentConfig;
use foundation_ai::types::{
    MessageRole, Messages, ModelId, SessionId, TextContent, UserModelContent,
};
use foundation_auth::{AuthCredential, ConfidentialText};
use foundation_compact::ids::new_scru128;
use foundation_core::valtron::valtron;
use foundation_db::{MemoryDocumentStore, MemoryStorage};

type Doc = MemoryDocumentStore;
type Mem = KvMemoryStore<MemoryStorage>;

/// Build a router preset that talks to OpenRouter with the given model as primary.
fn openrouter_router(
    api_key: &str,
    primary_model: &str,
    memory_model: Option<&str>,
) -> Result<RouterPreset, String> {
    let config = OpenAIConfig::new()
        .with_auth(AuthCredential::SecretOnly(ConfidentialText::new(api_key.to_string())))
        .with_base_url("https://openrouter.ai/api/v1");

    let provider = OpenAIProvider::with_config(config);

    let mut mix = RouterMix::new()
        .primary(provider, ModelId::Name(primary_model.to_string(), None));

    if let Some(mem) = memory_model {
        // Re-create provider for memory model (same config, different model id).
        let config2 = OpenAIConfig::new()
            .with_auth(AuthCredential::SecretOnly(ConfidentialText::new(api_key.to_string())))
            .with_base_url("https://openrouter.ai/api/v1");
        let provider2 = OpenAIProvider::with_config(config2);
        mix = mix.memory(provider2, ModelId::Name(mem.to_string(), None));
    }

    Ok(mix.build())
}

#[valtron]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = std::env::var("OPENROUTER_API_KEY")
        .expect("OPENROUTER_API_KEY must be set");

    // Use a free model to avoid charges: google/gemma-4-26b-a4b-it:free
    const PRIMARY: &str = "google/gemma-4-26b-a4b-it:free";

    println!("Building OpenRouter router for model: {PRIMARY}");
    let preset = openrouter_router(&api_key, PRIMARY, None)?;

    // Bridge into an AgentSessionBuilder.
    let builder = preset
        .into_agent_builder::<Doc, Mem>(SessionId::new())
        .with_config(AgentConfig {
            max_outer_iterations: 3,
            ..AgentConfig::default()
        })
        .with_system_prompt("You are a helpful assistant. Keep responses brief.");

    let agent = builder.build()?;

    let prompt = Messages::User {
        id: new_scru128(),
        role: MessageRole::User,
        content: UserModelContent::Text(TextContent {
            content: "Hello! Please say hi back in one sentence.".into(),
            signature: None,
        }),
        signature: None,
    };

    println!("Sending hello prompt via OpenRouter to {PRIMARY}...");
    let records = agent.run_turn(prompt)?;

    // Validate: we should get at least one Conversation record with an Assistant message.
    let conversation_count = records.iter().filter(|r| {
        matches!(r, foundation_ai::types::SessionRecord::Conversation {
            message: foundation_ai::types::Messages::Assistant { .. },
        })
    }).count();

    assert!(
        conversation_count > 0,
        "Expected at least one assistant response from OpenRouter but got none. Records: {records:#?}"
    );

    // Print the response.
    for record in &records {
        println!("{record:?}");
    }

    println!("\nOpenRouter responded successfully! Got {} records.", records.len());

    agent.end()?;
    Ok(())
}
