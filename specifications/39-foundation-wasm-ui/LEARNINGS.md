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

### Next: Task 5 — JS runtime split (megatron.js 7181 lines → foundation-wasm.js + foundation-wasm-ui.js).
Note open seam for integration: `InstructionReceiver` owns its OWN `MemoryAllocations`, but JS
`dispose_allocation` (exposed_runtime) frees the GLOBAL `ALLOCATIONS` static — these two arenas must
be reconciled when wiring the real WASM↔JS loop (likely the receiver should use the global arena).
