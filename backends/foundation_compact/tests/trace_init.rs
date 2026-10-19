//! `trace::try_init_tracing_with`: one process-wide subscriber whose filter each
//! call sets, so a `#[valtron_test(tracing = "...")]` gets the level it asked for
//! however many tests initialised tracing before it.
#![cfg(not(all(target_family = "wasm", any(target_os = "unknown", target_os = "none"))))]

use foundation_compact::trace::{try_init_tracing, try_init_tracing_with, TracingTestConfig};
use tracing::Level;

fn with_filter(filter: &str) -> TracingTestConfig {
    TracingTestConfig {
        env_filter: Some(filter.to_string()),
        ..TracingTestConfig::default()
    }
}

/// One test, so no other test in this binary changes the filter mid-way.
#[test]
fn every_call_sets_the_filter_and_later_calls_never_panic() {
    try_init_tracing_with(with_filter("warn"));
    assert!(tracing::enabled!(Level::WARN));
    assert!(!tracing::enabled!(Level::INFO), "first call's filter applies");

    try_init_tracing_with(with_filter("debug"));
    assert!(
        tracing::enabled!(Level::DEBUG),
        "a later call's filter applies too, not only the first"
    );
    assert!(!tracing::enabled!(Level::TRACE));

    try_init_tracing_with(with_filter("error"));
    assert!(!tracing::enabled!(Level::WARN));

    // Default config: RUST_LOG, falling back to info.
    try_init_tracing();
    if std::env::var_os("RUST_LOG").is_none() {
        assert!(tracing::enabled!(Level::INFO));
        assert!(!tracing::enabled!(Level::DEBUG));
    }

    // Events go through the subscriber without panicking from a spawned thread.
    std::thread::spawn(|| tracing::info!("from a spawned thread"))
        .join()
        .expect("spawned thread");
}
