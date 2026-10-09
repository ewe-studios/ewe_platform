//! Example: Build a Claude agent manually — no harness helpers.
//!
//! Demonstrates the full manual path: create providers, box them with
//! identities, register routing rules, build the ProviderRouter, and wire
//! into AgentSession — all without any `harness::*` functions.
//!
//! Run with:
//! ```bash
//! ANTHROPIC_API_KEY=sk-... cargo run -p foundation_ai \
//!   --example manual_claude_router
//! ```

use foundation_ai::agentic::{AgentConfig, AgentSession};
use foundation_ai::backends::anthropic_messages_provider::{
    AnthropicConfig, AnthropicMessagesProvider,
};
use foundation_ai::types::{ModelProviders, ProviderRouter, RoutableProviderBox, RoutingRule};
use foundation_core::valtron::valtron;

#[valtron]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .expect("ANTHROPIC_API_KEY must be set");

    // --- Step 1: Create providers ---
    // (`api_key(..)` is `with_config(AnthropicConfig::api_key(..))`, i.e. an
    // `AuthCredential::SecretOnly` credential.)
    let main_provider = AnthropicMessagesProvider::api_key(api_key.clone());
    let memory_provider = AnthropicMessagesProvider::with_config(AnthropicConfig::api_key(api_key));

    // --- Step 2: Box with explicit routing identities ---
    // name = model id string (must be distinct per role)
    // provider_id = from descriptor (anthropic-messages)
    let main_box = RoutableProviderBox::with_identity(
        main_provider,
        "claude-opus-4-8",
        ModelProviders::ANTHROPIC,
    );
    let memory_box = RoutableProviderBox::with_identity(
        memory_provider,
        "claude-sonnet-4-6",
        ModelProviders::ANTHROPIC,
    );

    // --- Step 3: Build router with explicit rules ---
    // Explicit RoutingRule is required because serves() is inconsistent:
    //   - AnthropicMessagesProvider::serves() → always true (greedy)
    //   - HuggingFaceGGUFProvider::serves() → always false (no catalog)
    let router = ProviderRouter::builder()
        .add_provider(Box::new(main_box))
        .add_provider(Box::new(memory_box))
        .rule(RoutingRule {
            model: "claude-opus-4-8".into(),
            provider_name: "claude-opus-4-8".into(),
        })
        .rule(RoutingRule {
            model: "claude-sonnet-4-6".into(),
            provider_name: "claude-sonnet-4-6".into(),
        })
        .build();

    // --- Step 4: Build the session ---
    let agent = AgentSession::builder(router)
        .with_model("claude-opus-4-8")
        .with_memory_model("claude-sonnet-4-6")
        .with_config(AgentConfig {
            max_outer_iterations: 3,
            ..AgentConfig::default()
        })
        .with_system_prompt("You are a helpful assistant.")
        .build()?;

    println!("Manual Claude router built successfully!");

    // --- Step 5: Run a turn ---
    println!("Sending hello prompt...");
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
        "\n{}\n\nManual Claude agent responded! Got {} records.",
        turn.text(),
        turn.len()
    );

    agent.end()?;
    Ok(())
}
