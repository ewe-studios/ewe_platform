//! Example: Send a "hello" prompt to OpenAI GPT-4o and print the response.
//!
//! Run with:
//! ```bash
//! OPENAI_API_KEY=sk-... cargo run -p foundation_ai \
//!   --example hello_openai --features agentic
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
    let api_key = std::env::var("OPENAI_API_KEY")
        .expect("OPENAI_API_KEY must be set");

    // GPT-4o (main) + GPT-4o-mini (memory) via Chat Completions API.
    let builder = harness::openai_chat_session::<Doc, Mem>(SessionId::new(), &api_key)?;

    let agent = builder
        .with_system_prompt("You are a helpful assistant.")
        .build()?;

    let prompt = Messages::User {
        id: new_scru128(),
        role: MessageRole::User,
        content: UserModelContent::Text(TextContent {
            content: "Hello! Please say hi back in one sentence.".into(),
            signature: None,
        }),
        signature: None,
    };

    println!("Asking OpenAI (Chat Completions)...");
    let records = agent.run_turn(prompt)?;

    for record in &records {
        println!("{record:?}");
    }

    let got_text = records.iter().any(|r| {
        matches!(r, foundation_ai::types::SessionRecord::Conversation {
            message: foundation_ai::types::Messages::Assistant { .. },
        })
    });
    assert!(got_text, "Expected a generation record but got none");
    println!("\nGPT-4o responded! Got {} records.", records.len());

    agent.end()?;
    Ok(())
}
