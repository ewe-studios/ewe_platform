//! Example: Build a custom `RouterMix` with heterogeneous providers.
//!
//! Demonstrates:
//!   - Manually mixing different provider types into one router
//!   - Setting primary, memory, and fallback models
//!   - Bridging into an AgentSessionBuilder
//!
//! This example uses Claude (Anthropic) as primary and OpenAI as fallback.
//!
//! Run with:
//! ```bash
//! ANTHROPIC_API_KEY=sk-... OPENAI_API_KEY=sk-... cargo run -p foundation_ai \
//!   --example custom_router
//! ```

use foundation_ai::agentic::AgentConfig;
use foundation_ai::harness::{CloudPresets, RouterMix, CLAUDE_OPUS, CLAUDE_SONNET, OPENAI_GPT4O};
use foundation_core::valtron::valtron;

#[valtron]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let anthropic_key = std::env::var("ANTHROPIC_API_KEY")
        .expect("ANTHROPIC_API_KEY must be set");
    let openai_key = std::env::var("OPENAI_API_KEY")
        .expect("OPENAI_API_KEY must be set");

    // Mix: Claude Opus primary, Claude Sonnet memory, GPT-4o fallback.
    let preset = RouterMix::new()
        .primary(CloudPresets::claude_opus(&anthropic_key)?, CLAUDE_OPUS)
        .memory(CloudPresets::claude_sonnet(&anthropic_key)?, CLAUDE_SONNET)
        .fallback(CloudPresets::openai_gpt4o(&openai_key)?, OPENAI_GPT4O)
        .build();

    println!(
        "Router built with primary={}, memory={:?}, fallbacks={:?}",
        preset.primary_model, preset.memory_model, preset.fallback_models,
    );

    // Bridge into an agent builder and finish customizing.
    let agent = preset
        .into_agent_builder()
        .with_config(AgentConfig {
            max_outer_iterations: 3,
            ..AgentConfig::default()
        })
        .with_system_prompt("You are a helpful assistant with a GPT-4o fallback.")
        .build()?;

    println!("Sending hello via mixed router...");
    let turn = agent.run_turn("Hello! Say hi and tell me which model you are.")?;

    for record in &turn {
        println!("{record:?}");
    }
    if let Some(error) = turn.failure() {
        return Err(format!("the turn ended early: {error}").into());
    }
    assert!(!turn.text().is_empty(), "Expected a reply but got none");
    println!(
        "\n{}\n\nCustom router agent responded! Got {} records.",
        turn.text(),
        turn.len()
    );

    agent.end()?;
    Ok(())
}
