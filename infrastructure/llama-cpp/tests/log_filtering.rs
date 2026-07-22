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
/// `tracing`'s `target:` argument needs a string literal, so `capture()` repeats
/// the spelling; `constant_matches_the_emitted_target` below pins the two
/// together so they cannot drift.
const NATIVE_LOG_TARGET: &str = "llama-cpp-2";

/// The directive `answerme-agent` ships in its `#[valtron(tracing = ...)]`.
/// Keep in sync with apps/answerme-agent/src/main.rs.
const SHIPPED_DIRECTIVE: &str = "info";

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
fn constant_matches_the_emitted_target() {
    // `capture()` must emit at exactly NATIVE_LOG_TARGET, or every other test
    // here would be asserting against a target the real logs never use. Proven
    // by filtering on the constant and observing the event disappear.
    let filtered = capture(&format!("info,{NATIVE_LOG_TARGET}=off"));
    assert!(
        !filtered.contains("NATIVE_MARKER"),
        "the constant must name the target capture() emits at: {filtered:?}"
    );
    // …and the event does appear when that directive is absent, so the check
    // above cannot pass vacuously.
    assert!(capture("info").contains("NATIVE_MARKER"));
}

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
fn the_shipped_directive_keeps_the_apps_own_logs() {
    // Guard the exact directive the app ships. It no longer needs to name the
    // native target at all — see `native_dump_is_quiet_under_a_plain_info_filter`
    // for why — but it must still let the app's own INFO through.
    let out = capture(SHIPPED_DIRECTIVE);
    assert!(
        out.contains("APP_MARKER"),
        "the shipped directive must keep the app's own INFO: {out:?}"
    );
}

#[test]
fn native_dump_is_quiet_under_a_plain_info_filter() {
    // The real contract, and the reason the app's directive shrank to "info":
    // llama.cpp's model-loader dump arrives at ggml INFO, which
    // `log::tracing_level_for` emits at tracing DEBUG. So a plain `info`
    // filter drops it without anyone naming `llama-cpp-2` at all.
    //
    // Emitted at DEBUG here to match what the bridge really does; the mapping
    // that guarantees it is unit-tested in `log::level_mapping_tests`.
    let buffer = Buffer::default();
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new("info"))
        .with_writer(buffer.clone())
        .with_ansi(false)
        .finish();

    tracing::subscriber::with_default(subscriber, || {
        tracing::debug!(target: "llama-cpp-2", module = "llama.cpp", "NATIVE_MARKER");
        tracing::warn!(target: "llama-cpp-2", module = "llama.cpp", "NATIVE_WARNING");
    });

    let out = buffer.contents();
    assert!(
        !out.contains("NATIVE_MARKER"),
        "the native dump must not survive a plain `info` filter: {out:?}"
    );
    assert!(
        out.contains("NATIVE_WARNING"),
        "a real native warning must still reach the user: {out:?}"
    );
}
