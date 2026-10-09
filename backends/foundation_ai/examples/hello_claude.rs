//! Example: Send a "hello" prompt to Anthropic Claude and print the response.
//!
//! Run with:
//! ```bash
//! ANTHROPIC_API_KEY=sk-... cargo run -p foundation_ai \
//!   --example hello_claude --features agentic
//! ```

use foundation_ai::harness;
use foundation_ai::types::SessionId;
use foundation_core::valtron::valtron;

#[valtron]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .expect("ANTHROPIC_API_KEY must be set");

    // One-call: Claude Opus (main) + Claude Sonnet (memory) wired for you.
    let builder = harness::claude_session(SessionId::new(), &api_key)?;

    let agent = builder
        .with_system_prompt("You are a helpful assistant.")
        .build()?;

    println!("Asking Claude...");
    let turn = agent.run_turn("Hello! Please say hi back in one sentence.")?;

    // Partial output first, then the error if the turn ended early.
    println!("{}", turn.text());
    if let Some(error) = turn.failure() {
        return Err(format!("the turn ended early: {error}").into());
    }
    assert!(
        !turn.text().is_empty(),
        "Expected a reply but got none: {turn:?}"
    );
    println!("\nClaude responded! Got {} records.", turn.len());

    agent.end()?;
    Ok(())
}
