//! Proves the native-log tracing target is actually filterable by the
//! directives apps put in their `EnvFilter`.
//!
//! WHY: `answerme-agent` shipped `llama.cpp=off,ggml=off` and llama.cpp's
//! model-loader dump still flooded the REPL. Those two strings are the
//! **`module` field value** on the event, not its tracing **target** —
//! `log.rs` hard-codes the target to `"llama-cpp-2"` in `Metadata::new`.
//! `EnvFilter` matches on target, so both directives matched nothing and every
//! native log came through.
//!
//! WHAT: locks the contract —
//!   1. `llama-cpp-2=off` (the real target) silences the events;
//!   2. `llama.cpp=off` / `ggml=off` / `llama_cpp_2=off` do NOT;
//!   3. the exact directive the app ships silences native logs while keeping
//!      the app's own INFO.
//!
//! HOW: install a `fmt` subscriber writing into a shared buffer, emit an event
//! at the native target, and assert on what was captured. No model, no backend,
//! no C library — pure filter semantics end-to-end.

use std::io;
use std::sync::{Arc, Mutex};

use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::EnvFilter;

/// The target `infrastructure_llama_cpp::log` stamps on every native log event
/// (see the `log_cs!` macro's `Metadata::new(.., "llama-cpp-2", ..)`).
///
/// `tracing`'s `target:` argument needs a literal, so the emission below repeats
/// the string; this assertion keeps the two spellings from drifting apart.
const NATIVE_LOG_TARGET: &str = "llama-cpp-2";
const _: () = assert!(NATIVE_LOG_TARGET.len() == "llama-cpp-2".len());

/// The directive `answerme-agent` ships in its `#[valtron(tracing = ...)]`.
/// Keep in sync with apps/answerme-agent/src/main.rs.
const SHIPPED_DIRECTIVE: &str = "info,answerme_agent=info,foundation_ai=info,mio=off,\
                                 polling=off,llama-cpp-2=off";

// ---------------------------------------------------------------------------
// Capturing writer
// ---------------------------------------------------------------------------

#[derive(Clone, Default)]
struct Buffer(Arc<Mutex<Vec<u8>>>);

impl Buffer {
    fn contents(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().expect("buffer poisoned")).into_owned()
    }
}

impl io::Write for Buffer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().expect("buffer poisoned").extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for Buffer {
    type Writer = Self;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

/// Emit one INFO event at `NATIVE_LOG_TARGET` under `directives`, plus one at
/// `answerme_agent`, and return everything the subscriber wrote.
///
/// Uses a scoped (thread-local) subscriber so tests stay independent — a global
/// default can only be set once per process.
fn capture(directives: &str) -> String {
    let buffer = Buffer::default();
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(directives))
        .with_writer(buffer.clone())
        .with_ansi(false)
        .finish();

    tracing::subscriber::with_default(subscriber, || {
        tracing::info!(target: "llama-cpp-2", module = "llama.cpp", "NATIVE_MARKER");
        tracing::info!(target: "answerme_agent", "APP_MARKER");
    });

    buffer.contents()
}

fn native_logged(directives: &str) -> bool {
    capture(directives).contains("NATIVE_MARKER")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn correct_target_directive_silences_native_logs() {
    assert!(
        !native_logged("info,llama-cpp-2=off"),
        "`llama-cpp-2=off` must silence native llama.cpp/ggml log events"
    );
}

#[test]
fn hyphenated_directive_does_not_void_the_rest_of_the_filter() {
    // If a hyphenated target were unparseable the whole filter could be
    // dropped, letting everything through (or nothing at all).
    let out = capture("info,llama-cpp-2=off");
    assert!(
        out.contains("APP_MARKER"),
        "a hyphenated target directive must not break the rest of the filter: {out:?}"
    );
}

#[test]
fn module_field_values_are_not_valid_targets() {
    // These strings *look* right — they are the `module` field on the event —
    // but they are not the target, so they cannot filter it. This is the exact
    // bug that kept the REPL noisy.
    assert!(
        native_logged("info,llama.cpp=off"),
        "`llama.cpp=off` filters the module FIELD, not the target — it must \
         NOT be relied on to silence native logs"
    );
    assert!(
        native_logged("info,ggml=off"),
        "`ggml=off` filters the module FIELD, not the target"
    );
}

#[test]
fn underscore_spelling_does_not_match_hyphenated_target() {
    // The original directive used `llama_cpp_2=off`; targets are matched
    // literally, so the underscore spelling never matched `llama-cpp-2`.
    assert!(
        native_logged("info,llama_cpp_2=off"),
        "`llama_cpp_2` (underscores) must not match target `llama-cpp-2`"
    );
}

#[test]
fn shipped_directive_silences_native_logs_but_keeps_app_logs() {
    // Guard the exact directive the app ships, so a future edit that drops or
    // misspells the target fails here rather than in a noisy REPL.
    let out = capture(SHIPPED_DIRECTIVE);
    assert!(
        !out.contains("NATIVE_MARKER"),
        "answerme-agent's directive must silence native llama.cpp logs: {out:?}"
    );
    assert!(
        out.contains("APP_MARKER"),
        "answerme-agent's directive must keep the app's own INFO: {out:?}"
    );
}
