//! Example: Send a "hello" prompt to OpenAI GPT-4o and print the response.
//!
//! Run with:
//! ```bash
//! OPENAI_API_KEY=sk-... cargo run -p foundation_ai \
//!   --example hello_openai
//! ```

use foundation_ai::agentic::Answer;
use foundation_ai::harness;
use foundation_ai::types::SessionId;
use foundation_core::valtron::valtron;

#[valtron]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = std::env::var("OPENAI_API_KEY")
        .expect("OPENAI_API_KEY must be set");

    // GPT-4o (main) + GPT-4o-mini (memory) via Chat Completions API.
    let builder = harness::openai_chat_session(SessionId::new(), &api_key)?;

    let agent = builder
        .with_system_prompt("You are a helpful assistant.")
        .build()?;

    println!("Asking GPT-4o...");
    // `ask` returns just the text: complete, or what was produced before a
    // failure ended the turn.
    match agent.ask("Hello! Please say hi back in one sentence.")? {
        Answer::Complete(text) => {
            assert!(!text.is_empty(), "Expected a reply but got none");
            println!("{text}\n\nGPT-4o responded!");
        }
        Answer::Failed {
            partial_text,
            error,
            ..
        } => {
            println!("{partial_text}");
            return Err(format!("the turn ended early: {error}").into());
        }
    }

    agent.end()?;
    Ok(())
}
