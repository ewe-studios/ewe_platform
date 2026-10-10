//! Example: Send a "hello" prompt via OpenRouter and validate the response.
//!
//! OpenRouter proxies 200+ models through a single OpenAI-compatible API.
//! This example demonstrates:
//!   - Setting up `OpenRouter` with the `OpenAI` provider (`OpenAIConfig::openrouter`)
//!   - Calling a model by its OpenRouter id (e.g. `google/gemma-4-26b-a4b-it:free`)
//!   - Validating the response contains actual generated content
//!
//! Run with:
//! ```bash
//! OPENROUTER_API_KEY=sk-or-... cargo run -p foundation_ai \
//!   --example hello_openrouter
//! ```

use foundation_ai::agentic::AgentConfig;
use foundation_ai::backends::openai_provider::{OpenAIConfig, OpenAIProvider};
use foundation_ai::harness::{RouterMix, RouterPreset};
use foundation_core::valtron::valtron;

/// Build a router preset that talks to OpenRouter with the given model as primary.
fn openrouter_router(
    api_key: &str,
    primary_model: &str,
    memory_model: Option<&str>,
) -> RouterPreset {
    let provider = || OpenAIProvider::with_config(OpenAIConfig::openrouter(api_key));

    let mut mix = RouterMix::new().primary(provider(), primary_model);
    if let Some(memory) = memory_model {
        // A second provider for the memory model (same config, different model id).
        mix = mix.memory(provider(), memory);
    }
    mix.build()
}

#[valtron]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = std::env::var("OPENROUTER_API_KEY")
        .expect("OPENROUTER_API_KEY must be set");

    // Use a free model to avoid charges: google/gemma-4-26b-a4b-it:free
    const PRIMARY: &str = "google/gemma-4-26b-a4b-it:free";

    println!("Building OpenRouter router for model: {PRIMARY}");
    let preset = openrouter_router(&api_key, PRIMARY, None);

    // Bridge into an AgentSessionBuilder.
    let agent = preset
        .into_agent_builder()
        .with_config(AgentConfig {
            max_outer_iterations: 3,
            ..AgentConfig::default()
        })
        .with_system_prompt("You are a helpful assistant. Keep responses brief.")
        .build()?;

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
        "Expected an assistant response from OpenRouter but got none. Records: {turn:#?}"
    );

    println!(
        "\n{}\n\nOpenRouter responded successfully! Got {} records.",
        turn.text(),
        turn.len()
    );

    agent.end()?;
    Ok(())
}
