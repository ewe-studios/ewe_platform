# Feature 02 — Status: COMPLETE (2026-06-12)

## What shipped

`backends/foundation_signals` — the standalone R3-style reactive crate
(decisions 002/003/004/008/011/029). Depends only on `foundation_ui_traits`
(for the `IntoHtml` bridge), serde/serde_json (EventData), tracing.

- **Runtime** (`runtime.rs`): unified graph behind ONE `RefCell` — bucket-queue
  `dirty[height]`, global version counter, `active` node + read trail for
  dynamic dependency tracking, deferred removals, callback registry,
  notification managers. `stabilize()` is the only flush path; `set()` never
  propagates inline. The file's core discipline: user closures always run with
  the graph borrow RELEASED (they re-enter through getters/setters).
- **Nodes** (`node.rs` + `arena.rs`): Signal/Computed/Effect in a hand-rolled
  generational arena (~80 lines — no `slotmap` dependency; stale `NodeId`s
  miss by generation, double-remove is a no-op). `ThreeState` Clean/Check/Dirty
  with the spec's transition rules; Check resolution compares dep versions
  snapshotted at last evaluation.
- **Handles** (`signal.rs`/`computed.rs`): `(SignalGetter, SignalSetter)`
  tuples (decision 029), `ComputedGetter`; values live TYPED in per-handle
  storages — the graph nodes carry only observers/deps/heights/versions plus a
  type-erased eval closure that reports "changed?" (spec's `Box<dyn Any>` value
  slots replaced; G18's downcast-failure path is unreachable by construction).
  `get_untracked()` provided for event-handler reads.
- **Context** (`context.rs`): scoped creation, `child()` cascade disposal,
  `on_cleanup` (attaches to the ACTIVE effect when called during one, else
  context-level), double-dispose no-ops at every level. Effects run
  immediately at creation (decision 008); computeds seed-evaluate tracked.
- **Callbacks** (`callback.rs`): `EventData` mirrors the JS `buildEventData`
  shape field-for-field (camelCase serde). G17: `callback_id` assigned at
  signal creation; signals of `String`/`bool`/numeric types get a DEFAULT
  conversion callback registered immediately (TypeId dispatch); other types
  are registered by the `html!` macro via `Runtime::register_callback`.
  `invoke_callback`/`invoke_callback_json`; stale ids silently dropped; bad
  payloads logged + skipped.
- **IntoHtml** for `SignalGetter`/`ComputedGetter` (owned + borrowed) — the F01
  integration: `{count}` in `html!` reads-and-subscribes.
- `single_threaded` default feature (G15); the Mutex path is a `compile_error!`
  until a real consumer needs it (documented).

## Verification

21 tests covering every spec test group: basics, immediate effect run,
exactly-once re-runs, conditional (cold-branch) dependency drop AND re-acquire,
computed batching, diamond exactly-once with fresh branches, Check
short-circuit, context/cascade/double disposal, context-level cleanup,
monotonic callback ids, default + JSON callback dispatch, numeric parse +
type-mismatch retention, stale-id drop, set-coalescing, height-ordered effect
runs, re-entrant set propagating in the same pass, cleanup-before-re-run,
dynamic height recomputation ordering, notification-manager ordering. Plus the
lib.rs doctest. Zero clippy warnings (`--all-targets`, uat).

## Spec deviations (justified)

| Spec says | Shipped | Why |
|-----------|---------|-----|
| `crates/foundation_signals/` | `backends/foundation_signals/` | Same as F01 — `crates/*` is workspace-excluded. |
| `slotmap` dependency | In-crate generational arena | ~80 lines; avoids a new workspace dep; same O(1)+ABA guarantees. |
| Nodes hold `Box<dyn Any>` values | Typed storage per handle; nodes hold eval closures returning `changed: bool` | Same observable semantics, no downcast failure path (G18 unreachable), values never erased. |
| `Effect` trait `register`/`unregister` called by the system | Trait provided as the downstream extension contract | The runtime runs closures; concrete effect TYPES (DomSignalBinding, F04+) implement the trait. Spec's own lifecycle text matches this reading. |
| `ctx.effect(\|\| ... ctx.on_cleanup(...))` example | `Runtime::on_cleanup_active` for inside-effect registration | The spec example borrows `ctx` inside a `'static` closure — not expressible in Rust. Capturing a runtime clone is; `Context::on_cleanup` still handles both positions. |
| files `effect.rs`/`notification.rs`/`callback.rs` | present | layout matches section "File Ownership". |

## For downstream features

- F03 (`html!`): read `setter.callback_id()`, emit `primal:setter(ID)`, and
  `Runtime::register_callback` for non-default-convertible types.
- F04 (`InstructionReceiver`): effects capture a receiver clone and `queue()`
  DomOps — see `DomSignalBinding` sketch in the spec (lives in
  foundation_wasm_ui, NOT here).
- F08 (events): JS calls `invoke_callback(id, …)`/`invoke_callback_json`,
  then `stabilize()`.

## Q&A: why `RefCell`/`Rc` instead of `Mutex`/`Arc`? (2026-06-12)

Raised in review: "if others use this library shouldn't it be Send/Sync-safe?"

- **It's the spec's G15 decision, with a seam**: WASM — the primary target — is
  single-threaded, so `single_threaded` is the DEFAULT feature; the spec keeps
  a cfg-based cell alias for a future Mutex path. The off-state is currently a
  `compile_error!` so nobody silently ships an untested locking path.
- **The hot path is reads**: every effect/computed evaluation calls `get()` on
  several signals, and handles are cloned into closures constantly. RefCell is
  a branch; Rc clone is a non-atomic increment. Mutex+Arc puts an atomic CAS on
  every read and every handle clone — paid even when there is only one thread,
  which on wasm32 is always.
- **Send bounds are viral**: `Mutex<Graph>` alone isn't enough — every stored
  closure (`Box<dyn FnMut>` effects/computeds/callbacks) would need `+ Send`,
  so user closures could no longer capture `Rc` state, DOM handles, or WASM
  externals (all `!Send`). That breaks the primary consumer to serve a
  hypothetical one.
- **Deadlock replaces panic**: the graph re-enters itself BY DESIGN (effects
  call getters/setters mid-stabilize). The RefCell discipline (release the
  borrow around all user code) maps onto a Mutex too — but a violation becomes
  a silent deadlock instead of a loud borrow panic.
- **Do the signal PROPERTIES survive Send+Sync?** The algorithms (height order,
  diamond exactly-once, Check short-circuit, FIFO-within-height) are
  thread-agnostic — but they're only OBSERVABLE if stabilize is exclusive. Two
  threads stabilizing concurrently forces either one big graph lock (so you pay
  atomic costs to get exactly single-threaded behavior) or a fundamentally
  different lock-free design. Determinism (e.g. DomOp flush order) survives
  only under the big lock.
- **The right multi-thread story when it's needed**: route cross-thread writes
  through valtron/channels INTO the signal thread (actor style) — graph stays
  single-threaded, zero property loss, no viral bounds. The feature-gated Mutex
  swap remains the escape hatch for a consumer that truly needs shared handles.

