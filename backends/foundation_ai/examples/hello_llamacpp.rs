//! Example: Run a local GGUF model via Llama.cpp (GLM 5.2 + Gemma 4 E2B).
//!
//! This downloads models from HuggingFace on first run. GPU offloading can be
//! enabled via the `metal`, `vulkan`, or `cuda` features.
//!
//! Run with:
//! ```bash
//! # CPU only:
//! cargo run -p foundation_ai \
//!   --example hello_llamacpp
//!
//! # With Apple Metal GPU:
//! cargo run -p foundation_ai \
//!   --example hello_llamacpp --features metal
//!
//! # With CUDA GPU:
//! cargo run -p foundation_ai \
//!   --example hello_llamacpp --features cuda
//! ```

use foundation_ai::agentic::TurnEvent;
use foundation_ai::harness::RouterPreset;
use foundation_core::valtron::valtron;

#[valtron]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // GLM 5.2 (main) + Gemma 4 E2B (memory) via Llama.cpp.
    // None, None = default Q4_K_M quantization, default cache dir, CPU.
    let builder = RouterPreset::glm52_gemma(None, None)?.into_agent_builder();

    let agent = builder
        .with_system_prompt("You are a helpful assistant.")
        .build()?;

    println!("Asking GLM 5.2 (local GGUF)...");
    // Stream the answer as it is generated.
    let mut answer = String::new();
    for event in agent
        .run_turn_stream("Hello! Please say hi back in one sentence.")?
        .events()
    {
        match event {
            TurnEvent::Text(delta) => {
                print!("{delta}");
                answer.push_str(&delta);
            }
            // The loop withdrew what it had streamed and is asking again.
            TurnEvent::Retract { .. } => answer.clear(),
            TurnEvent::Failed(error) => return Err(format!("\nturn failed: {error}").into()),
            TurnEvent::Done(summary) => println!("\n[{} tokens]", summary.usage.total),
            _ => {}
        }
    }
    assert!(!answer.is_empty(), "Expected a generation but got none");
    println!("GLM 5.2 responded!");

    agent.end()?;
    Ok(())
}
