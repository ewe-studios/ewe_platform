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
//!   --example custom_router --features agentic
//! ```

use foundation_ai::agentic::KvMemoryStore;
use foundation_ai::harness::{
    CloudPresets, RouterMix, CLAUDE_OPUS, CLAUDE_SONNET, OPENAI_GPT4O,
};
use foundation_ai::types::{MessageRole, Messages, ModelId, SessionId, TextContent, UserModelContent};
use foundation_ai::{
    agentic::{AgentConfig, AgentSession, KvMemoryStore},
    types::ToolShed,
};
use foundation_compact::ids::new_scru128;
use foundation_core::valtron::valtron;
use foundation_db::{MemoryDocumentStore, MemoryStorage};

type Doc = MemoryDocumentStore;
type Mem = KvMemoryStore<MemoryStorage>;

#[valtron]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let anthropic_key = std::env::var("ANTHROPIC_API_KEY")
        .expect("ANTHROPIC_API_KEY must be set");
    let openai_key = std::env::var("OPENAI_API_KEY")
        .expect("OPENAI_API_KEY must be set");

    // Mix: Claude Opus primary, Claude Sonnet memory, GPT-4o fallback.
    let preset = RouterMix::new()
        .primary(
            CloudPresets::claude_opus(&anthropic_key)?,
            ModelId::Name(CLAUDE_OPUS.into(), None),
        )
        .memory(
            CloudPresets::claude_sonnet(&anthropic_key)?,
            ModelId::Name(CLAUDE_SONNET.into(), None),
        )
        .fallback(
            CloudPresets::openai_gpt4o(&openai_key)?,
            ModelId::Name(OPENAI_GPT4O.into(), None),
        )
        .build();

    println!(
        "Router built with primary={}, memory={:?}, fallbacks={:?}",
        preset.primary_model, preset.memory_model, preset.fallback_models,
    );

    // Bridge into an agent builder and finish customizing.
    let agent = preset
        .into_agent_builder::<Doc, Mem>(SessionId::new())
        .with_config(AgentConfig {
            max_outer_iterations: 3,
            ..AgentConfig::default()
        })
        .with_system_prompt("You are a helpful assistant with a GPT-4o fallback.")
        .build()?;

    let prompt = Messages::User {
        id: new_scru128(),
        role: MessageRole::User,
        content: UserModelContent::Text(TextContent {
            content: "Hello! Say hi and tell me which model you are.".into(),
            signature: None,
        }),
        signature: None,
    };

    println!("Sending hello via mixed router...");
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
    println!("\nCustom router agent responded! Got {} records.", records.len());

    agent.end()?;
    Ok(())
}
