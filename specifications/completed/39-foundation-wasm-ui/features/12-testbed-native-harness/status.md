# Feature 12 — Status: COMPLETE (2026-06-11)

| Criterion | Evidence |
|---|---|
| `wasm-testbed node <crate>` full loop, no bindgen/wasm-pack | `fwt_runner::run_node`: build (existing LLVM-backend primitive) → `fwt::discover_cases` → stage a SELF-CONTAINED temp harness (embedded foundation-wasm(-ui).js via `embedded-js`, generic `runner.mjs`, `cases.json`, `module.wasm`) → run → exit code. Verified red (deliberate failure → exit 1, failure named) and green (`--filter` → exit 0) in `tests/fwt_runner_tests.rs` + live CLI runs. |
| `#[wasm_test]` + `__fwt_` discovery on foundation-wasm.js | One generic `runner.mjs` (plain ESM) drives all hosts: fresh instance per case (a trapped instance is dead), `should_panic` inversion, async awaited via `TestReports.next()`. |
| Pass/fail/panics reported + summarised | Per-case `ok/FAILED/ignored` lines + `test result: …` summary; process exit non-zero on failure. |
| Deno headless; browser via Playwright | `wasm-testbed deno` green (verified live). `wasm-testbed web` implemented over the EXISTING server+Playwright plumbing (an index.html shell mirrors runner output into `#output`, which browser.rs already polls); not e2e-verified here — Playwright isn't installed in this environment. |
| Spec-31 feature-02 tests re-expressed | `backends/foundation_core/integration/wasm_tests` — the four JS-yield valtron cases ported to `#[wasm_test]` on `js-foundation-wasm`: timing via a REGISTERED host fn (Date.now over the owned ABI), completion awaited on the owned re-poll loop. **All four green via `wasm-testbed node`.** Originals retained as the F14 opt-in. |
| bindgen retained as explicit opt-in | `test bindgen-*` subcommands only, now logging an INTEROP MODE warning (F14). |
| Shared wasm-build primitive | `build::run` (LLVM env override + workspace target dir) is the single primitive; the owned runner and the legacy modes both call it (feature 10 consumes the same fn). |

Found + fixed along the way: foundation_core unconditionally enabled getrandom's
wasm-bindgen backends on wasm32, dragging `__wbindgen_*` imports into every owned
module. Library rand is now seeded-only (`default-features = false`); the getrandom
js backends are tied to the `js-wasmbindgen` opt-in; native tests keep full rand
via dev-dependencies.

`init node <crate>` scaffolds a sample `#[wasm_test]` cases file (the harness
itself is generated at run time, so there is no JS dir to scaffold).
