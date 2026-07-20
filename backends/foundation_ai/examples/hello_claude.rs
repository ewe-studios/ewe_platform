//! Example: Send a "hello" prompt to Anthropic Claude and print the response.
//!
//! Run with:
//! ```bash
//! ANTHROPIC_API_KEY=sk-... cargo run -p foundation_ai \
//!   --example hello_claude --features agentic
//! ```

use foundation_ai::agentic::KvMemoryStore;
use foundation_ai::harness;
use foundation_ai::types::{MessageRole, Messages, SessionId, TextContent, UserModelContent};
use foundation_compact::ids::new_scru128;
use foundation_core::valtron::valtron;
use foundation_db::{MemoryDocumentStore, MemoryStorage};

type Doc = MemoryDocumentStore;
type Mem = KvMemoryStore<MemoryStorage>;

#[valtron]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .expect("ANTHROPIC_API_KEY must be set");

    // One-call: Claude Opus (main) + Claude Sonnet (memory) wired for you.
    let builder = harness::claude_session::<Doc, Mem>(SessionId::new(), &api_key)?;

    let agent = builder
        .with_system_prompt("You are a helpful assistant.")
        .build()?;

    // Build a user message.
    let prompt = Messages::User {
        id: new_scru128(),
        role: MessageRole::User,
        content: UserModelContent::Text(TextContent {
            content: "Hello! Please say hi back in one sentence.".into(),
            signature: None,
        }),
        signature: None,
    };

    println!("Asking Claude...");
    let records = agent.run_turn(prompt)?;

    for record in &records {
        println!("{record:?}");
    }

    // Verify we got an assistant response (conversation records containing Assistant messages).
    let got_text = records.iter().any(|r| {
        matches!(r, foundation_ai::types::SessionRecord::Conversation {
            message: foundation_ai::types::Messages::Assistant { .. },
        })
    });
    assert!(got_text, "Expected a generation record but got none");
    println!("\nClaude responded! Got {} records.", records.len());

    agent.end()?;
    Ok(())
}
