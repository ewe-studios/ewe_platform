//! Example: Run a local GGUF model via Llama.cpp (GLM 5.2 + Gemma 4 E2B).
//!
//! This downloads models from `HuggingFace` on first run. GPU offloading can be
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

use std::sync::Arc;

use foundation_ai::agentic::TurnEvent;
use foundation_ai::harness::RouterPreset;
use foundation_core::valtron::{valtron, MonotonicClock};
use futures_lite::StreamExt;

#[valtron]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // GLM 5.2 (main) + Gemma 4 E2B (memory) via Llama.cpp.
    // None, None = default Q4_K_M quantization, default cache dir, CPU.
    let clock: Arc<dyn foundation_core::valtron::Clock> = Arc::new(MonotonicClock::new());
    let builder =
        RouterPreset::glm52_gemma(None, None, Arc::clone(&clock))?.into_agent_builder(clock);

    let agent = builder
        .with_system_prompt("You are a helpful assistant.")
        .build()
        .await?;

    println!("Asking GLM 5.2 (local GGUF)...");
    // Stream the answer as it is generated.
    let mut answer = String::new();
    let mut events = agent
        .run_turn_stream("Hello! Please say hi back in one sentence.")?
        .events();
    while let Some(event) = events.next().await {
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

    agent.end().await?;
    Ok(())
}
