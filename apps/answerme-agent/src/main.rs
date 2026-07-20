//! answerme-agent — interactive REPL backed by a Gemma model via llama.cpp.
//!
//! Starts a `foundation_repl` session, sends each user input to an
//! `AgentSession` running on a local model, and prints the assistant's
//! response back into the REPL.

use std::path::PathBuf;

use foundation_ai::agentic::{
    AgentConfig, AgentSession, ContextConfig, ErrorPolicy, KvMemoryStore, MemoryConfig,
};
use foundation_ai::backends::huggingface_gguf_provider::{
    HuggingFaceGGUFConfig, HuggingFaceGGUFProvider,
};
use foundation_ai::backends::llamacpp::{LlamaBackendConfig, LlamaBackends};
use foundation_ai::harness::RouterMix;
use foundation_ai::types::{Messages, ModelId, SessionId, SessionRecord};
use foundation_ai::types::{ModelOutput, TextContent};
use foundation_db::{MemoryDocumentStore, MemoryStorage};
use foundation_repl::Repl;
use foundation_core::valtron::valtron;

/// Model cache directory — defaults to the workspace `artefacts/models`,
/// overridden at runtime by `ANSWERME_MODEL_DIR`.
fn model_dir() -> PathBuf {
    std::env::var("ANSWERME_MODEL_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../artefacts/models")
                .canonicalize()
                .unwrap_or_else(|e| panic!("model dir not found: {e}"))
        })
}

#[valtron]
fn main() {
    let session_id = SessionId::new();

    // Configure the local llama.cpp backend.
    let llama_config = LlamaBackendConfig::builder()
        .n_gpu_layers(0) // CPU-only; bump for GPU
        .context_length(4096)
        .batch_size(512)
        .n_threads(4)
        .build();

    // Configure the HuggingFace GGUF provider with our model cache dir.
    let hf_config = HuggingFaceGGUFConfig::builder()
        .cache_dir(model_dir())
        .llama_backend(LlamaBackends::LLamaCPU)
        .llama_config(llama_config)
        .build();

    let provider =
        HuggingFaceGGUFProvider::new(hf_config).expect("failed to initialize GGUF provider");

    let model_id = ModelId::Name("gemma-2-2b-it".into(), None);

    // Build the router preset and agent session.
    let preset = RouterMix::new().primary(provider, model_id.clone()).build();

    let session: AgentSession<MemoryDocumentStore, KvMemoryStore<MemoryStorage>> = preset
        .into_agent_builder(session_id)
        .with_system_prompt("You are a helpful assistant. Be concise and direct.")
        .with_model(model_id.clone())
        .with_config(AgentConfig {
            primary_model: model_id.clone(),
            ..Default::default()
        })
        .with_context_config(ContextConfig::default())
        .with_memory_config(MemoryConfig::default())
        .with_error_policy(ErrorPolicy::new())
        .build()
        .expect("failed to build agent session");

    // Launch the REPL.
    let repl = Repl::builder()
        .prompt("| ")
        .continuation_prompt("|... ")
        .banner(
            "answerme-agent — Gemma-2-2b local session\nType /help for commands, /exit to quit.\n",
        )
        .goodbye("Goodbye!")
        .build();

    repl.register_command("status", |_| "agent: running (gemma-2-2b-it)".into());

    for input in repl.messages() {
        let prompt = Messages::User {
            id: foundation_compact::ids::new_scru128(),
            role: foundation_ai::types::MessageRole::User,
            content: foundation_ai::types::UserModelContent::Text(TextContent {
                content: input,
                signature: None,
            }),
            signature: None,
        };

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
                if let ModelOutput::Text(text) = content {
                    parts.push(text.content.clone());
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
