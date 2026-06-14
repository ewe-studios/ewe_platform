//! spec-44 — embedded Deno (deno_core + deno_web), in-process JS for the wasm harness.
//!
//! WHY: run the wasm test harness's JavaScript in-process with no external
//! `node`/`deno` binary — `cargo build` gives JS execution for free (spec-44).
//!
//! WHAT: a `JsRuntime` composed from `deno_core` + `deno_web` (+ `deno_webidl`),
//! NOT the full `deno_runtime` (its `deno_crypto` pins `aes =0.8.3`, which clashes
//! with turso's `aes 0.8.4` workspace-wide — see spec-44 §4.4). We give up
//! `crypto.subtle` (the harness doesn't use it), keeping everything else the
//! harness needs: ES modules, `WebAssembly`, `console`, `TextEncoder`/`TextDecoder`,
//! and timers. Plus two harness ops (`op_fwt_read_file`, `op_fwt_report`) that
//! replace the runner's `node:fs`/`fetch` byte loading and its stdout/exit-code
//! reporting with a direct Rust ↔ JS contract.
//!
//! HOW: `deno_core` already installs `console` and provides the module loader and
//! event loop. `deno_web` ships `TextEncoder`/`TextDecoder`/timers as
//! `lazy_loaded_js` IIFE scripts that it does NOT auto-install as globals (the full
//! `deno_runtime` bootstrap normally does that). So [`build_runtime`] runs a tiny
//! bootstrap that loads those scripts via `Deno.core.loadExtScript` and assigns the
//! classes onto `globalThis`.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use deno_core::convert::Uint8Array;
use deno_core::{
    op2, Extension, FsModuleLoader, JsRuntime, OpState, PollEventLoopOptions, RuntimeOptions,
};
use deno_error::JsErrorBox;

/// Installs the Web globals `deno_web` provides but does not auto-expose.
///
/// `deno_web` ships these as `lazy_loaded_js` IIFE modules (`08_text_encoding.js`,
/// `02_timers.js`), loaded on demand via `Deno.core.loadExtScript`, which returns
/// the module's export object. The scripts pull their own transitive deps
/// (`deno_webidl`) and read `__bootstrap` (provided by `deno_core`) themselves.
const BOOTSTRAP_GLOBALS: &str = r#"
((globalThis) => {
  const load = globalThis.Deno.core.loadExtScript;
  const enc = load("ext:deno_web/08_text_encoding.js");
  const timers = load("ext:deno_web/02_timers.js");
  const url = load("ext:deno_web/00_url.js");
  Object.assign(globalThis, {
    TextEncoder: enc.TextEncoder,
    TextDecoder: enc.TextDecoder,
    TextEncoderStream: enc.TextEncoderStream,
    TextDecoderStream: enc.TextDecoderStream,
    setTimeout: timers.setTimeout,
    setInterval: timers.setInterval,
    clearTimeout: timers.clearTimeout,
    clearInterval: timers.clearInterval,
    URL: url.URL,
    URLSearchParams: url.URLSearchParams,
  });
})(globalThis);
"#;

// ── Harness ops (Rust ↔ JS bridge) ──────────────────────────────────────────

/// Read a file's bytes — replaces the runner's `node:fs`/`fetch` byte loading.
/// The runner passes the local path of a staged file (`module.wasm`/`cases.json`).
#[op2]
fn op_fwt_read_file(#[string] path: String) -> Result<Uint8Array, JsErrorBox> {
    std::fs::read(&path)
        .map(Uint8Array::from)
        .map_err(|e| JsErrorBox::generic(format!("op_fwt_read_file({path}): {e}")))
}

/// Receive the runner's final result as JSON — replaces stdout parsing +
/// `process.exit`/`Deno.exit` (neither exists in this runtime). Captured into the
/// shared [`ReportSink`] so the driver reads it after the event loop drains.
#[op2(fast)]
fn op_fwt_report(state: &mut OpState, #[string] json: String) {
    let sink = state.borrow::<ReportSink>();
    *sink.0.borrow_mut() = Some(json);
}

/// Shared cell the report op writes into; the driver keeps a clone to read out.
#[derive(Default, Clone)]
struct ReportSink(Rc<RefCell<Option<String>>>);

deno_core::extension!(
    foundation_fwt,
    ops = [op_fwt_read_file, op_fwt_report],
    options = { sink: ReportSink },
    state = |state, options| {
        state.put(options.sink);
    },
);

/// The runner's final result, captured via `op_fwt_report`.
#[derive(Debug, serde::Deserialize)]
pub struct HarnessReport {
    /// Cases that passed.
    pub passed: u32,
    /// Cases that failed.
    pub failed: u32,
    /// Cases that were ignored.
    pub ignored: u32,
    /// The per-case + summary lines the runner produced.
    pub output: String,
}

impl HarnessReport {
    /// 0 when every case passed, 1 otherwise — the runner's process-exit semantics.
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        i32::from(self.failed != 0)
    }
}

// ── Runtime construction + drivers ──────────────────────────────────────────

/// Build a `JsRuntime` with the harness's Web platform, an FS module loader, and
/// any `extra` extensions, then install the globals `deno_web` doesn't expose.
fn build_runtime_with(extra: Vec<Extension>) -> Result<JsRuntime, Box<dyn std::error::Error>> {
    let mut extensions = vec![
        deno_webidl::deno_webidl::init(),
        deno_web::deno_web::init(
            Arc::new(deno_web::BlobStore::default()),
            None,  // maybe_location
            false, // enable_css_parser_features
            deno_web::InMemoryBroadcastChannel::default(),
        ),
    ];
    extensions.extend(extra);

    let mut runtime = JsRuntime::new(RuntimeOptions {
        module_loader: Some(Rc::new(FsModuleLoader)),
        extensions,
        ..Default::default()
    });
    runtime.execute_script("ext:foundation_testbed/bootstrap.js", BOOTSTRAP_GLOBALS)?;
    Ok(runtime)
}

/// Build a `JsRuntime` with the harness's Web platform and an FS module loader,
/// then install the globals `deno_web` doesn't expose on its own.
///
/// The runtime resolves ES module imports from disk (the harness stages itself to
/// a temp dir), so relative imports like `./foundation-wasm.js` resolve naturally.
///
/// # Errors
/// Fails if the globals bootstrap script throws (a `deno_web` API drift would show
/// up here).
pub fn build_runtime() -> Result<JsRuntime, Box<dyn std::error::Error>> {
    build_runtime_with(vec![])
}

/// Load + evaluate an ES module file in-process and drive the event loop to
/// completion (so `setTimeout`/microtasks/async `import`s settle).
///
/// # Errors
/// Fails on module resolution/load errors, an exception during evaluation, or an
/// event-loop error.
pub fn run_module(entry: &Path) -> Result<(), Box<dyn std::error::Error>> {
    tokio_runtime()?.block_on(async {
        let mut runtime = build_runtime_with(vec![])?;
        let id = load_entry(&mut runtime, entry).await?;
        evaluate(&mut runtime, id).await
    })
}

/// Run a staged `#[wasm_test]` harness (`runner.mjs` + `foundation-wasm.js` +
/// `module.wasm` + `cases.json`) in-process and return its captured result.
///
/// This is the embedded replacement for shelling out to `node`/`deno`: it builds
/// the runtime with the harness ops, evaluates `runner.mjs`, drives the event loop,
/// and reads the report the runner pushed through `op_fwt_report`.
///
/// # Errors
/// Fails on module/evaluation/event-loop errors, if the runner never reported, or
/// if the report JSON is malformed.
pub fn run_staged_harness(stage_dir: &Path) -> Result<HarnessReport, Box<dyn std::error::Error>> {
    let sink = ReportSink::default();
    let captured = sink.clone();
    let runner = stage_dir.join("runner.mjs");

    tokio_runtime()?.block_on(async {
        let mut runtime = build_runtime_with(vec![foundation_fwt::init(sink)])?;
        let id = load_entry(&mut runtime, &runner).await?;
        evaluate(&mut runtime, id).await
    })?;

    let json = captured
        .0
        .borrow_mut()
        .take()
        .ok_or("harness finished without calling op_fwt_report")?;
    Ok(serde_json::from_str(&json)?)
}

/// A current-thread tokio runtime with the time driver — the minimal executor
/// deno_core's event loop + timers (`reactor_tokio.rs`) require.
fn tokio_runtime() -> Result<tokio::runtime::Runtime, Box<dyn std::error::Error>> {
    Ok(tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()?)
}

async fn load_entry(
    runtime: &mut JsRuntime,
    entry: &Path,
) -> Result<deno_core::ModuleId, Box<dyn std::error::Error>> {
    let specifier = deno_core::resolve_path(&entry.to_string_lossy(), &std::env::current_dir()?)?;
    Ok(runtime.load_main_es_module(&specifier).await?)
}

async fn evaluate(
    runtime: &mut JsRuntime,
    id: deno_core::ModuleId,
) -> Result<(), Box<dyn std::error::Error>> {
    let evaluate = runtime.mod_evaluate(id);
    runtime
        .run_event_loop(PollEventLoopOptions::default())
        .await?;
    evaluate.await?;
    Ok(())
}

// ── W1 spike (kept: smallest possible build/eval/WebAssembly proof) ──────────

/// Evaluate `1 + 1` in an in-process V8 isolate and return the number (W1 spike).
#[must_use]
pub fn spike_eval_addition() -> f64 {
    let mut rt = JsRuntime::new(RuntimeOptions::default());
    let value = rt.execute_script("<spike>", "1 + 1").expect("execute_script");
    deno_core::scope!(scope, rt);
    let local = deno_core::v8::Local::new(scope, value);
    local.number_value(scope).unwrap_or(f64::NAN)
}

/// Synchronously compile + instantiate a minimal (header-only) wasm module in V8 —
/// proves the engine's `WebAssembly` works (the harness needs it) (W1 spike).
#[must_use]
pub fn spike_wasm_instantiate() -> bool {
    let mut rt = JsRuntime::new(RuntimeOptions::default());
    let src = "(() => {\
        const bytes = new Uint8Array([0,97,115,109,1,0,0,0]);\
        const m = new WebAssembly.Module(bytes);\
        const i = new WebAssembly.Instance(m);\
        return typeof i === 'object';\
    })()";
    let value = rt.execute_script("<spike-wasm>", src).expect("execute_script");
    deno_core::scope!(scope, rt);
    let local = deno_core::v8::Local::new(scope, value);
    local.boolean_value(scope)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_v8_evaluates_js() {
        assert_eq!(spike_eval_addition(), 2.0, "in-process V8 evaluated 1 + 1");
    }

    #[test]
    fn embedded_v8_runs_webassembly() {
        assert!(spike_wasm_instantiate(), "in-process V8 instantiated a wasm module");
    }

    /// W2: the FS module loader resolves a relative-import ESM graph, and the
    /// deno_web globals (TextEncoder/TextDecoder) + timers (setTimeout, driven by
    /// the event loop) all work in-process — the capabilities the harness needs.
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

    /// W3: run the real `fwt_sample.wasm` fixture's five `#[wasm_test]` cases
    /// end-to-end through the embedded runtime — no external node/deno. The fixture
    /// has a deliberate failure, so the verdict is red (3 passed, 1 failed, 1
    /// ignored), with `should_panic` inverted and the async case completing.
    #[test]
    fn embedded_runs_sample_fixture_end_to_end() {
        let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("integration/fixtures/fwt_sample.wasm");
        if !fixture.is_file() {
            eprintln!("fixture not built (run integration/build-module.sh) — skipping");
            return;
        }

        let cases = crate::wasm::fwt::discover_cases(&fixture).expect("discovery");
        assert_eq!(cases.len(), 5);

        let dir = tempfile::tempdir().expect("tempdir");
        crate::wasm::fwt_runner::stage_from_wasm(&fixture, &cases, dir.path())
            .expect("stage from prebuilt wasm");

        let report = run_staged_harness(dir.path()).expect("embedded harness ran");

        assert_eq!(report.passed, 3, "output: {}", report.output);
        assert_eq!(report.failed, 1, "output: {}", report.output);
        assert_eq!(report.ignored, 1, "output: {}", report.output);
        assert_eq!(report.exit_code(), 1);
        assert!(report.output.contains("FAILED  fails_with_assertion"));
        assert!(report.output.contains("ok      passes_simple"));
        assert!(
            report.output.contains("ok      panics_as_expected"),
            "should_panic is inverted"
        );
        assert!(report.output.contains("ok      async_completes_after_yield"));
        assert!(report.output.contains("ignored ignored_case"));
        assert!(report
            .output
            .contains("test result: FAILED. 3 passed; 1 failed; 1 ignored"));
    }
}
