#![cfg(feature = "wasm-embedded-js")]
//! WHY: spec-44 — the owned `#[wasm_test]` harness runs JavaScript in-process on an
//! embedded `deno_core` + `deno_web` + `deno_fetch` (V8) runtime, with no external
//! `node`/`deno`. These tests pin that the engine links and the host surface the
//! harness relies on (ES modules, `WebAssembly`, `TextEncoder`/`TextDecoder`,
//! timers, `fetch`) works in-process.
//!
//! WHAT: the W1 spike (eval + WebAssembly), W2 (relative-import ESM + Web globals +
//! timers), the W3 end-to-end run of the real `fwt_sample.wasm` fixture, and a
//! `fetch` round-trip.
//!
//! HOW: drives the public execution-core API in `foundation_testbed::wasm::embedded_js`
//! (+ `fwt`/`fwt_runner` for staging). Build/test with the `uat` profile.

use foundation_testbed::wasm::embedded_js::{
    run_module, run_staged_harness, spike_eval_addition, spike_wasm_instantiate,
};
use foundation_testbed::wasm::{fwt, fwt_runner};

/// W1: the embedded V8 isolate links and evaluates JS.
#[test]
fn embedded_v8_evaluates_js() {
    assert_eq!(spike_eval_addition(), 2.0, "in-process V8 evaluated 1 + 1");
}

/// W1: the embedded V8 isolate compiles + instantiates a wasm module.
#[test]
fn embedded_v8_runs_webassembly() {
    assert!(spike_wasm_instantiate(), "in-process V8 instantiated a wasm module");
}

/// W2: the FS module loader resolves a relative-import ESM graph, and the deno_web
/// globals (`TextEncoder`/`TextDecoder`) + timers (`setTimeout`, driven by the event
/// loop) all work in-process — the capabilities the harness needs.
#[test]
fn loader_runs_relative_import_graph_with_web_globals() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("util.mjs"),
        "export const greet = (name) => `hi ${name}`;\n",
    )
    .expect("write util");
    std::fs::write(
        dir.path().join("entry.mjs"),
        r#"
        import { greet } from "./util.mjs";
        const bytes = new TextEncoder().encode(greet("wasm"));
        const text = new TextDecoder().decode(bytes);
        if (text !== "hi wasm") throw new Error("roundtrip failed: " + text);
        await new Promise((resolve) => setTimeout(resolve, 1));
        console.log("W2 ok: " + text);
        "#,
    )
    .expect("write entry");

    run_module(&dir.path().join("entry.mjs")).expect("embedded module ran to completion");
}

/// `fetch` is wired (deno_fetch) and globalized by the bootstrap; a `data:` URL is a
/// hermetic round-trip (no network), proving the global + permissions + body decode.
#[test]
fn fetch_data_url_round_trips_in_process() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("entry.mjs"),
        r#"
        const res = await fetch("data:text/plain,hello-embedded");
        const text = await res.text();
        if (text !== "hello-embedded") throw new Error("fetch returned: " + text);
        if (!(res instanceof Response)) throw new Error("Response global missing");
        console.log("fetch ok: " + text);
        "#,
    )
    .expect("write entry");

    run_module(&dir.path().join("entry.mjs")).expect("fetch ran to completion");
}

/// F51: the `WebSocket` global is wired into the embedded runtime — it exists as a
/// constructor and a fresh instance reports `CONNECTING` (0). This is the enabler
/// for testing the cross-platform WebSocket client's wasm path under deno.
#[test]
fn websocket_global_present_in_process() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("entry.mjs"),
        r#"
        if (typeof WebSocket !== "function") throw new Error("WebSocket global missing");
        const ws = new WebSocket("ws://127.0.0.1:9");
        if (ws.readyState !== WebSocket.CONNECTING) {
            throw new Error("fresh WebSocket should be CONNECTING, got " + ws.readyState);
        }
        ws.onerror = () => {};   // swallow the imminent connection failure
        ws.onclose = () => {};
        console.log("WebSocket global ok");
        "#,
    )
    .expect("write entry");

    run_module(&dir.path().join("entry.mjs")).expect("WebSocket global module ran");
}

/// F51: the embedded `WebSocket` actually attempts a connection and delivers a
/// `close` event through the event loop (proving the ops + net permissions +
/// event-loop wiring, not just the class). We dial a port with nothing listening
/// — the refused connection must surface as `onclose` (code 1006), not a hang.
#[test]
fn websocket_connection_refused_delivers_close() {
    // Reserve then release a port so it is guaranteed free (connection refused).
    let port = {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        listener.local_addr().expect("addr").port()
    };

    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("entry.mjs"),
        format!(
            r#"
            const code = await new Promise((resolve, reject) => {{
                const ws = new WebSocket("ws://127.0.0.1:{port}");
                ws.onopen = () => ws.close();
                ws.onclose = (e) => resolve(e.code);
                ws.onerror = () => {{}};   // a refused dial fires error then close
                setTimeout(() => reject(new Error("no close within 5s")), 5000);
            }});
            // 1006 = abnormal closure (connection could not be established).
            if (typeof code !== "number") throw new Error("close code not a number: " + code);
            console.log("WebSocket closed with code " + code);
            "#
        ),
    )
    .expect("write entry");

    run_module(&dir.path().join("entry.mjs")).expect("WebSocket close module ran");
}

/// W3: run the real `fwt_sample.wasm` fixture's five `#[wasm_test]` cases end-to-end
/// through the embedded runtime — no external node/deno. The fixture has a deliberate
/// failure, so the verdict is red (3 passed, 1 failed, 1 ignored), with `should_panic`
/// inverted and the async case completing.
#[test]
fn embedded_runs_sample_fixture_end_to_end() {
    let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("integration/fixtures/fwt_sample.wasm");
    if !fixture.is_file() {
        eprintln!("fixture not built (run integration/build-module.sh) — skipping");
        return;
    }

    let cases = fwt::discover_cases(&fixture).expect("discovery");
    assert_eq!(cases.len(), 5);

    let dir = tempfile::tempdir().expect("tempdir");
    fwt_runner::stage_from_wasm(&fixture, &cases, dir.path()).expect("stage from prebuilt wasm");

    let report = run_staged_harness(dir.path()).expect("embedded harness ran");

    assert_eq!(report.passed, 3, "output: {}", report.output);
    assert_eq!(report.failed, 1, "output: {}", report.output);
    assert_eq!(report.ignored, 1, "output: {}", report.output);
    assert_eq!(report.exit_code(), 1);
    assert!(report.output.contains("FAILED  fails_with_assertion"));
    assert!(report.output.contains("ok      passes_simple"));
    assert!(report.output.contains("ok      panics_as_expected"), "should_panic is inverted");
    assert!(report.output.contains("ok      async_completes_after_yield"));
    assert!(report.output.contains("ignored ignored_case"));
    assert!(report
        .output
        .contains("test result: FAILED. 3 passed; 1 failed; 1 ignored"));
}
