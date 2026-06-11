# Feature 00 — Status: COMPLETE (2026-06-11)

Every existing capability preserved (the goal was "organizing, not removing"); megatron
drop-in parity proven by running all 21 megatron-era compiled fixtures on the new runtime.

## Delivered, mapped to the spec

| Spec section | State |
|---|---|
| Part A — foundation_wasm file map | DONE. `jsapi.rs` split into `host_runtime.rs` (FFI + invocation) and `protocol.rs` (WasmEnvelope, dispatch, ReturnValueParserIter, Group returns); all base/mem/ops/registry/schedule/intervals/wrapped types stay. Megatron-era externs the first split dropped were RESTORED: `host_invoke_function_as_object`, object/string-cache drop externs (+ `as_dom`/dom-drop in the UI crate's own FFI), fixing `invoke_for_object`/`invoke_for_dom` which mis-wrapped encoded slot ids as pointers. |
| Part B — foundation_wasm_ui crate | DONE. `protocol/{arrow,batch_instructions,json}.rs`, `instruction/receiver.rs`, `wasm/dom/{constants,element}.rs`, `embedded.rs` (feature `embedded-js`, forwards to `foundation_wasm/embedded-js`; embeds both runtime files). |
| Part C — three-layer protocols | DONE. Layer 1 encoders in `foundation_ui_traits`; Layer 2 `ProtocolHandler` + `WasmEnvelope` + `dispatch_message` in `foundation_wasm`; Layer 3 `ArrowV1`/`BatchInstructionsV1`/`JsonV1` + `ProtocolMethods` in `foundation_wasm_ui`. (Byte 0 was later re-based onto the Instructions format per decision 022 — the scaffolded row codec was deleted; see feature 17 status + LEARNINGS.) |
| Part D — JS runtime split | DONE for the megatron port. `foundation-wasm.js` is ONE self-contained file (ESM + `globalThis.FoundationWasmRuntime`): full V1+V2 codecs, hints/replies, heaps, timers, rAF, string cache, callbacks, context `as*` API, AsyncTaskCollector, WasmLoader/WasmWebScripts. `foundation-wasm-ui.js`: ArrowParser, NodeRegistry, ArrowDomApplicator, EventDispatcher, DomHeap + `domAbi`. |
| Part E — runtime architecture | InstructionReceiver per decision 030, with the arena seam RESOLVED: `with_global_arena()` allocates message slots in the global arena (the one JS's `dispose_allocation` frees) via the new `internal_api::with_global_allocations`; owned-arena mode remains for native tests/custom hosts. AppOrchestrator composition is feature 02/04 wiring. |
| Part F — JS→WASM flow | DONE. Spec's `instruction/bridge.rs` responsibilities landed in `CallbackRegistry` (JS) + `invoke_callback`/`parse_callback_replies` (Rust) — validated by the async e2e + legacy callback fixtures; no separate file warranted. |
| Part G — arena lifecycle | Preserved + e2e-validated (generation checks, dispose-ACK in finally, stale-id errors). |
| Part H — event listener flow | `primal:on*` wiring via EventDispatcher + `callbackDeliver` (UI suite). Full event-runtime semantics = feature 08. |
| Part I — Cargo changes | `web` feature gates the JS host ABI (default off); `embedded-js` on both crates; frames.rs STAYS in foundation_wasm (user-approved deviation — it is pure structure, no host ABI calls). |
| Refactoring strategy | Followed (new-files-first); old `sdk/jsruntime/megatron.js` master DELETED after parity proof. The per-test copies under `integrations/nodejs/integrations/*` remain as the legacy suite's own harness until F12/F13 retires it. |

## Validation

- 61 node tests in `foundation_wasm/integration` (unit + real uat-wasm e2e + 20 legacy
  megatron fixtures as parity oracles — the modules self-assert decoded values in Rust).
- 15 node tests in `foundation_wasm_ui/integration` (DOM ABI e2e + legacy DOM fixture).
- Rust: `foundation_wasm` + `foundation_wasm_ui` tests, check, clippy clean
  (incl. `--features embedded-js`).

## Deferred (own features, not part of the port)

SignalBridge (02), ComponentRegistry (06), MorphDom (07), event-runtime semantics +
MutationObserver (08), SSE/transports (11/25-era decisions), `window.primal` namespace —
net-new systems from decisions 002/013/018/027/028; megatron.js never contained them.
