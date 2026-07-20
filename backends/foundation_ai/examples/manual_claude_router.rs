//! Example: Build a Claude agent manually — no harness helpers.
//!
//! Demonstrates the full manual path: create providers, box them with
//! identities, register routing rules, build the ProviderRouter, and wire
//! into AgentSession — all without any `harness::*` functions.
//!
//! Run with:
//! ```bash
//! ANTHROPIC_API_KEY=sk-... cargo run -p foundation_ai \
//!   --example manual_claude_router --features agentic
//! ```

use foundation_ai::backends::anthropic_messages_provider::{
    AnthropicConfig, AnthropicMessagesProvider,
};
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

#[valtron]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .expect("ANTHROPIC_API_KEY must be set");

    // --- Step 1: Create providers ---
    let main_provider = AnthropicMessagesProvider::with_config(
        AnthropicConfig::new()
            .with_auth(AuthCredential::SecretOnly(ConfidentialText::new(api_key.clone()))),
    );
    let memory_provider = AnthropicMessagesProvider::with_config(
        AnthropicConfig::new()
            .with_auth(AuthCredential::SecretOnly(ConfidentialText::new(api_key))),
    );

    // --- Step 2: Box with explicit routing identities ---
    // name = model id string (must be distinct per role)
    // provider_id = from descriptor (anthropic-messages)
    let main_box = RoutableProviderBox::with_identity(
        main_provider,
        "claude-opus-4-8".into(),
        "anthropic-messages".into(),
    );
    let memory_box = RoutableProviderBox::with_identity(
        memory_provider,
        "claude-sonnet-4-6".into(),
        "anthropic-messages".into(),
    );

    // --- Step 3: Build router with explicit rules ---
    // Explicit RoutingRule is required because serves() is inconsistent:
    //   - AnthropicMessagesProvider::serves() → always true (greedy)
    //   - HuggingFaceGGUFProvider::serves() → always false (no catalog)
    let router = ProviderRouter::builder()
        .add_provider(Box::new(main_box))
        .add_provider(Box::new(memory_box))
        .rule(RoutingRule {
            model: ModelId::Name("claude-opus-4-8".into(), None),
            provider_name: "claude-opus-4-8".into(),
        })
        .rule(RoutingRule {
            model: ModelId::Name("claude-sonnet-4-6".into(), None),
            provider_name: "claude-sonnet-4-6".into(),
        })
        .build();

    // --- Step 4: Build the session ---
    let agent = AgentSession::<Doc, Mem>::builder(SessionId::new(), router)
        .with_toolshed(ToolShed::default())
        .with_model(ModelId::Name("claude-opus-4-8".into(), None))
        .with_memory_model(ModelId::Name("claude-sonnet-4-6".into(), None))
        .with_config(AgentConfig {
            max_outer_iterations: 3,
            ..AgentConfig::default()
        })
        .with_system_prompt("You are a helpful assistant.")
        .build()?;

    println!("Manual Claude router built successfully!");

    // --- Step 5: Run a turn ---
    let prompt = Messages::User {
        id: new_scru128(),
        role: MessageRole::User,
        content: UserModelContent::Text(TextContent {
            content: "Hello! Say hi back in one sentence.".into(),
            signature: None,
        }),
        signature: None,
    };

    println!("Sending hello prompt...");
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
    println!("\nManual Claude agent responded! Got {} records.", records.len());

    agent.end()?;
    Ok(())
}
