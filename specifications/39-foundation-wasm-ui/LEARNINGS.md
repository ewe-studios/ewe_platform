# LEARNINGS

## Feature 00 — foundation_wasm Refactor & Split

### Layer 1: encoders in foundation_ui_traits (DONE — 2026-06-10)

- Added `backends/foundation_ui_traits/src/protocol.rs`: `ProtocolEncoder<T>` trait,
  `Envelope` (6-byte `[protocol][version][length:4 LE]`), `DecodeError`, and three
  encoders — `ArrowEncoder`, `JsonEncoder`, `CustomBinaryEncoder` — all over `Vec<DomOp>`.
- Tests in `tests/protocol_tests.rs` (12 tests): round-trip per encoder, envelope
  write/parse, protocol-byte identity, decoder error cases. `cargo build` (no_std path)
  is clean.

**Key resolution — "Arrow" in a no_std zero-dep crate:** `foundation_arrow` exists but
requires `std` (chrono+std, serde_json). Feature 00's test list mandates encoders compile
under `#![no_std]`. So `ArrowEncoder` emits the **custom 6-column columnar layout** from
decision 010 (op_id, node_id, operation u8, attribute, value, text_val) with Arrow-style
varlen string columns (`[N+1 offsets][data_len][utf8]`) — Arrow-*inspired*, NOT Apache
Arrow IPC. JS parses it via TypedArray column views. If true Apache Arrow IPC is ever
required for DOM ops, it must live in a std-capable crate, not foundation_ui_traits.

**Canonical DomOp ↔ operation-code mapping** (decision 010, shared by Arrow + CustomBinary):
CreateEl=0, SetText=2, SetAttr=3, AppendChild=8, Remove=10, InsertBefore=11, Replace=12,
SetStyle=13, SetClass(→ADD_CLASS)=14, Morph=16. Child/ref/new ids that don't have a
dedicated numeric column are stored as decimal strings in attribute/value/text columns
(per decision 010's 6-column schema) and parsed back with `str::parse`.

**JSON in no_std:** no workspace JSON lib is no_std (foundation_nostd = concurrency only),
so `JsonEncoder` hand-rolls encode + a minimal recursive-descent parser (`protocol::json`)
scoped to the emitted shape, with full JSON string escape/unescape handling.

**no_std/zero-dep deviation:** skipped `tracing` in foundation_ui_traits (pure data crate,
no deps by design); hand-impl'd `Display` for `DecodeError` instead of derive_more.

### Layer 2 (part 1 — additive transport types): DONE — 2026-06-10

- Added `backends/foundation_wasm/src/protocol.rs` (NEW, additive — jsapi.rs untouched):
  `ProtocolHandler` trait, 14-byte `WasmEnvelope`
  (`[protocol:1][version:1][memory_id:8 LE][length:4 LE]`), `ProtocolHandlerRegistry`,
  and `dispatch_message` (panics on unknown protocol byte per decision 028).
  `WasmEnvelope::parse` panics on <14 bytes (by design); `try_parse` is the fallible variant.
- Tests in `backends/foundation_wasm/tests/protocol_tests.rs` (5 tests, all green).
- `MemoryId` (base.rs) packs index in the HIGH 32 bits, generation in the LOW 32; the
  envelope round-trips because both sides use `as_u64`/`from_u64`.

**RULE reminders honored:**
- Tests go in `{crate}/tests/`, NOT inline `#[cfg(test)] mod tests` (corrected after
  first putting them inline in src/protocol.rs).
- "New file, don't touch old files until the replacement is proven." Only additive
  `mod`/`pub use` lines were added to each crate's lib.rs (required to compile/test new
  code). No existing logic file (jsapi.rs, frames.rs, …) modified or deleted.

### Layer 2 (part 2 — relocate return-value parser): DONE — 2026-06-10

Moved `ReturnValueParserIter` (priv), `impl FromBinary for ReturnTypeHints`, and
`GroupReturnTypeHints` (pub) out of `jsapi.rs` (was lines 1857-3174) into `protocol.rs`,
byte-for-byte. `jsapi.rs` 3174 → 1856 lines; `protocol.rs` now owns the parser.

**Why this move could NOT use the parallel old+new "parity test" pattern:** (1) it's a
trait impl (`FromBinary for ReturnTypeHints`) — Rust coherence forbids two impls, so old
and new can't coexist; (2) the parser reads/frees array-buffer return values from the
**global `ALLOCATIONS` arena** — duplicating it would mean two arenas and broken behavior.
So the move is necessarily atomic. Safety net = byte-identical relocation + full suite:
`cargo test -p foundation_wasm` (47 tests green) + dependents `foundation_core`,
`foundation_netio` compile against the unchanged public API.

**Coupling handled:** `ALLOCATIONS` made `pub(crate)` in jsapi.rs; `protocol.rs` does
`use crate::jsapi::ALLOCATIONS`. The two `use super::{… GroupReturnTypeHints …}` lists in
jsapi's `host_runtime`/`web` submodules repointed to `use crate::GroupReturnTypeHints`.
Pruned now-unused imports from jsapi.rs top `use crate::{…}`.

### Layer 2 (part 3 — file split jsapi.rs → host_runtime.rs): DONE — 2026-06-10

- `git mv jsapi.rs host_runtime.rs`. Updated lib.rs (`mod jsapi`→`mod host_runtime`,
  `pub use`) and protocol.rs (`crate::jsapi::ALLOCATIONS`→`crate::host_runtime::ALLOCATIONS`).
- **Collision fix + denest:** the file-module `host_runtime` contained a `pub mod host_runtime`
  (the FFI interface) → `pub use host_runtime::*` would clash. Renamed the inner FFI module
  `host_runtime`→`abi` (62-token single-file sed rename; matches its
  `#[link(wasm_import_module = "abi")]`). Public path changed `foundation_wasm::host_runtime::web`
  → `foundation_wasm::abi::web` — zero consumers, verified. `internal_api`/`exposed_runtime`
  crate-root re-exports unchanged.
- Verified: foundation_wasm 41+5+1 tests green; foundation_core + foundation_netio build.

**foundation_wasm now = host_runtime.rs + protocol.rs (+ base/ops/mem/registry/schedule/
intervals/frames/error/wrapped). jsapi.rs deleted.** L2 file-split deliverable complete.

### SPEC CORRECTION (judgement call, record): feature 00 Part A says "frames.rs moves
ENTIRELY to foundation_wasm_ui (TickState, FrameCallback, FnFrameCallback, FrameCallbackList)."
This is WRONG/self-contradictory:
- `TickState` (defined in frames.rs) is used by `intervals.rs` (stays) — timers requeue via it.
- `ANIMATION_FRAME_CALLBACKS` static + `exposed_runtime::{trigger_animation_callbacks,
  get_total_animation_callbacks}` WASM exports (stay) depend on `FrameCallbackList`.
- foundation_wasm CANNOT depend upward on foundation_wasm_ui.
=> **frames.rs (TickState, FrameCallback*, FrameCallbackList) + all animation registry/exports
STAY in foundation_wasm** as generic ABI. Only the pure DOM-reference surface leaves.

### Task 3 — DOM extraction to foundation_wasm_ui: DONE — 2026-06-10

Scaffolded `foundation_wasm_ui`: Cargo.toml (deps foundation_wasm + foundation_ui_traits),
`#![no_std]` lib.rs, module tree `wasm/dom/{constants,element}.rs`.

Replicated-then-removed (user directive), all verified:
- `DOM_SELF/THIS/WINDOW/DOCUMENT/BODY` → `foundation_wasm_ui::wasm::dom::constants`
  (slots 0-4, `ExternalPointer::pointer` is `const fn`).
- `allocate_dom_reference` + its DOM FFI `dom_allocate_external_pointer` (extern + non-wasm
  stub) → `wasm::dom::element`. foundation_wasm_ui declares its OWN `#[link(wasm_import_module
  = "abi")]` extern, so the DOM FFI no longer lives in the ABI crate.
- `HostFunction::invoke_for_dom` → `DomInvoke` extension trait in `wasm::dom::element`
  (works because `HostFunction.handler` is `pub` and `abi::web::invoke` is public). The method
  is GONE from foundation_wasm; `invoke_for_object` (generic) stays.
- `ReturnTypeId::DOMObject` STAYS in base.rs (spec) — it's just a binary tag the parser reads.

Removed from foundation_wasm/host_runtime.rs (replaced with breadcrumb comments): the 5 consts,
the `dom_allocate_external_pointer` extern + stub, `allocate_dom_reference`, `invoke_for_dom`.
Verified: foundation_wasm 47 tests green, foundation_wasm_ui 3 DOM tests green, dependents build.
Grep confirms no live DOM refs remain in foundation_wasm (only comments).

**foundation_wasm is now DOM-free** (the feature's core purity goal) — except the `DOMObject`
return-type tag which is protocol-agnostic and correctly stays.

### Layer 3 — protocol impls + InstructionReceiver in foundation_wasm_ui: DONE — 2026-06-10

- `protocol/`: `ProtocolMethods<T>: ProtocolHandler` trait, `SendResult`, `HandleResult`
  (`Result<Vec<DomOp>, DecodeError>`), and `ArrowV1`/`CustomBinaryV1`/`JsonV1` — each pairs a
  Layer-1 encoder (foundation_ui_traits) with the Layer-2 transport (foundation_wasm). Shared
  `encode_and_ship` helper: encode → allocate ONE arena slot → frame payload in a 14-byte
  `WasmEnvelope` (so JS reads protocol byte + memory_id) → `host_apply`.
- `instruction/receiver.rs`: `InstructionReceiver` (decision 030) — `queue`/`flush`/`ack`,
  owns ops + protocol + `MemoryAllocations`. `flush` drains via `mem::take`, returns
  `Option<SendResult>` (None on empty — no encoding/FFI).
- Tests `tests/protocol_impl_tests.rs` (5): per-protocol encode→frame→decode round-trip,
  one-slot assertion, ack frees, receiver batch/empty-flush. All 8 ui tests green.

**`host_apply` placement (corrected after review):** the uniform 3-param transport FFI is generic
Layer-2, so it lives in `foundation_wasm::abi::web` (added: extern import + non-wasm stub), NOT in
foundation_wasm_ui. Layer 3 calls `foundation_wasm::abi::web::host_apply`. (`#[allow(unused_unsafe)]`
on the `ship` wrapper — unsafe on wasm extern, safe stub on native.)

### `web` feature gating (decoupling the JS host ABI from the pure ABI) — DONE — 2026-06-10

The `abi` module IS the JS/web host contract (`#[link(wasm_import_module = "abi")]`). Baking it in
unconditionally forced EVERY foundation_wasm consumer (WASI/native/custom hosts) to supply those
imports. Fix:
- `foundation_wasm`: new `web` feature (NOT default). `#[cfg(feature = "web")] pub mod abi`.
  All 55 web-FFI refs are contained in `abi`; `internal_api`/`exposed_runtime`/`protocol.rs`/`mem`/
  `ops`/registries have ZERO — so the pure ABI compiles without `web` (verified, warning-free; the
  web-only top-of-file `use crate::{...}` imports are also `#[cfg(feature="web")]`).
- `foundation_wasm_ui`: depends on `foundation_wasm = { features = ["web"] }` (always on — it's the
  web UI layer).

### foundation_core bug fixed (exposed by enabling js-foundation-wasm) — 2026-06-10

The valtron JS-yield backends (`executors/wasm/`) were feature-gated but NOT target-gated, despite
the doc saying "JSThreadYielder on wasm32, NoThreadController on native." So a NATIVE build with
`js-foundation-wasm` (or `js-wasmbindgen`) wrongly selected the wasm-only `JSThreadYielder`, whose
continuation closure isn't `Send` while the native `register_schedule` registry requires it → E0277.
Fix (matches documented intent): gate `JSThreadYielder` + the backend modules to
`any(target_arch=wasm32/wasm64)` + feature; `DefaultController` falls back to `NoThreadController` on
native+feature; provide a native `JS_WAIT_CHECK_INTERVAL` fallback const for the shared feature-gated
yield path in `local.rs`. Also updated the path my rename broke:
`foundation_wasm::host_runtime::web::register_schedule` → `foundation_wasm::abi::web::register_schedule`,
and `js-foundation-wasm = ["foundation_wasm/web"]`. Verified: foundation_core builds native default,
`--features js-foundation-wasm`, AND `--features js-wasmbindgen`. (wasm32 target build blocked by this
env's nightly std for wasm32-unknown-unknown — toolchain limitation, not code.)

### Task 5 — JS runtime split (in progress): foundation-wasm.js CORE done — 2026-06-10

New file `backends/foundation_wasm/runtime/foundation-wasm.js` (ES module, megatron.js left
intact per the new-file rule). Core ABI classes, clean rewrite aligned to the CURRENT Rust
exports (not a mechanical copy of megatron's 7181-line tangle):
- `WasmEnvelope` (14-byte header, mirrors Rust), `ProtocolDispatcher` (routes by protocol byte,
  throws on unknown — mirrors `dispatch_message`), `MemoryAllocations` (arena view), `TimerRegistry`
  (schedule_*/run_*_callback), `CallbackRegistry` (invoke_callback), and `FoundationWasm` runtime
  that exposes `web_abi` (the `{ abi: {...} }` import object) + `init(module)`.

**Bootstrap contract (learned from foundation_wasm_testbed + integrations/nodejs/integrations):**
instantiate with `{ abi: rt.web_abi }`; the WASM module EXPORTS its own memory, so memory =
`instance.exports.memory` (set in `init()`, NOT a JS-provided `WebAssembly.Memory`). Import
closures read a lazily-populated `bridge` so they work before the instance exists. `node:test` is
the established convention.

**Fresh test harness** (user said the old `integrations/nodejs/integrations` tests need a compiled
.wasm per case and are heavy → new one): `integrations/nodejs/foundation-wasm/` — zero-dep
`node --test`, `mock-wasm.js` simulates the arena in a JS ArrayBuffer + records export calls. 11
tests green (envelope round-trip/layout/bounds, dispatcher routing + unknown-throw, memory
create/write/get/dispose, timer fire + interval-stop, callback invoke, host_apply dispatch+dispose
incl. dispose-on-throw). Added `runtime/package.json {"type":"module"}` to silence Node's
typeless-module warning.

### Task 5 — REAL-MODULE e2e testing now works (key enabler) — 2026-06-10

**wasm32 builds: use `--profile uat`.** The repo's `dev` profile uses the Cranelift codegen
backend (`.cargo/config.toml` + root Cargo.toml `[profile.dev] codegen-backend="cranelift"`),
which can't target wasm32 ("Support for this target has not been implemented yet" comes from
Cranelift, NOT a missing target). `[profile.uat] codegen-backend="llvm"` builds wasm32 fine.
foundation_wasm (no_std) compiles to wasm32-unknown-unknown this way.

**e2e harness:** `integrations/nodejs/foundation-wasm/module/` is a tiny STANDALONE crate (own
`[workspace]`, path deps, NOT a workspace member — keeps the `web` feature out of the main
workspace's feature unification; also in root Cargo.toml `exclude`). It's a `std` crate (gets the
default allocator + panic handler — like the existing fixtures; `#![no_std]` cdylibs would need a
manual `#[global_allocator]`/`#[panic_handler]`). Exports `emit_arrow_batch`: allocates a GLOBAL
arena slot via `exposed_runtime::create_allocation`, frames an Arrow batch in a `WasmEnvelope`,
ships via `abi::web::host_apply`. Imports only `abi.host_apply`; exports memory + the arena fns.
- Build: `./build-module.sh` → copies to `fixtures/foundation_wasm_e2e.wasm` (committed, like the
  old fixtures). `module/target/` gitignored.
- `test/e2e-real-module.test.js`: instantiates with `{ abi: rt.web_abi }`, `rt.init(instance)`,
  calls `emit_arrow_batch()`, asserts the handler saw a 2-row Arrow payload and the slot was freed.
  Skips gracefully if the fixture isn't built. 12 JS tests green total (11 mock + 1 real).

This proves the full Rust↔JS protocol path against a REAL module. Going forward, JS features can be
tested against real wasm, not just mocks.

**Side effect noted (not fixed):** the OLD megatron-era fixtures under
`integrations/nodejs/integrations/*` use the pre-rename path `host_runtime::web::*` (now `abi::web`)
and the old megatron.js — their SOURCE is stale after the rename, but they're excluded and ship
committed .wasm so nothing rebuilds them. They'll be superseded as the new harness grows; revisit if
we keep them.

### Task 5 — foundation-wasm-ui.js: ArrowDomApplicator increment DONE — 2026-06-10

`backends/foundation_wasm_ui/runtimes/foundation-wasm-ui.js` (ES module): `ArrowParser`
(decodes the columnar layout — exact mirror of `foundation_ui_traits::ArrowEncoder`),
`NodeRegistry` (primal-id→node, reserved 0-2 = head/body/html per G1), `ArrowDomApplicator`
(applies all decision-010 op codes to a `document`), `arrowHandler()` factory for the dispatcher.
Tested in the same harness (`test/foundation-wasm-ui.test.js` + `mock-dom.js`): synthetic
create/attr/class/append batch, unknown-node-id throw, and **full WASM→JS→DOM e2e** — the real
module's Arrow batch is parsed (nodeIds=[5,6], ops=[SET_TEXT,REMOVE], textVal[0]="hi") and applied
to the mock DOM. **15 JS tests green total.**

### Task 5 REMAINING:
- `FunctionRegistry` + the parameter/return codec (megatron's `ParameterParserV2` ~1000 lines,
  `ReturnHintParser`) — the big host_invoke_* surface.
- foundation-wasm-ui.js rest: SignalBridge, ComponentRegistry (islands/mount), EventDispatcher +
  Hydrator (primal:on* wiring, decision 018), SSEClient, transports (decision 028), MorphDom
  (decision 027 — applicator currently has an innerHTML fallback for MORPH_NODE).
- Retire megatron.js once the new pair reaches parity; optionally wire into foundation_wasm_testbed
  (deno) for browser-shaped e2e.

### Open integration seam (still): `InstructionReceiver` owns its OWN `MemoryAllocations`, but JS
`dispose_allocation` (exposed_runtime) frees the GLOBAL `ALLOCATIONS` static — reconcile when wiring
the live loop (likely the receiver should use the global arena).
