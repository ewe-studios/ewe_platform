# Feature 14 — Status: COMPLETE (2026-06-11)

| Criterion | Evidence |
|---|---|
| Opt-in only; no default path invokes wasm-bindgen | Owned `node`/`deno`/`web` commands never touch it. The `test bindgen-*` modes are the explicit opt-in and now log an `INTEROP MODE` warning. The biggest hidden default-path leak — foundation_core's unconditional getrandom `wasm_js`/`js` features pulling `__wbindgen_*` imports into EVERY wasm32 build — is fixed: those backends are now enabled only by the `js-wasmbindgen` feature (see F12 status). |
| worker-rs interop via a thin shim | `foundation_db`'s `worker` dep (Cloudflare workers-rs, D1) is optional + feature-gated, confined to its wasm module — app logic stays on owned types at the seam. The testbed's `bindgen-wrangler` mode + templates remain the sanctioned harness for such crates. |
| Representative SDK seam | `foundation_db/src/wasm/*`: wasm-bindgen/worker types never cross out of the module; conversions happen at the boundary. Its `#[wasm_bindgen_test]` cases run via the opt-in modes. |
| Owned vs interop clearly distinguished | Owned commands are top-level (`node`/`deno`/`web`); interop lives under `test bindgen-*` with the warning line. |
| No wasm-bindgen symbols in the framework | Verified by grep: zero references in `foundation_wasm/src`, `foundation_wasm_ui/src`, `foundation-wasm.js`, `foundation-wasm-ui.js`; the owned e2e modules import ONLY the `abi` module (checked via `WebAssembly.Module.imports`). |

`walrus` remains in use only on the opt-in `__wbgt_` discovery path; the owned
discovery rides the F15 wasmbin port (decision 031).
