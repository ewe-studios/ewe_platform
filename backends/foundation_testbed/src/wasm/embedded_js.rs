//! spec-44 W1 spike — prove an embedded Deno/V8 runtime runs JS (and
//! `WebAssembly`) in-process, with no external `node`/`deno` binary.
//!
//! This is the go/no-go gate for spec-44: if `deno_core` (V8 via rusty_v8) builds
//! and links on our toolchain and runs these, the embedding path is viable.

use deno_core::{JsRuntime, RuntimeOptions};

/// Evaluate `1 + 1` in an in-process V8 isolate and return the number.
#[must_use]
pub fn spike_eval_addition() -> f64 {
    let mut rt = JsRuntime::new(RuntimeOptions::default());
    let value = rt.execute_script("<spike>", "1 + 1").expect("execute_script");
    deno_core::scope!(scope, rt);
    let local = deno_core::v8::Local::new(scope, value);
    local.number_value(scope).unwrap_or(f64::NAN)
}

/// Synchronously compile + instantiate a minimal (header-only) wasm module in V8 —
/// proves the engine's `WebAssembly` works (the harness needs it).
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
}
