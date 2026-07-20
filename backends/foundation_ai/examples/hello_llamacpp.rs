//! Example: Run a local GGUF model via Llama.cpp (GLM 5.2 + Gemma 4 E2B).
//!
//! This downloads models from HuggingFace on first run. GPU offloading can be
//! enabled via the `metal`, `vulkan`, or `cuda` features.
//!
//! Run with:
//! ```bash
//! # CPU only:
//! cargo run -p foundation_ai \
//!   --example hello_llamacpp --features "agentic llamacpp"
//!
//! # With Apple Metal GPU:
//! cargo run -p foundation_ai \
//!   --example hello_llamacpp --features "agentic llamacpp metal"
//!
//! # With CUDA GPU:
//! cargo run -p foundation_ai \
//!   --example hello_llamacpp --features "agentic llamacpp cuda"
//! ```

use foundation_ai::agentic::KvMemoryStore;
use foundation_ai::harness;
use foundation_ai::types::{MessageRole, Messages, SessionId, TextContent, UserModelContent};
use foundation_compact::ids::new_scru128;
use foundation_db::{MemoryDocumentStore, MemoryStorage};

type Doc = MemoryDocumentStore;
type Mem = KvMemoryStore<MemoryStorage>;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // GLM 5.2 (main) + Gemma 4 E2B (memory) via Llama.cpp.
    // None, None = default Q4_K_M quantization, default cache dir, CPU.
    let builder = harness::glm52_gemma_session::<Doc, Mem>(SessionId::new(), None, None)?;

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

    println!("Asking GLM 5.2 (local GGUF)...");
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
    println!("\nGLM 5.2 responded! Got {} records.", records.len());

    agent.end()?;
    Ok(())
}
