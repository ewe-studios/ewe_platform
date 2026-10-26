//! Tests for the no_std tracing subscriber (`foundation_nostd::trace`).

#![cfg(feature = "trace")]

use std::sync::Mutex;

use foundation_nostd::trace::{set_sink, try_init};
use tracing::Level;

/// Captured lines, one per event, joined by newlines.
static CAPTURED: Mutex<String> = Mutex::new(String::new());

fn capture(line: &str) {
    CAPTURED.lock().unwrap().push_str(line);
    CAPTURED.lock().unwrap().push('\n');
}

#[test]
fn forwards_enabled_events_and_filters_levels() {
    set_sink(capture);
    try_init(Level::INFO).expect("install the subscriber");

    // Below the max level: dropped.
    tracing::trace!(target: "hidden", "trace is filtered out");
    // At or above: forwarded.
    tracing::info!(target: "test", answer = 42u64, name = "bob", "a message");

    let out = CAPTURED.lock().unwrap().clone();
    assert!(!out.contains("trace is filtered out"), "trace! leaked: {out}");
    assert!(out.contains("INFO test:"), "level/target missing: {out}");
    assert!(out.contains("answer=42"), "i64 field missing: {out}");
    assert!(out.contains("name=\"bob\""), "str field missing: {out}");
    // The message field is a `fmt::Arguments` value, so its `Debug` (and this
    // visitor) print it unquoted, unlike the `&str` `name` field.
    assert!(out.contains("message=a message"), "message field missing: {out}");
}
