//! Example: Build an OpenRouter agent manually — no harness helpers.
//!
//! Demonstrates:
//!   - Creating an OpenAI provider pointed at OpenRouter's base_url
//!   - Boxing providers with routing identities
//!   - Building a ProviderRouter with explicit RoutingRules
//!   - Wiring into AgentSession and validating the response
//!
//! Run with:
//! ```bash
//! OPENROUTER_API_KEY=sk-or-... cargo run -p foundation_ai \
//!   --example manual_openrouter --features agentic
//! ```

use foundation_ai::backends::openai_provider::{OpenAIConfig, OpenAIProvider};
use foundation_ai::types::{
    MessageRole, Messages, ModelId, ProviderRouter, RoutableProviderBox, RoutingRule, SessionId,
    TextContent, UserModelContent,
};
use foundation_ai::{
    agentic::{AgentConfig, AgentSession, KvMemoryStore},
    types::ToolShed,
};
use foundation_auth::{AuthCredential, ConfidentialText};
use foundation_compact::ids::new_scru128;
use foundation_core::valtron::valtron;
use foundation_db::{MemoryDocumentStore, MemoryStorage};

type Doc = MemoryDocumentStore;
type Mem = KvMemoryStore<MemoryStorage>;

/// Build a ProviderRouter that talks to OpenRouter.
/// No harness helpers — explicit provider creation, boxing, and routing rules.
fn build_openrouter_router(
    api_key: &str,
    primary_model: &str,
    memory_model: Option<&str>,
) -> ProviderRouter {
    let make_provider = || {
        OpenAIProvider::with_config(
            OpenAIConfig::new()
                .with_auth(AuthCredential::SecretOnly(ConfidentialText::new(
                    api_key.to_string(),
                )))
                .with_base_url("https://openrouter.ai/api/v1"),
        )
    };

    let mut builder = ProviderRouter::builder()
        .add_provider(Box::new(RoutableProviderBox::with_identity(
            make_provider(),
            primary_model.to_string(),
            "openai-completions".to_string(),
        )))
        .rule(RoutingRule {
            model: ModelId::Name(primary_model.to_string(), None),
            provider_name: primary_model.to_string(),
        });

    if let Some(mem) = memory_model {
        builder = builder
            .add_provider(Box::new(RoutableProviderBox::with_identity(
                make_provider(),
                mem.to_string(),
                "openai-completions".to_string(),
            )))
            .rule(RoutingRule {
                model: ModelId::Name(mem.to_string(), None),
                provider_name: mem.to_string(),
            });
    }

    builder.build()
}

#[valtron]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = std::env::var("OPENROUTER_API_KEY")
        .expect("OPENROUTER_API_KEY must be set");

    // Use a free model to avoid charges.
    const PRIMARY: &str = "google/gemma-4-26b-a4b-it:free";

    println!("Building manual OpenRouter router for model: {PRIMARY}");
    let router = build_openrouter_router(&api_key, PRIMARY, None);

    let agent = AgentSession::<Doc, Mem>::builder(SessionId::new(), router)
        .with_toolshed(ToolShed::default())
        .with_model(ModelId::Name(PRIMARY.into(), None))
        .with_config(AgentConfig {
            max_outer_iterations: 3,
            ..AgentConfig::default()
        })
        .with_system_prompt("You are a helpful assistant. Keep responses brief.")
        .build()?;

    println!("Manual OpenRouter agent built successfully!");

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

    // Validate response.
    let conversation_count = records
        .iter()
        .filter(|r| {
            matches!(r, foundation_ai::types::SessionRecord::Conversation {
                message: foundation_ai::types::Messages::Assistant { .. },
            })
        })
        .count();

    assert!(
        generation_count > 0,
        "Expected at least one generation record from OpenRouter but got none. Records: {records:#?}"
    );

    for record in &records {
        println!("{record:?}");
    }

    println!("\nManual OpenRouter agent responded! Got {} records.", records.len());

    agent.end()?;
    Ok(())
}
