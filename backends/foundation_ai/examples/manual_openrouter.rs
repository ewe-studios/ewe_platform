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

use foundation_ai::agentic::{AgentConfig, AgentSession};
use foundation_ai::backends::openai_provider::{OpenAIConfig, OpenAIProvider};
use foundation_ai::types::{ModelProviders, ProviderRouter, RoutableProviderBox, RoutingRule};
use foundation_core::valtron::valtron;

/// Build a ProviderRouter that talks to OpenRouter.
/// No harness helpers — explicit provider creation, boxing, and routing rules.
fn build_openrouter_router(
    api_key: &str,
    primary_model: &str,
    memory_model: Option<&str>,
) -> ProviderRouter {
    // `OpenAIConfig::openrouter(key)` = an API-key credential plus the
    // OpenRouter base URL.
    let make_provider = || OpenAIProvider::with_config(OpenAIConfig::openrouter(api_key));

    let mut builder = ProviderRouter::builder()
        .add_provider(Box::new(RoutableProviderBox::with_identity(
            make_provider(),
            primary_model.to_string(),
            ModelProviders::OPENROUTER,
        )))
        .rule(RoutingRule {
            model: primary_model.into(),
            provider_name: primary_model.to_string(),
        });

    if let Some(mem) = memory_model {
        builder = builder
            .add_provider(Box::new(RoutableProviderBox::with_identity(
                make_provider(),
                mem.to_string(),
                ModelProviders::OPENROUTER,
            )))
            .rule(RoutingRule {
                model: mem.into(),
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

    let agent = AgentSession::builder(router)
        .with_model(PRIMARY)
        .with_config(AgentConfig {
            max_outer_iterations: 3,
            ..AgentConfig::default()
        })
        .with_system_prompt("You are a helpful assistant. Keep responses brief.")
        .build()?;

    println!("Manual OpenRouter agent built successfully!");

    println!("Sending hello prompt via OpenRouter to {PRIMARY}...");
    let turn = agent.run_turn("Hello! Please say hi back in one sentence.")?;

    for record in &turn {
        println!("{record:?}");
    }
    if let Some(error) = turn.failure() {
        return Err(format!("the turn ended early: {error}").into());
    }
    assert!(
        !turn.text().is_empty(),
        "Expected a reply but got none: {turn:#?}"
    );
    println!(
        "\n{}\n\nManual OpenRouter agent responded! Got {} records.",
        turn.text(),
        turn.len()
    );

    agent.end()?;
    Ok(())
}
