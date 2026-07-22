//! answerme-agent — talk to a Gemma model via llama.cpp.
//!
//! Two entry points, both driving the same `AgentSession`:
//!
//! * `ask "<question>"` — one-shot; sends the question, prints the answer, exits.
//!   This is the quick path for validating a change (including tracing output)
//!   without driving a terminal UI.
//! * `agent` — starts a `foundation_repl` session for an interactive chat.

use std::path::PathBuf;

use clap::{Parser, Subcommand};
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
use foundation_core::valtron::valtron;
use foundation_db::{MemoryDocumentStore, MemoryStorage};
use foundation_repl::Repl;

/// The concrete session type — spelled once so both commands share it.
type Session = AgentSession<MemoryDocumentStore, KvMemoryStore<MemoryStorage>>;

#[derive(Parser)]
#[command(name = "answerme-agent", about = "Local Gemma agent over llama.cpp")]
struct Cli {
    /// Defaults to `agent` so a bare invocation still opens the REPL.
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Send a single question, print the answer, and exit.
    Ask {
        /// The question, as one quoted argument.
        question: String,
    },
    /// Start the interactive REPL.
    Agent,
}

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

// Scoped filter, not a bare `trace`: a global trace level pulls in `mio`'s
// per-poll events, which fire continuously under the REPL's raw-mode input loop
// and shred the prompt. Trace what we own, silence the transport churn.
//
// llama.cpp/ggml route their native logs through tracing, and the target for
// BOTH is the literal `llama-cpp-2` — see the `log_cs!` macro in
// infrastructure/llama-cpp/src/log.rs, which hard-codes it in `Metadata::new`.
// `llama.cpp` and `ggml` are only the `module` FIELD value on those events, so
// directives like `llama.cpp=off` match nothing and silence nothing; that is
// why the model-loader dump and per-tensor `repack:` spam kept flooding the
// REPL. `llama-cpp-2=off` is the directive that actually works.
// Regression-locked by infrastructure/llama-cpp/tests/log_filtering.rs.
//
// To see the native logs again, change `llama-cpp-2=off` to `llama-cpp-2=debug`
// here and rebuild. RUST_LOG will NOT do it: `try_init_tracing_with` only reads
// RUST_LOG when no explicit `tracing = "..."` is given (foundation_compact/src/
// trace/mod.rs — `Some(s) => EnvFilter::new(s)`, `None => try_from_default_env`),
// and this entry point always supplies one. Measured on the shipped directive:
// 0 stderr lines from llama.cpp; flipped to `=debug`: 1214 lines, 1200 of them
// the model-loader metadata dump.
//
// The valtron pool logs its own lifecycle — worker STARTED/STOPPED, shutdown,
// registry clearing — at INFO and WARN, which put 11 more lines around a
// one-line answer. They describe routine startup and teardown rather than
// anything actionable, so this CLI drops them to `error`: a worker that fails
// for real still reports, the routine chatter does not.
#[valtron(
    tracing = "info,answerme_agent=info,foundation_ai=info,mio=off,polling=off,llama-cpp-2=off,foundation_core::valtron::executors=error",
    tracing_targets = true,
    tracing_names = true
)]
fn main() {
    let cli = Cli::parse();
    let session = build_session();

    match cli.command.unwrap_or(Command::Agent) {
        Command::Ask { question } => run_ask(&session, question),
        Command::Agent => run_repl(&session),
    }
}

/// Build the llama.cpp-backed agent session shared by both commands.
fn build_session() -> Session {
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

    let model_id = ModelId::Name("unsloth/gemma-4-E2B-it-GGUF".into(), None);

    // Build the router preset and agent session.
    let preset = RouterMix::new().primary(provider, model_id.clone()).build();

    preset
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
        .expect("failed to build agent session")
}

/// One-shot: run a single turn and print the reply on stdout.
fn run_ask(session: &Session, question: String) {
    tracing::trace!("ask: sending question to model: {question}");

    match session.run_turn(user_message(question)) {
        Ok(records) => println!("{}", extract_assistant_text(&records)),
        Err(e) => {
            tracing::error!("ask turn failed: {e}");
            std::process::exit(1);
        }
    }
}

/// Interactive: drive the REPL loop, one turn per line of input.
fn run_repl(session: &Session) {
    let repl = Repl::builder()
        .prompt("| ")
        .continuation_prompt("|... ")
        .banner("answerme-agent — local Gemma session\nType /help for commands, /exit to quit.\n")
        .goodbye("Goodbye!")
        .build();

    repl.register_command("status", |_| "agent: running (gemma-4-E2B-it)".into());

    for input in repl.messages() {
        if input.is_empty() {
            continue;
        }

        tracing::trace!("agent: sending message to model: {input}");

        match session.run_turn(user_message(input)) {
            Ok(records) => repl.reply(&extract_assistant_text(&records)),
            Err(e) => repl.reply(&format!("error: {e}")),
        }
    }
}

/// Wrap raw input text as a user message for the session.
fn user_message(input: String) -> Messages {
    Messages::User {
        id: foundation_compact::ids::new_scru128(),
        role: foundation_ai::types::MessageRole::User,
        content: foundation_ai::types::UserModelContent::Text(TextContent {
            content: input,
            signature: None,
        }),
        signature: None,
    }
}

/// Extract the assistant's reply text from a list of session records.
fn extract_assistant_text(records: &[SessionRecord]) -> String {
    tracing::trace!("received {} records", records.len());
    let mut parts = Vec::new();
    for record in records {
        match record {
            SessionRecord::Conversation { message } => {
                if let Messages::Assistant { content, .. } = message {
                    tracing::debug!("assistant content: {content:?}");
                    if let ModelOutput::Text(text) = content {
                        parts.push(text.content.clone());
                    }
                } else if let Messages::User { content, .. } = message {
                    tracing::debug!("user message: {content:?}");
                }
            }
            SessionRecord::Summary { message_count, .. } => {
                tracing::debug!("summary: message_count={message_count}");
            }
            SessionRecord::FailedAction { error, .. } => {
                tracing::error!("failed action: {error}");
            }
            other => {
                tracing::debug!("other record: {other:?}");
            }
        }
    }
    if parts.is_empty() {
        "(no response)".into()
    } else {
        parts.join("\n")
    }
}
