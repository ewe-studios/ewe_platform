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
    AgentConfig, AgentSession, Answer, ContextConfig, ErrorPolicy, MemoryConfig,
};
use foundation_ai::backends::huggingface_gguf_provider::{
    HuggingFaceGGUFConfig, HuggingFaceGGUFProvider,
};
use foundation_ai::backends::llamacpp::{LlamaBackendConfig, LlamaBackends};
use foundation_ai::harness::RouterMix;
use foundation_ai::types::ModelId;
use foundation_core::valtron::valtron;
use foundation_repl::{Repl, ReplTheme};

/// The concrete session type — the in-memory stores, so no type parameters.
type Session = AgentSession;

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

// A plain `info` is all this needs, because the noisy libraries now log at
// levels that respect it rather than requiring per-app silencing:
//
//   * llama.cpp/ggml native logs — their INFO is a 165-line model-loader dump,
//     so `infrastructure_llama_cpp::log::tracing_level_for` emits it at DEBUG
//     (ggml WARN is also demoted to DEBUG so a plain `info` filter drops it;
//     only a real ERROR survives). This used to need `llama-cpp-2=off`, and note
//     that the target really is the literal `llama-cpp-2`: `llama.cpp` and
//     `ggml` are only the `module` FIELD on those events, so directives naming
//     them match nothing. Locked by infrastructure/llama-cpp/tests/log_filtering.rs.
//   * valtron pool lifecycle — worker start/stop, shutdown, registry clearing
//     are at DEBUG; only a worker panic is loud, and it is an ERROR.
//   * mio/polling — quiet at INFO on their own.
//
// Measured on this directive: 0 stderr lines around a one-line answer.
//
// To see the native dump, use `debug` here and rebuild. RUST_LOG will NOT do
// it: `try_init_tracing_with` only reads RUST_LOG when no explicit
// `tracing = "..."` is given (foundation_compact/src/trace/mod.rs), and this
// entry point always supplies one.
#[valtron(tracing = "info", tracing_targets = true, tracing_names = true)]
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
        .into_agent_builder()
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

    match session.ask(question) {
        Ok(Answer::Complete(text)) => println!("{}", or_no_response(&text)),
        // Partial output first, then the error that ended the turn.
        Ok(Answer::Failed {
            partial_text,
            error,
            ..
        }) => {
            if !partial_text.is_empty() {
                println!("{partial_text}");
            }
            tracing::error!("ask turn failed: {error}");
            std::process::exit(1);
        }
        Err(e) => {
            tracing::error!("ask turn could not start: {e}");
            std::process::exit(1);
        }
    }
}

/// Interactive: drive the REPL loop, one turn per line of input.
///
/// A local Gemma turn takes seconds, so it runs behind an activity indicator —
/// without one the terminal is indistinguishable from a hung process for the
/// whole generation.
fn run_repl(session: &Session) {
    let repl = Repl::builder()
        .prompt("| ")
        .continuation_prompt("|... ")
        .banner("answerme-agent — local Gemma session\nType /help for commands, /exit to quit.")
        .goodbye("Goodbye!")
        .theme(ReplTheme::from_env("ANSWERME_THEME"))
        .build();

    repl.register_command("status", |_| "agent: running (gemma-4-E2B-it)".into());

    for input in repl.messages() {
        if input.trim().is_empty() {
            continue;
        }

        tracing::trace!("agent: sending message to model: {input}");

        let thinking = repl.animation("thinking");
        let outcome = session.ask(input);
        thinking.finish();

        // `report_error` styles errors as errors rather than as the model's
        // own words, which `reply` would have done.
        match outcome {
            Ok(Answer::Complete(text)) => repl.reply(or_no_response(&text)),
            Ok(Answer::Failed {
                partial_text,
                error,
                ..
            }) => {
                if !partial_text.is_empty() {
                    repl.reply(partial_text);
                }
                repl.report_error(format!("error: {error}"));
            }
            Err(e) => repl.report_error(format!("error: {e}")),
        }
    }
}

/// The reply text, or a placeholder when the model said nothing.
fn or_no_response(text: &str) -> String {
    if text.is_empty() {
        "(no response)".into()
    } else {
        text.to_string()
    }
}
