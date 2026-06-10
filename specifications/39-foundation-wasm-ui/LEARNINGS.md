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

### Feature 12 added — testbed owns native testing (spec-31 review) — 2026-06-10

Reviewed `specifications/completed/31-wasm-testbed`: it claims to replace wasm-pack/wasm-bindgen but
actually DEPENDS on them — `bindgen-*` modes run the wasm-bindgen CLI for JS glue, discover tests via
the `__wbgt_` wasm-bindgen convention (walrus), and feature-02 wrote `#[wasm_bindgen_test]` tests.
That contradicts spec-39 owning the WASM↔JS interop. Added `features/12-testbed-native-harness/`
specifying the owned replacement: `foundation_wasm_testbed` builds a foundation_wasm cdylib (LLVM
backend — it already sets `CARGO_PROFILE_DEV_CODEGEN_BACKEND=llvm`), runs it under foundation-wasm.js
via `{ abi: rt.web_abi }`, with OUR `#[wasm_test]` macro + `__fwt_` discovery + a result protocol;
node/deno/browser runners; wasm-bindgen kept only as an explicit opt-in. Proven seed already in-tree:
`integrations/nodejs/foundation-wasm/` (15 node tests incl. real-module e2e, zero wasm-bindgen).
Registered in requirements.md Feature Index (rows 01-08 there are the stale original plan; authoritative
set is features/00-14).

**Refined into a policy + focused features (2026-06-10, per user):** own WASM infra end-to-end;
wasm-bindgen permitted ONLY at small, explicit, opt-in integration points (Cloudflare worker-rs which
is heavily wasm-bindgen, foundation_db/SDKs); walrus is fine (wasm-format tool, incl. parsing
wasm-bindgen output + our own discovery). Added:
- **decision 031** — owned WASM infrastructure policy (the principle + permitted boundaries).
- **feature 12** — testbed runners/orchestration (build→stage→run→report).
- **feature 13** — owned test execution: `#[wasm_test]` (foundation_macros) + `__fwt_` discovery
  (walrus, our prefix) + result protocol over our ABI. Replaces `#[wasm_bindgen_test]`/`__wbgt_`.
- **feature 14** — the sanctioned, opt-in wasm-bindgen boundary; never in the owned backbone.
Shared wasm build primitive (cargo→wasm32, LLVM backend via CARGO_PROFILE_DEV_CODEGEN_BACKEND=llvm)
factored across F10 (ewe-wasm) and F12 (testbed).

**Feature 15 added (2026-06-10):** port `wasmbin` (https://github.com/RReverser/wasmbin, Apache-2.0,
© Ingvar Stepanyan, v0.9.2 — source at `/home/darkvoid/Boxxed/@formulas/src.rust/src.wasm/src.tsgen/wasmbin/`)
into `foundation_codegen::wasm` for type-safe, minimal-diff (`Lazy<T>`) wasm editing; its derive
macros → `foundation_macros` (per macros-location rule); CLI binary → `foundation_codegentools`
(beside `wasm_bins`). Extend with WAT (text) ⇄ binary. Workspace is also Apache-2.0 → compatible;
attribution MANDATORY (vendored LICENSE, NOTICE, per-file credit headers, README links, upstream
version, stated modifications — Apache-2.0 §4). **wasmbin vs walrus (clarified 2026-06-10):** wasmbin covers everything we actually use walrus for
(our only live walrus use is one read-only export scan in `foundation_wasm_testbed/src/wasm_test.rs`),
plus type-safe minimal-diff editing — a superset. The ONLY edge walrus has is mutation ergonomics:
its id-arena + `tombstone_arena` + `passes/{used,gc}` auto-fix references on heavy structural
rewrites; wasmbin is a faithful 1:1 mirror (you manage indices). So **wasmbin is the single owned
wasm tool**; walrus dropped from the owned path (decision 031 updated).

**Feature 16 added — walrus transform port, DEFERRED (2026-06-10, per user):** reviewed walrus
(https://github.com/rustwasm/walrus, MIT/Apache-2.0, © Nick Fitzgerald, v0.23.3, source at
`/home/darkvoid/Boxxed/@formulas/src.rust/src.wasm/src.wasmbindgen/walrus/`). ~12.5k LOC, NOT
self-contained (wraps wasmparser + wasm-encoder), heavy deps (id-arena, gimli/DWARF, rayon,
walrus-macro). Overlaps wasmbin for parse/serialize; unique value = mutable IR + auto-relocation for
heavy rewrites. **Deferred** — record intent; trigger to revisit = real need for heavy structural
rewrites. The one capability worth having (reference cleanup on delete) is recorded as a wasmbin (F15)
**future enhancement**: cache deleted refs/tombstones, run a cleanup+relocation pass before serialize
(design ref: walrus `tombstone_arena.rs` + `passes/used.rs`+`gc.rs`) — cherry-pick, not full port.

### Task 5 — EventDispatcher increment DONE — 2026-06-10

`foundation-wasm-ui.js` gained `EventDispatcher` (decision 018): `scanAndWire` finds `primal:on*`
attributes (via `getAttributeNames`, descends children), wires direct listeners (default binding —
works for non-bubbling events), idempotent rewiring (G2 — `off` before re-add), `removeListeners`
cleanup. On an event it builds EventData {type, primalId, value, checked, keyCode, modifiers} (G4)
and calls an injected `deliver(callbackId, eventData)`. `parseCallbackId` accepts `"7"`/`"callback-7"`
(numeric → WASM callback), returns null for JS function refs (`controller.delete` — later increment).
`callbackDeliver(callbackRegistry, encode=JSON)` is the default deliver — ships EventData via the
`CallbackRegistry` (foundation-wasm.js); the EventData wire format stays swappable (F08 owns it).
Tests: `foundation_wasm_ui/integration/test/event-dispatcher.test.js` (7) + mock-dom event support.
**foundation_wasm_ui/integration: 10 node tests green** (3 DOM applicator + 7 event).

### Integration harness RELOCATED — 2026-06-10 (new skill rule)

Per the new rust-clean-code rule (crate-owned integration harnesses), the node harness moved from
top-level `integrations/nodejs/foundation-wasm/` into the crates:
`backends/foundation_wasm/integration/` (12 tests: ABI core + e2e module + fixture + build-module.sh)
and `backends/foundation_wasm_ui/integration/` (10 tests). Run per crate: `cd <crate>/integration &&
node --test`. The e2e module is a standalone `[workspace]` crate at
`foundation_wasm/integration/module` (workspace-excluded). Rule added to
`.agents/skills/rust-clean-code/testing/skill.md` §3 (`.agents` is a submodule — the rule edit is
committed there separately by the user).

### Task 5 — AnimationDriver increment DONE — 2026-06-10

foundation-wasm.js gained `AnimationDriver`: the `hook_up_animation_frames` import starts a rAF
loop that calls `trigger_animation_callbacks(ts)` each frame and stops when
`get_total_animation_callbacks()` returns 0. Idempotent start (one loop), injectable rAF
(`opts.rafHost`; default = `requestAnimationFrame`, ~16ms timer fallback for node/deno), `stop()`.
Wired into `web_abi.hook_up_animation_frames` (was a no-op stub). mock-wasm gained the animation
exports; 3 tests. **foundation_wasm/integration: 15 node tests green.**

### Task 5 — StringCache increment DONE — 2026-06-10

foundation-wasm.js gained `StringCache` + the `host_cache_string(ptr, len, encoding)` import:
reads a UTF-8 (0) or UTF-16LE (1) string from WASM memory and interns it, returning a stable
`bigint` handle (same string → same handle) — the building block `CachedText` params reference.
3 tests. **foundation_wasm/integration: 18 node tests green.**

### Feature 17 — ABI function-call codec RESEARCH locked in — 2026-06-10

Before porting the codec, mapped both sides into `features/17-abi-function-call-codec/`:
- `research.md` — the invoke flow + the TWO param encodings (FLAT: Rust `Params::to_binary` ↔ JS
  `ParameterParserV1`, used by host_invoke_function; MARKER+QUANTIZED: Rust `Batchable::encode` ↔ JS
  `ParameterParserV2`, used by batch/instructions). Resolves the earlier contradiction —
  `host_invoke_function_with_return` uses V1 (flat), so current Rust+JS match.
- `research-core-types.md` — parity map of the foundational types everything builds on:
  ExternalPointer/InternalPointer/CachePointer (u64 ids into host heaps), ThreeState
  (ThreeStateId 70/80/90), ReturnHint family (No/Single/List/MultiReturn ↔ ReturnIds 0/1/2/3),
  ReturnHintValue, ReturnHintParser (ReturnHintMarker Start=200/Stop=201), TypedArraySlice
  (TypedSlice 1-10, param id 30), ReplyContainer, Reply (encode ↔ ReturnValueParserIter decode),
  BatchOperation + Operations (ArgumentOperations Start/Begin/End/Stop=1/2/3/4), BatchInstructions.
  Includes the consolidated discriminant tables (the contract), parity rules, and a carry-forward
  open-items checklist so no detail is lost in the port.

### Task 5 REMAINING:
- `FunctionRegistry` + the parameter/return codec (megatron's `ParameterParserV2` ~1000 lines,
  `ReturnHintParser`) — the big host_invoke_* surface. **Format grounding (studied 2026-06-10):**
  `abi::web::invoke` ships `params.to_binary()` + `returns.to_binary()` via `host_invoke_function`.
  Each param is `[ArgumentOperations::Begin][ParamTypeId:u8][optional TypeOptimization byte][value…]
  [ArgumentOperations::End]` — NOT a flat `[type][payload]`. The optimized path runs value
  QUANTIZATION (`value_quantitization::qf64/qi16/qu16/...` in ops.rs) so value width is variable;
  Float32/Int8/Bool have no tq byte, Float64/Int16+/ErrorCode do. `ParamTypeId` discriminants:
  Null=0,Undefined=1,Bool=2,Text8=3,Text16=4,Int8=5..Float64=14,ExternalReference=15,
  *ArrayBuffer=16-25,InternalReference=26,Int128=27,Uint128=28,CachedText=29,TypedArraySlice=30,
  ErrorCode=31 (base.rs:566). **Authoritative format spec = ops.rs encode tests (ops.rs ~1135-1330)
  assert exact bytes per type.** This is a dedicated effort (faithful decode incl. quantization +
  return encoder + e2e WASM module that register_function/invoke); do NOT rush — high mismatch risk.
  Port from megatron `ParameterParserV2`/`ReturnHintParser`/`Reply` (the working impl) for parity.

  **Full invoke contract (studied 2026-06-10):** `host_invoke_function(handler:u64, params_ptr,
  params_len, returns_ptr, returns_len) -> u64`. Returns a **MemoryId** (as u64) pointing to an
  ALLOCATIONS slot holding the **ReturnValues binary** matching the hints. JS flow:
  (1) decode params from `[params_ptr,len]`; (2) decode return hints from `[returns_ptr,len]`;
  (3) call the registered JS fn(args) (with `this` = the runtime, e.g. `this.mock`); (4) ENCODE the
  result as ReturnValues per the hints (megatron `Reply`); (5) `create_allocation`, write, return
  `MemoryId.as_u64()`. `host_register_function(src_ptr, src_len) -> handle`: JS reads the function
  SOURCE string, evals → fn, stores, returns a handle. There are typed fast-paths
  `host_invoke_function_as_{bool,i8..u64,f32,f64}` too.

  **This IS the custom-binary protocol (byte 0)** — per user: the Params/Instructions encoding is
  our custom binary format; the transport prefixes messages with the envelope header
  `[protocol][version][memory_id][length]` and the dispatcher routes byte 0 → the custom-binary
  handler whose payload is this Params/ReturnValues codec. (host_invoke_function passes params raw,
  not enveloped; the envelope wrapping applies to the host_apply/DOM-op path.)

  **Validation suite = `integrations/nodejs/integrations/*`** (real .wasm + .wat + index.node.js,
  megatron-driven): tests_callfunction, tests_registerfunction, tests_instructions_{array,function,
  multi_return,none_return_callback,array_callback}, tests_js_invoke_function(+_and_return_{big_int,
  bool,dom,none,object,string,types},_with_array), tests_js_{raf,timeout,interval,invoke_async_function,
  invoke_failed_async_function}. PLAN: port the codec into a `FunctionRegistry` in foundation-wasm.js,
  then re-point these tests' `require("./megatron.js")` → `foundation-wasm.js` (`{abi: rt.web_abi}`,
  `rt.init`) to validate parity (the F12/F13 migration). Import sigs confirmed from .wat:
  `host_register_function`:(i64,i64,i32)->i64, `host_invoke_function`:(i64,i32,i64,i32,i64)->i64.
- foundation-wasm-ui.js rest: SignalBridge, ComponentRegistry (islands/mount), Hydrator
  (styles/scripts; events now via EventDispatcher), MutationObserver auto-wire/cleanup (decision 018
  §3, browser-only), SSEClient, transports (decision 028), MorphDom (decision 027 — applicator has an
  innerHTML fallback for MORPH_NODE).
- Retire megatron.js once the new pair reaches parity; optionally wire into foundation_wasm_testbed
  (deno) for browser-shaped e2e.
- Per user's plan ordering: finish F00 → F15 (wasmbin, grounds F14) → F14 → then testable wasm.

### Open integration seam (still): `InstructionReceiver` owns its OWN `MemoryAllocations`, but JS
`dispose_allocation` (exposed_runtime) frees the GLOBAL `ALLOCATIONS` static — reconcile when wiring
the live loop (likely the receiver should use the global arena).
