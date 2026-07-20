//! answerme-agent — interactive REPL backed by a Gemma-4 model via llama.cpp.
//!
//! Starts a `foundation_shell_repl` session, sends each user input to an
//! `AgentSession` running on the local `gemma-4b` model, and prints the
//! assistant's response back into the REPL.

use std::sync::Arc;

use foundation_ai::agentic::{
    AgentConfig, AgentSession, AgentProgress, ContextConfig, ErrorPolicy, MemoryConfig,
};
use foundation_ai::backends::llamacpp::{LlamaBackendConfig, LlamaBackends, LlamaModels};
use foundation_ai::backends::huggingface_gguf_provider::HuggingfaceGgufProvider;
use foundation_ai::types::{Messages, ModelId, ProviderRouter, SessionId, SessionRecord, Stream};
use foundation_ai::errors::AgenticError;
use foundation_db::{MemoryDocumentStore, MemoryStorage};
use foundation_shell_repl::Repl;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let session_id = SessionId::new();

    // Build the provider router with a local Gemma-4b model.
    let llama_config = LlamaBackendConfig::builder()
        .n_gpu_layers(0)       // CPU-only; bump for GPU
        .context_length(4096)
        .batch_size(512)
        .n_threads(4)
        .build();

    let provider = HuggingfaceGgufProvider::builder()
        .repo("bartowski/gemma-2-2b-it-GGUF")
        .filename("gemma-2-2b-it-Q4_K_M.gguf")
        .llama_backend(LlamaBackends::LLamaCPU)
        .llama_config(llama_config)
        .build()
        .unwrap();

    let router = ProviderRouter::builder()
        .model(ModelId::Name("gemma-2-2b-it".into(), None), Arc::new(provider))
        .build();

    // Build the agent session.
    let session: AgentSession<MemoryDocumentStore, foundation_ai::agentic::KvMemoryStore> =
        AgentSession::builder(session_id, router)
            .with_system_prompt("You are a helpful assistant. Be concise and direct.")
            .with_model(ModelId::Name("gemma-2-2b-it".into(), None))
            .with_config(AgentConfig {
                primary_model: ModelId::Name("gemma-2-2b-it".into(), None),
                ..Default::default()
            })
            .with_context_config(ContextConfig::default())
            .with_memory_config(MemoryConfig::default())
            .with_error_policy(ErrorPolicy::new())
            .build()
            .expect("failed to build agent session");

    // Launch the REPL.
    let mut repl = Repl::builder()
        .prompt("| ")
        .continuation_prompt("|... ")
        .banner("answerme-agent — Gemma-4b local session\nType /help for commands, /exit to quit.\n")
        .goodbye("Goodbye!")
        .build();

    repl.register_command("status", |_| "agent: running (gemma-2-2b-it)".into());

    for input in repl.messages() {
        let prompt = Messages::user(&input);

        match session.run_turn(prompt) {
            Ok(records) => {
                let response = extract_assistant_text(&records);
                repl.reply(&response);
            }
            Err(e) => {
                repl.reply(&format!("error: {e}"));
            }
        }
    }
}

/// Extract the assistant's reply text from a list of session records.
fn extract_assistant_text(records: &[SessionRecord]) -> String {
    let mut parts = Vec::new();
    for record in records {
        if let SessionRecord::Conversation { message } = record {
            if let Messages::Assistant { content, .. } = message {
                for item in content {
                    if let foundation_ai::types::TextContent::Text(t) = item {
                        parts.push(t.text.clone());
                    }
                }
            }
        }
    }
    if parts.is_empty() {
        "(no response)".into()
    } else {
        parts.join("\n")
    }
}
