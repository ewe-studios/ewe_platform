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

### Task 5 — FunctionRegistry codec PORTED + validated against real WASM — 2026-06-10

`backends/foundation_wasm/runtime/function-registry.js` (ES module, imported by foundation-wasm.js):
faithful port of megatron `ParameterParserV1` (FLAT `[ParamType:u8][value]` decode — all 0-31 types
incl. Text8/16 pointer-into-memory, arrays, CachedText via StringCache, 128-bit, refs as
Ext/Internal/CachePointer), `ReturnHintParser` (`[Start=200][ReturnIds][ThreeState][Stop=201]`),
and `Reply`/`ReplyEncoder` (`[ReturnType:u8][value]` matching `ReturnValueParserIter`; `return_naked`
scalar fast-path; `encode_into_memory` → MemoryId; None→-1n). `FunctionRegistry`: `register`
(reads source, `Function(...)` eval, handle heap), `invoke` (generic → MemoryId), and typed
`invoke_as_{bool,float,int,bigint}` (naked). Wired into `web_abi` (host_register_function,
host_invoke_function, host_invoke_function_as_*, host_unregister_function).

**Validated against a REAL module** (foundation_wasm/integration/module exports roundtrip_i32/f64/
bool_and + capture_mixed_params): Rust `Params::to_binary` → JS decode → fn → JS encode → Rust
`invoke_as_*` decode, all green. **foundation_wasm/integration: 22 node tests.**

**CRITICAL parity finding (2026-06-10):** the reply ReturnValues binary written by `encode_into_memory`
MUST be framed `[ReturnValueMarker::Begin=100]…[ReturnValueMarker::End=101]` — `FromBinary for
ReturnTypeHints` (protocol.rs) rejects anything else (`WrongStarterCode`/`WrongEndingCode`) and strips
the frame before `ReturnValueParserIter`. Initially omitted → string returns failed. Also: the
`invoke_for_{bool,i8..u64,f32,f64}` HostFunction methods are NAKED aliases (call `invoke_as_*`), NOT
the from_binary path — only `invoke_for_replies` / `invoke_for_str` / `invoke_for_none` /
`invoke_for_object` go through `from_binary`. So a TRUE generic-return test uses `invoke_for_replies`.
String return = inner slot of UTF-8 bytes + `[Text8=2][inner_slot_id:u64]`; Rust Text8 arm
`take()`s + frees that slot. Validated: roundtrip_via_reply_i32 (invoke_for_replies, framed scalar),
roundtrip_string_len / _echo_len (Text8 in + string out). **foundation_wasm/integration: 25 tests.**

REMAINING on the codec (carry-forward): string return DONE; remaining:
ReplyEncoder for arrays/MemorySlice/Object/DOMObject/refs (slot semantics), validating the generic
MemoryId-return path + List/Multi hints, and the async-callback path (`invoke_callback` replies).
The marker/quantized V2 batch codec remains separate (host_apply/instructions path).

### Task 5 STILL REMAINING (other):
- (codec extras above)
- `FunctionRegistry` legacy note (megatron's `ParameterParserV2` ~1000 lines,
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

## V2 batch codec ported + validated (2026-06-11)

**`runtime/batch-instructions.js`** is the faithful port of megatron's
`ParameterParserV2` + `BatchInstructions` + `BatchOperation` (the custom protocol's backbone —
NOT legacy; coexists with Arrow as separate protocol paths). Validated e2e against a real
uat-profile module: MakeFunction+Invoke with group returns, no-return mixed params, InvokeAsync
callback delivery, and quantized-vs-full-width spread (32/32 tests green).

Pinned contract (Rust `ops.rs`/`base.rs` are the byte-level source of truth):
- **Operations**: Begin=0, MakeFunction=1, Invoke=2, InvokeAsync=3, End=254, Stop=255. Ops stream =
  `[Begin](op…)*[Stop]`, each op = `[opId][payload][End]`. TWO buffers ship via
  `host_batch_apply(ops_ptr, ops_len, text_ptr, text_len)`: OPS (opcodes) + TEXTS (raw UTF-8).
- **MakeFunction payload**: `[ParamTypeId.ExternalReference=15][TQ][handle]` then
  `[ParamTypeId.Text8=3][index:u64 RAW][len:u64 RAW]` (string location into TEXTS — unquantized,
  unlike Text8 params!). Handle comes from `function_allocate_external_pointer` pre-allocation;
  JS `heap.update(handle, fn)`. Thunk yields One(ExternalReference) — so a returning batch's
  results INCLUDE the MakeFunction handle (results[0]) before invoke results.
- **Invoke/InvokeAsync payload**: external handle, (async: `[26][TQ][callback]`), return-hint frame
  `[200][ReturnIds][ThreeStates…][201]`, args `[ArgStart=1]([ArgBegin=2][type][TQ?][value][ArgEnd=3])*
  [ArgStop=4]`, then `[End=254]`. `encode_params(None)` writes NOTHING (no ArgStart) — Rust callers
  must pass `Some(&[])`; JS requires the Start marker (megatron parity).
- **TypeOptimization quantization** (0–27): Bool/Int8/Uint8/Float32 carry NO TQ byte; all other
  numerics/refs/pointers carry `[TQ][narrowed bytes]` per `value_quantitization::q*` (i16→i8,
  i32→i8/i16, i64→i8/i16/i32, u-equivalents, f64→f32 when in f32 range, 128-bit→8/16/32/64,
  ptr→u8/u16/u32). 128-bit None layout = [msb:8][lsb:8] LE halves, msb first.
  `create_instructions(text, ops)` always sets optimized=true.
- **Text8 param** = TWO TQ'd u64s (index, len) into the TEXTS string (substr) — NOT WASM memory.
  Text16/TypedArraySlice/*ArrayBuffer = TQ'd pointer + TQ'd length into WASM memory
  (*ArrayBuffer len = ELEMENT count ×BYTES_PER_ELEMENT; TypedArraySlice len = bytes; Text16 len ×2).
- **Group returns** (`host_batch_returning_apply` → u64 slot id, -1 if none): frame
  `[GroupReturnHintMarker.Start=111]([ReturnIds][Multi only: count:u16 LE][ThreeStates…]
  [slot_id:u64 LE])*[Stop=222]`; each slot holds a Begin=100..End=101 framed ReturnValues binary;
  Rust `GroupReturnTypeHints::from_binary` decodes + frees each slot. Decoded per-instruction as
  Returns::One/List/Multi.
- **Unified function heap**: host_register_function, function_allocate_external_pointer, and batch
  MakeFunction all share ONE generation-arena heap (`ExternalHeap`, uid = (index<<32)|generation,
  bigint) — handles interchangeable across flat invoke and batch invoke (megatron parity).
  `object_allocate_external_pointer` gets its own ExternalHeap on the runtime.
- **megatron bugs NOT ported** (wire format follows the Rust encoder instead): `parseErrorCode`
  read a u64 but Rust encodes ErrorCode via qu16 (`[TQ][u8|u16]`); `parseText16` was broken
  (no return); `parseNull` validated against Undefined. Also `parseNumber64` missed several TQ
  arms (e.g. QuantizedUint16AsU8) — the port's `readQuantized` handles the full table.
- **Async-in-batch**: InvokeAsync thunk returns null (never an inline result); reply delivered via
  the shared `ReplyEncoder.callbackSuccess` → `CallbackRegistry.invoke` → `invoke_callback`.
  None-hint = fire-and-forget (promise unwatched).

## Single-file browser-safe runtimes (2026-06-11)

`runtime/foundation-wasm.js` is now ONE self-contained file (function-call codec +
V2 batch codec + core runtime merged; `function-registry.js`/`batch-instructions.js`
deleted). No internal imports → loads in-browser without a bundler:
`<script type="module">` for the ESM exports, plus a `globalThis.FoundationWasmRuntime`
frozen mirror for classic scripts. `foundation-wasm-ui.js` (already single-file) gets the
matching `globalThis.FoundationWasmUiRuntime` mirror. All 34 + 10 node tests green
against the merged files — future runtime work edits these single files directly.

## Naked object/DOM fast-paths + heap drops restored (2026-06-11)

Feature-00's Rust port had dropped megatron-era externs; restored for 1:1 parity:
- **`host_invoke_function_as_object`** (core) / **`host_invoke_function_as_dom`** (UI crate's own
  FFI, like `dom_allocate_external_pointer`): the JS fn's result interns into the object/DOM heap
  and the HANDLE crosses naked — no reply encoding. `invoke_for_object`/`invoke_for_dom` previously
  mis-wrapped the generic `invoke()` result (an encoded MemoryId) as an ExternalPointer — fixed to
  ride the naked externs. JS: `ReplyEncoder.#naked` transforms BEFORE the naked check (megatron
  `Reply.immediate` order): Object/DOMObject → heap.create → handle; refs → raw id; scalars as-is.
- **Drops**: `host_object_drop_external_pointer` + `drop_object_reference`,
  `host_string_cache_drop_external_pointer` + `drop_cached_string` (core);
  `host_dom_drop_external_pointer` + `drop_dom_reference` (UI). JS heaps destroy by generation;
  StringCache.drop evicts both maps. (`host_function_drop_external_pointer`'s role is served by
  `host_unregister_function`.)
- **DomHeap** (foundation-wasm-ui.js, import-free duplicate of the arena): reserved slots 0–4 =
  self/heap/window/document/body (megatron DOMArena parity; destroy refuses). CRITICAL: the Rust
  constants DOM_SELF..DOM_BODY are the RAW values 0–4, not packed uids — DomHeap resolves
  uid < 5 directly to the reserved slots (safe: reserved slots never bump generation, and packed
  index-n uids are n<<32). `domAbi(rt, dom)` returns the import fragment
  ({dom_allocate_external_pointer, host_dom_drop_external_pointer, host_invoke_function_as_dom})
  and wires `rt.functions.reply.dom`.
- **UI crate e2e harness**: `foundation_wasm_ui/integration/{module,fixtures,build-module.sh,
  test/dom-abi.test.js}` — standalone uat-profile wasm fixture (depends on foundation_wasm_ui),
  excluded from the root workspace like the core one. 37 core + 14 UI tests green.
- `foundation_wasm` now re-exports `raw_parts` (lib.rs) so dependent crates can hand param
  buffers to their own host FFI the same way the core crate does.

## Megatron drop-in parity proven on legacy fixtures (2026-06-11)

`integration/test/megatron-parity.test.js` runs ALL 20 megatron-era compiled fixtures
(integrations/nodejs/integrations/*/module.wasm, referenced in place read-only) on the NEW
runtime — 21 parity tests green (the DOM one lives in foundation_wasm_ui's suite with
DomHeap+domAbi). The modules SELF-ASSERT decoded values in Rust (panic→trap), so green =
byte-level drop-in parity. What it took beyond the codecs:

- **Context `as*` helper API**: registered fns call `this.mock.*` AND `this.asUint8(10)`,
  `this.asMemorySlice(0)`, `this.asFakeNode('div')` etc. — the megatron middleware was their
  `this`. Ported the full helper family onto FunctionRegistry (the default context):
  ReplyContainer (pre-typed return slot, passes through #containers untouched), FakeNode,
  ReplyError (Error with .code), asNone/asBool/ints/floats/128s/asText8/asErrorCode/
  asObject (interns)/asDOMObject/asFakeNode/asMemorySlice (numeric = EXISTING slot id!)/
  asTypedArraySlice/as*Array (ride MemorySlice bytes)/asInternal+ExternalReference.
- **Union ThreeState resolution = megatron check_for_type ORDER**: candidates tried in
  declared order, first whose RUNTIME TYPE matches wins (`1` vs Three(Bool,Int8,Uint8) →
  Int8, NOT Bool). The old infer-then-fallback encoded wrong types (caught by the module's
  own assert_eq in tests_instructions_multi_return).
- **invoke() -1 bug**: only a None HINT yields -1n; an undefined result under a One(None)
  hint still encodes a framed None reply whose slot id the module dereferences
  (tests_…_return_types trapped with InvalidAllocationId before the fix).
- **ErrorCode unwrap**: echoed ErrorCodeValue params / ReplyError must encode their u16
  `.code` (errorCodeOf helper; encode arm + #naked arm + callbackFailure).
- **V1 64/128-bit params surface as BigInt** (megatron parseBigInt64). The OLD suite used
  NON-strict deepEqual (5n == 5 passes) — don't import its literal expectations into
  strict asserts.
- Old fixtures' import set is a strict subset of the new web_abi (verified via
  WebAssembly.Module.imports across all 21 modules), so megatron.js retirement (F12/F13)
  is unblocked.

## Final megatron pieces: AsyncTaskCollector + WasmLoader/WasmWebScripts (2026-06-11)

The last unported megatron classes are in foundation-wasm.js: AsyncTaskCollector
(promise tracking for async invocations, OFF by default; `rt.tasks.enable()` +
`rt.awaitTasks()`; wired into both the flat invokeAsync and batch InvokeAsync paths) and
WasmLoader/WasmWebScripts (loadURL via instantiateStreaming / loadBytes, `js.mem` memory
import, JS-string-builtins compile options, `script[type="application/wasm"]` page
bootstrap with runAll()). With these, every class in megatron.js has a new-runtime
counterpart: 61 core + 15 UI tests green. The JS runtime split (megatron 1:1 port) is
functionally COMPLETE — remaining foundation-wasm-ui.js items (SignalBridge,
ComponentRegistry, Hydrator, MutationObserver, SSE, transports, MorphDom) are NET-NEW
spec features (decisions 013/018/027/028), not megatron ports.

## Clarification: the BigInt finding was a test-oracle trap, not a coverage gap (2026-06-11)

The V1 64/128-bit BigInt behavior IS tested — more strictly than ever before. The sequence:
the new parity test copied the OLD suite's literal expectation (`[5, 5, 10, 10, …]`, plain
numbers) and FAILED under `assert/strict`, because Int64/Uint64/Int128/Uint128 params arrive
as `5n`/`10n`. Cross-checking megatron's own source (parseBigInt64, megatron.js:2123 — pushes
the raw `getBigInt64` result) confirmed megatron ALWAYS delivered BigInts there; the old test
only passed because node's NON-strict `assert.deepEqual` treats `5n == 5` as equal — it never
distinguished JS value types. The new runtime matches megatron exactly; only the copied
expectation was wrong.

Now pinned by TWO oracles: (1) the strict assert verifies exact JS types (number vs BigInt)
per parameter position; (2) the wasm module independently self-asserts the round-trip in Rust
(`100 * v1` checked module-side; mismatch = trap). Rule for porting the remaining old tests:
the old suite's literal expectation arrays are NOT a reliable source for value TYPES — always
cross-check megatron's parser source (or the Rust encoder) when writing strict assertions.

## F15 started: wasmbin port landed (derives + core model) (2026-06-11)

- **Derives in foundation_macros** (`wasmbin_codec.rs`, from wasmbin-derive v0.2.4):
  Wasmbin (Encode/Decode/DecodeWithDiscriminant), WasmbinCountable, Visit. Adaptations:
  trait paths resolve via proc-macro-crate → `foundation_codegen::wasm::{io,builtins,visit}`
  (upstream hardcoded `crate::…`); decl_derive! → plain #[proc_macro_derive] wrappers
  (run_synstructure helper). PathItem/in_path must be PUBLIC in the model for the derives
  to expand outside foundation_codegen.
- **Core model in foundation_codegen::wasm** (from wasmbin v0.9.2): mechanical port
  (`crate::X` → `crate::wasm::X`, `wasmbin_derive::` → `foundation_macros::`) compiled on
  the second pass. Gotchas: upstream uses edition-2024 let-chains (one rewrite in
  builtins/lazy.rs); upstream's optional `serde` feature must be RENAMED (`wasm-serde`)
  because foundation_codegen has a non-optional serde dep and cargo forbids a feature
  sharing a dependency's name; doctests carry `use wasmbin::…` paths that need repathing.
  New deps: leb128, thiserror 2, custom_debug, once_cell, serde_bytes (optional).
- **Attribution set**: workspace NOTICE, vendored LICENSE at src/wasm/vendor/,
  module README (modifications per Apache-2.0 §4(b)), per-file port notes, v0.9.2 marker.
- **Round-trip oracle = the workspace's real .wasm fixtures**: all 23 (2 e2e + 21 legacy)
  decode→encode byte-identically; export enumeration (F13 path) + append-only custom-section
  injection (F10 path) proven in tests/wasm_roundtrip_tests.rs.
- Remaining F15: WAT ⇄ binary layer (net-new design), CLI in foundation_codegentools
  (inspect/edit/convert/validate), then the deferred reference auto-cleanup enhancement.

## F15 complete: WAT layer + CLI (2026-06-11)

- `foundation_codegen::wasm::wat` (feature `wat`): from_wat/to_wat via the `wat` +
  `wasmprinter` boundary crates (small/isolated/opt-in per the F14 rules the spec
  authorizes). CRITICAL pairing: `wat` 1.NNN ↔ `wasmprinter` 0.NNN must match — a newer
  printer (0.224) emits `$"…"` quoted identifiers the older parser (1.207) rejects on
  real rustc-built modules (Rust-mangled element-segment names). Byte-identity through
  TEXT is not expected (name section re-derived); the right property is a BINARY FIXPOINT
  after one text round-trip.
- CLI: `foundation_codegentools::cli::wasm` — inspect / validate / convert / edit
  (add-custom-section, rename-export). `validate` uses a deep `Visit` traversal, which
  forces every `Lazy` payload (function bodies) to decode — Lazy makes a plain decode
  succeed even on payloads that are internally corrupt.
- Feature 15: ALL success criteria met (status.md). Follow-ups recorded: owned WAT
  printer; deferred reference auto-cleanup; F16 stays deferred.

## Custom protocol (byte 0) re-based on BatchInstructions (2026-06-11)

User caught a protocol mismatch: a scaffolded `CustomBinaryEncoder` row codec had claimed
protocol byte 0, but decision 022 defines byte 0 as "Custom binary (foundation_wasm
Instructions)" — the BatchOperations/ParameterParserV2 batching system. Correction landed:

- **Row codec DELETED**; `Row` (the canonical decision-010 DomOp ⇄ row mapping) made public
  in foundation_ui_traits so all protocol impls share one mapping.
- **`BatchMessage` trait** (foundation_wasm, next to Instructions): payload types encode
  themselves INTO an Instructions batch — the selective opt-in contract. JS twin =
  `BatchInstructions.registerOperation`.
- **`BatchInstructionsV1`** (foundation_wasm_ui, byte 0): packs `[texts_off:u32][texts_len:
  u32][Operations stream][texts pool]` into ONE envelope slot via the uniform host_apply.
  `DomOpsBatch` = DomOps as registered opcode `BATCH_OP_APPLY_DOM = 10` (outside the core
  0–3/254/255 table) with Row fields as quantized params, strings via the texts pool.
  Arrow/JSON remain DomOp's default transports. A scoped native V2 reader (Uint8/Uint32/
  Text8 + their quantizations) keeps byte-0 round-trips testable without a JS host.
- **JS**: `batchProtocolHandler` (core, PRE-WIRED for byte 0 in the FoundationWasm
  constructor) computes absolute pointers from the live payload view and reuses
  `rt.batches.applyNoReturn` — no new parser. `registerDomBatchOperation(rt, applicator)`
  (UI) registers the DOM opcode feeding the same applicator as Arrow.
- **CRITICAL re-entrancy fix**: shipping while holding the global arena lock
  deadlock-panics — `host_apply` synchronously re-enters WASM (JS ACKs via the
  `dispose_allocation` export, which locks the global arena). `ProtocolMethods` gained
  `encode_and_write` (write the framed slot, return its live address) with
  `encode_and_send` as a provided one-call wrapper for OWNED arenas; the receiver's
  Global arm and any global-arena sender MUST write under the lock and `send_to_js`
  AFTER releasing it. e2e fixture goes through the real live loop
  (`InstructionReceiver::with_global_arena` → flush → byte-0 → JS batch runtime → DOM).
- **`ack` moved to `ProtocolHandler`** (Layer 2) with a stale-safe default — every
  handler ships through arena slots, so every handler can release one.
- **Tooling lesson**: `grep -cE '^(warning|error)'` NEVER matches cargo's colored output
  (ANSI codes precede the word) — every earlier "clippy clean" check this session was
  vacuous. Use `CARGO_TERM_COLOR=never` + tee to a file, and `touch` lib.rs first
  (clippy caches per-crate results and prints nothing on re-runs).

## F13 landed: owned #[wasm_test] execution model (2026-06-11)

- **`#[wasm_test]`** (foundation_macros): keeps the fn, emits `__fwt_<name>() -> u32`
  (0 = reported sync, 1 = report pending/async) + a `name|flags\n` line into the
  `__fwt_manifest` CUSTOM SECTION via `#[link_section]` + `#[used]` statics (lld
  concatenates same-section statics — the wasm-bindgen trick, on our prefix). Flags:
  a/p/i. Gated `#[cfg(wasm32/64)]` so native builds skip the export + section.
- **Result protocol** = `host_report(status, ptr, len)` import (0 pass / 1 fail /
  2 ignored + optional UTF-8 message). JS: `TestReports` collector with promise-based
  `next()` so runners await async outcomes.
- **wasm32 panics ABORT** — `catch_unwind` is useless there. Capture order: the macro
  installs a `std` panic hook IN THE USER CRATE (foundation_wasm is no_std) that calls
  `testing::fail_current(panic_text)` BEFORE the trap; the runner catches the
  `WebAssembly.RuntimeError` and re-instantiates (a trapped instance is dead — fresh
  instance per case is the robust runner shape). `should_panic` is therefore a
  MANIFEST flag the runner inverts on; the module can't observe its own panic.
- **Async cases** ride an owned re-poll loop: poll with `Waker::noop()`; on `Pending`,
  `abi::web::register_schedule(0.0, …)` re-polls on the next host timer tick (the
  schedule registry takes `Fn`, so the closure re-arms by cloning an `Rc<RefCell<…>>`
  future slot). Validated e2e: a Pending-once future resolves through node's timer.
- **Discovery** (`testbed::fwt::discover_cases`) uses OUR wasmbin port
  (foundation_codegen::wasm) — export scan for `__fwt_` + manifest section for flags.
  Spec said walrus; F15 was built with this use case as a success criterion (031:
  owned preferred). The old `__wbgt_` walrus discovery remains for the F14 opt-in.
- Publishing the testbed lib modules (so main.rs consumes the lib instead of
  re-declaring the module tree) surfaced ~30 pre-existing pedantic lints — fixed.
  foundation_netio (238), foundation_core (15), foundation_http (13) carry their own
  pre-existing warnings, OUTSIDE this spec's surface — flagged, not fixed here.

## F12+F14 landed: owned runners + interop boundary enforced (2026-06-11)

- **Owned runner** (`wasm-testbed node|deno|web <crate>`): build → `fwt` discovery →
  stage a SELF-CONTAINED temp harness (embedded runtime via `embedded-js` + one generic
  `runner.mjs` + `cases.json` + module) → run → exit code. ONE plain-ESM runner script
  serves node, deno (`deno run -A`), and the browser (index.html mirrors output into
  `#output` for the existing Playwright poller). Fresh instance per case (trapped
  instances are dead); `--filter` for substring selection.
- **spec-31 feature-02 migration**: the four valtron JS-yield integration tests ported
  to `#[wasm_test]` on `js-foundation-wasm` (`foundation_core/integration/wasm_tests`)
  — ALL GREEN through `wasm-testbed node`. Owned replacements for the bindgen bits:
  timing = a REGISTERED host fn (`Date.now()` over the ABI); safety timeouts = an
  awaited `WaitUntil` future on the owned re-poll loop (each Pending poll IS an executor
  re-entry, which is the thing under test). Originals retained as the F14 opt-in.
- **CRITICAL leak found+fixed**: foundation_core unconditionally enabled getrandom's
  `wasm_js`/`js` (wasm-bindgen) backends for wasm32 — EVERY owned module carried
  `__wbindgen_*` imports and could not instantiate on the owned runtime. Library rand
  is seeded-only now (`default-features = false`; all non-test usage is
  ChaCha8Rng/SeedableRng); getrandom backends tied to the `js-wasmbindgen` opt-in;
  native tests get full rand via dev-dependencies. Rule: check
  `WebAssembly.Module.imports` — an owned module imports ONLY the `abi` module.
- **F14**: bindgen modes log an `INTEROP MODE` warning; framework grep clean;
  foundation_db's optional `worker`/wasm-bindgen deps are the conformant seam example.
- foundation_core's pre-existing pedantic lints are now FIXED (lib + all test
  targets + doctests; foundation_macros test fixtures too). Two gotchas worth keeping:
  `--all-targets` clippy surfaces test-target lints that lib-only checks never show,
  and broken doctests don't fail `cargo test` unless `--doc` targets actually compile.
- **`#[valtron]` / `#[valtron_test]`** (foundation_macros, re-exported from
  `foundation_core::valtron`): tokio-style wrappers around
  `initialize_pool(seed, threads)` — body moves into an inner fn (preserves `return`/`?`
  and the return type), the PoolGuard is a named local dropped explicitly AFTER the
  body. Test variant adds `#[test]`, defaults threads to `Some(3)` and clamps explicit
  values to >= 3. Default seed derives from `RandomState` (no rand dep in user crates).
- **Dev-dep feature unification trap**: `cargo test -p foundation_core` builds with
  `multi` ON even though it's not a default feature — the dev-dependency chain
  (foundation_testing -> foundation_deployment -> foundation_db default) re-enters
  foundation_core with `multi`. Tests of unified entry points must use the
  feature-agnostic surface (`sync_one`, `execute`, ...) — `single::spawn` panics
  "Thread pool not initialized" because the unified init routed to the multi pool.
- **proc-macro-crate doctest gotcha**: inside the defining crate's own doctests,
  `FoundCrate::Itself` makes derives expand to `crate::...`, which points at the
  DOCTEST binary. Fix in the doctest itself: import the needed module at the doctest
  crate root AND declare an explicit `fn main` (the implicit wrapper would move the
  `use` inside a function where `crate::` paths can't see it). See
  `foundation_core::type_uuid` module docs.

## Feature 01 (foundation-ui-traits, 2026-06-11)

- **One Row mapping to rule every format**: Arrow columns, flat JSON objects, and
  the byte-0 batch stream all serialise through `Row::from_op`/`into_op`. Adding
  the 9 new ops touched exactly one mapping — the encoders stayed mechanical.
- **`id:` prefix wire convention**: known tags/attrs encode as `"id:<n>"` in the
  string columns; `TAG_NAMES`/`ATTR_NAMES` index+1 == id, order is ABI. The JS
  mirror tables are GENERATED from html.rs (python extraction), not hand-copied —
  regenerate when appending.
- **Registry staging model**: CreateElement/CreateTextNode park nodes in a
  `pending` map; REGISTER_NODE promotes (or resolves `[primal-id]` in the
  document); ReplaceNode is the one op allowed to consume a staged node directly
  (its implicit registration). Referencing a staged id any other way throws —
  producer bugs surface immediately.
- **Spec-internal conflicts resolved by layer**: F01's "uses the arrow crate"
  collides with its own "no dependencies / any Rust target" constraint — the
  no_std columnar layout stays (the shipped JS ArrowParser pins it) and F05 owns
  real Arrow IPC. Byte-0's CustomBinaryEncoder placement was superseded by
  decision 022 (BatchInstructions need the arena ⇒ wasm_ui layer). Deviations
  table in the feature's status.md.
- **PROTOCOL_VERSION 0 → 1** was safe to bump: JS reads the byte but never
  enforced it; all Rust consumers reference the const.

## Feature 02 (foundation_signals, 2026-06-12)

- **Borrow discipline IS the architecture**: the whole graph sits in one
  `RefCell`; correctness comes from never holding the borrow across user code.
  Pattern: snapshot (short borrow) -> clone the node's `Rc` eval closure ->
  drop borrow -> run closure (getters/setters take their own short borrows) ->
  re-borrow to commit. `process()` is deliberately one long fn so the
  discipline is visible in one place.
- **Typed storage beats `Box<dyn Any>` nodes**: graph nodes hold only
  topology + a `FnMut() -> bool` ("changed?") closure that captures the typed
  `Rc<...Storage<T>>`. No downcasts anywhere; spec G18 becomes unreachable.
- **Re-entrant set just works with a growable bucket queue**: the stabilize
  loop re-checks `dirty.len()` each iteration and pops one node at a time, so
  effects writing signals mid-pass schedule observers into the SAME pass when
  their height is still ahead. No recursion, no guard.
- **Default G17 callbacks via TypeId dispatch**: `ctx.signal::<T>` registers an
  EventData->T conversion callback when T is String/bool/numeric (Box<dyn Any>
  round-trip per type); other T get theirs from the macro. Keeps `signal()`
  generic without specialization.
- **`invoke_callback` removes the entry while running** (re-entrancy: callback
  sets a signal whose default callback is itself registered) and reinstates it
  after — unless disposal removed it meanwhile.

## Feature 04 (InstructionReceiver completion, 2026-06-12)

- **The loop e2e test caught a real ordering bug**: foundation_signals' dirty
  buckets were LIFO (Vec::pop) — two same-height effects flushed their DomOps
  in reverse creation order. DomOp streams are order-dependent, so buckets are
  now VecDeque FIFO. Lesson: cross-crate integration tests (signals → receiver
  → protocol) catch contracts no single-crate test expresses.
- **G22 capacity preservation**: flush uses `mem::replace(&mut ops,
  Vec::with_capacity(64))` — the protocol takes ownership of the batch Vec, so
  plain `clear()` can't work and plain `mem::take` zeroes capacity.
- **Recorder-handle mocks**: a mock consumed by `Box<dyn ProtocolMethods>`
  can't be inspected afterwards — expose `Rc<RefCell<..>>` recorder clones
  BEFORE boxing (MockProtocol::sent_batches/acked_ids).
- **Disk note (2026-06-12)**: the repo disk (/dev/sda "Boxxed") hit 100% —
  target/debug alone was 260G (Cranelift dev artifacts; all spec work builds
  --profile uat). Cleared target/debug. `df` on /home is the WRONG filesystem
  for this repo; check `df -h /home/darkvoid/Boxxed`.

## Feature 03 (html! macro, 2026-06-12)

- **One closure per expression, or nothing compiles**: the spec's "evaluate
  the slot for Html AND again in the effect" double-moves captured handles.
  Resolution: reactive slots put a placeholder in the tree and let the
  IMMEDIATE effect run (decision 008) deliver initial content — the expression
  exists in exactly one `move` closure. Consequence: a handle used in N slots
  needs N-1 explicit clones (plain Rust, documented).
- **Parent-SetText is a trap the spec set for itself**: spec test 25 mixes
  slots and elements under one parent; SetText-on-parent clobbers siblings.
  Dedicated text node per slot (CreateTextNode + Register + Append, SetText on
  IT) preserves mixed content and needed no new ops.
- **String id prefixes don't fit a u32 wire**: "42:0" can't ride
  DomOp.node_id. `ctx.allocate_id_block(total)` (monotonic) + `base + index`
  gives identical per-instance disjointness, wire-compatible.
- **Proc-macro crates can't export test hooks**: exact error-message tests for
  the parser live as in-file unit tests (documented deviation from the
  tests-in-tests/ rule) — integration tests can only observe successful
  expansion or add a compile-fail harness dep.
- **`__macro` hidden re-exports**: generated code can't say `::std::vec!`
  (no_std callers) or `::alloc::vec!` (std callers without extern alloc);
  re-exporting alloc types through foundation_ui_traits::__macro works in both.

## Feature 05 (Arrow IPC encoder, 2026-06-12)

- **The version byte earned its keep**: protocol byte 1 now demuxes by envelope
  VERSION — v1 owned columnar (wasm loop) vs v2 real Arrow IPC
  (foundation_arrow::ArrowIpcEncoder, servers/analytics). Same Row mapping,
  same ProtocolEncoder face; arrow-rs stays OUT of wasm binaries.
- Real nulls vs empty strings: v2 uses Arrow validity bitmaps for absent
  cells; Row's Option<String> mapped onto both conventions without changes.
- Invalid-UTF-8 testing is unreachable through arrow-rs (it validates at read)
  — spec rows demanding it should be marked N/A rather than force unsafe
  fixture construction.

