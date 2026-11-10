//! End-to-end tests for `WasmtimeResponder` with real WAT modules.
//!
//! WHY: the responder must return the body that the module's `handle_request`
//! export produces. It must return an error response when the export is
//! missing, and it must not return a fixed placeholder string.
//!
//! WHAT: each test builds a `WasmtimeResponder` from a small WAT module, calls
//! `respond`, and checks the status and body.
//!
//! HOW: `wat::parse_str` turns the text module into the bytes that
//! `WasmtimeBuilder` compiles. `PlatformSession::new_test` builds a session
//! with a real injected clock.

#![cfg(all(
    feature = "wasmtime",
    not(any(target_os = "ios", target_os = "tvos", target_os = "watchos"))
))]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use foundation_core::valtron::{Ctx, MonotonicClock, SimulatedClock};
use foundation_platform::*;
use foundation_wasmtime::WasmtimeBuilder;
use tauri::http::{header, StatusCode};

/// A module whose `handle_request` echoes the request path.
const ECHO_MODULE: &str = r#"
(module
  (memory (export "memory") 1)
  (func (export "ewe_alloc") (param $size i32) (result i32)
    i32.const 1024)
  (func (export "handle_request") (param $ptr i32) (param $len i32) (result i64)
    (i64.or
      (i64.shl (i64.extend_i32_u (local.get $ptr)) (i64.const 32))
      (i64.extend_i32_u (local.get $len)))))
"#;

/// A module that has memory and an allocator but no `handle_request` export.
const NO_HANDLER_MODULE: &str = r#"
(module
  (memory (export "memory") 1)
  (func (export "ewe_alloc") (param $size i32) (result i32)
    i32.const 1024))
"#;

fn session() -> Arc<PlatformSession> {
    PlatformSession::new_test(
        PathBuf::from("."),
        foundation_core::valtron::Ctx::fixed(std::sync::Arc::new(
            foundation_core::valtron::MonotonicClock::new(),
        )),
    )
}

fn intent(url: &str) -> NavigationIntent {
    NavigationIntent {
        url: url.to_string(),
        method: Method::Get,
        source: IntentSource::LinkClick,
        referrer: None,
    }
}

fn responder(module: &str) -> WasmtimeResponder {
    responder_with_cap(module, WasmtimeBuilder::DEFAULT_MAX_RESPONSE_BYTES)
}

fn responder_with_cap(module: &str, max_response_bytes: usize) -> WasmtimeResponder {
    let bytes = wat::parse_str(module).expect("the WAT source parses");
    // These modules read no clock and no randomness, so a plain bundle is
    // enough. The session-path test below covers the injected values.
    let ctx = Ctx::fixed(Arc::new(MonotonicClock::new()));
    WasmtimeResponder::new(
        WasmtimeBuilder::new(bytes)
            .with_name("app_shell")
            .with_max_response_bytes(max_response_bytes),
        ctx,
    )
}

#[test]
fn responds_with_the_module_body() {
    let session = session();
    let responder = responder(ECHO_MODULE);
    let response = responder.respond(
        &intent("ewe://localhost/shell/items/42"),
        &wasmtime_app("app_shell"),
        &session,
    );

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.body().as_slice(), b"/shell/items/42");
    assert_eq!(
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("text/plain; charset=utf-8")
    );
}

#[test]
fn missing_export_returns_the_error_response() {
    let session = session();
    let responder = responder(NO_HANDLER_MODULE);
    let response = responder.respond(
        &intent("ewe://localhost/shell/items"),
        &wasmtime_app("app_shell"),
        &session,
    );

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = std::str::from_utf8(response.body()).expect("the error body is UTF-8");
    assert!(
        body.contains("WASM export not found: handle_request"),
        "unexpected body: {body}"
    );
}

#[test]
fn the_builder_response_cap_applies_to_the_body() {
    let session = session();
    let responder = responder_with_cap(ECHO_MODULE, 4);
    let response = responder.respond(
        &intent("ewe://localhost/shell/items"),
        &wasmtime_app("app_shell"),
        &session,
    );

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = std::str::from_utf8(response.body()).expect("the error body is UTF-8");
    assert!(
        body.contains("exceeds the maximum of 4 bytes"),
        "unexpected body: {body}"
    );
}

/// A module whose `handle_request` returns the realtime clock in nanoseconds.
const CLOCK_MODULE: &str = r#"
(module
  (import "wasi_snapshot_preview1" "clock_time_get"
    (func $clock_time_get (param i32 i64 i32) (result i32)))
  (memory (export "memory") 1)
  (func (export "ewe_alloc") (param $size i32) (result i32)
    i32.const 1024)
  (func (export "handle_request") (param $ptr i32) (param $len i32) (result i64)
    ;; REALTIME = 0, one nanosecond of precision, result at offset 0.
    (drop (call $clock_time_get (i32.const 0) (i64.const 1) (i32.const 0)))
    (i64.const 8)))
"#;

/// Regression test for issue #426 at the session boundary.
///
/// WHY: the session must hand its injected bundle to the wasmtime shell. If the
/// shell built its own host bundle, the guest would read the real wall clock
/// and this assertion would fail.
#[test]
fn the_session_clock_reaches_the_guest() {
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    clock.advance_wall(Duration::from_secs(1_700_000_000));
    let session = PlatformSession::new_test(PathBuf::from("."), Ctx::fixed(clock));

    let bytes = wat::parse_str(CLOCK_MODULE).expect("the WAT source parses");
    session.register_wasmtime("clock", WasmtimeBuilder::new(bytes));
    let responder = session
        .wasmtime_shell()
        .get("clock")
        .expect("the app is registered");

    let response = responder.respond(
        &intent("ewe://localhost/clock"),
        &wasmtime_app("clock"),
        &session,
    );

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.body().as_slice(),
        1_700_000_000u64.saturating_mul(1_000_000_000).to_le_bytes(),
        "the guest must read the session's clock, not the host clock"
    );
}
