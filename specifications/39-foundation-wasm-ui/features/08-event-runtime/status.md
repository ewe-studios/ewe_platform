# Feature 08 — Status: COMPLETE (2026-06-12)

## What shipped

**JS event runtime** (`foundation-wasm-ui.js`, building on the F00 dispatcher):

- `EventDispatcher` handler resolution order per element/event:
  1. `callback-N` / `"N"` refs → registry-callback bridge (`deliver`)
  2. `primal:setter="N"` on the element → SIGNAL bridge (`deliverSignal`) —
     this is the F03 two-way-binding leg (the `html!` marker `primal:onX="true"`
     plus the stamped setter id now actually dispatch)
  3. dot-path refs (`"controller.delete"`) → `resolveFunctionRef` against an
     injectable `scope` (default `globalThis`), bound so `this` = the element
  4. otherwise → `console.warn`, no listener
- **Delegation** (decision 018, opt-in): `primal:onclick:delegate="#container"`
  attaches on the resolved target (`#sel` / `parent` / `body` / query) and
  stamps `event.delegateTarget` when the event originates inside the element;
  compound keys (`click:delegate:<primal-id>`) let many elements share one
  container without colliding.
- **MutationObserver**: `handleMutations` wires added subtrees / cleans removed
  ones, SKIPPING anything `closest('island')` matches (boundary rule — islands
  own their lifecycle, F06); `observe(doc)` guards on observer availability.
- **`trackRemoved`** microtask batching (N synchronous removals → one pass).
- **Programmatic API**: `on(el, type, ref, {delegate})`, `off(el, type?)`
  (prefix-matches compound keys; bare `off(el)` clears all), and
  `on<event>` convenience methods for the 11 spec events.
- `initEventRuntime(dispatcher, doc)` — DOMContentLoaded + readyState guard.
- `signalDeliver(bridge)` — writes EventData JSON into a fresh global-arena
  slot and calls the dedicated `invoke_signal_callback(setterId, memId)`
  export.

**Rust bridge** (`foundation_wasm_ui::events`):

- `install_event_bridge(Rc<SignalsRuntime>)` / `uninstall_event_bridge` —
  process-global slot (single-threaded-wasm pattern, same precedent as the
  ABI registries).
- `#[no_mangle] invoke_signal_callback(callback_id, allocation_id)`: reads the
  JSON from the global arena, ALWAYS frees the slot (Rust owns cleanup, even
  on failure), dispatches via `Runtime::invoke_callback_json`, and runs
  `stabilize()` in the same synchronous chain — DOM updates from the setter
  flush before the export returns. Every failure mode (no bridge, stale id,
  missing slot, bad UTF-8/JSON) is logged-and-dropped per §13.
- Callable natively — the whole leg is tested without a JS host.

## Verification

- JS: 19 dispatcher tests (7 pre-existing + 12 new) covering spec tables —
  dot-path bind/`this`/warn (1, 5, 7), setter routing, delegation 9-13
  (placement, parent/body, delegateTarget, missing-target warn, compound-key
  non-collision), mutation handling + island boundary 14-19 (driven through
  `handleMutations` records — node has no real MutationObserver), microtask
  batching 24-26, programmatic API + convenience 27-31. Full wasm_ui JS suite:
  32/32.
- Rust: `events_tests.rs` end-to-end on the export — set + synchronous
  stabilize (effect re-ran inside the call), slot freed on every path, stale
  id / bad JSON / missing slot / no-bridge / uninstalled all leave signals
  untouched. Full wasm_ui Rust suite 62 green; zero clippy `--all-targets`.

## Spec deviations (justified)

| Spec says | Shipped | Why |
|-----------|---------|-----|
| Reuse the `invoke_callback(N, memId)` export | dedicated `invoke_signal_callback` | Signal ids (G17, signals registry) and function-registry ids are SEPARATE namespaces with independent counters; the F00 `invoke_callback` export already serves the function-call ABI. One export per namespace, no id collisions. |
| Flat `EventData` (`shift_key` etc.) | F02's shape (camelCase, `modifiers{alt,ctrl,shift,meta}`) | Already implemented, serde-tested, and emitted by `buildEventData` since F00 — one wire shape end-to-end. |
| §6 delegation pseudocode (handler never invoked) | implemented to the spec's TESTS: target placement + `delegateTarget` stamping | The spec's own §6 listener body only stamps the target; tests 9-13 assert exactly that. Handler dispatch composition is left to consumers reading `delegateTarget`. |
| Global singleton runtime + DOMContentLoaded auto-init | injectable `EventDispatcher` + explicit `initEventRuntime(dispatcher)` | No module-level globals/side effects on import (testability; multiple runtimes per page possible). |
| MutationObserver unit tests | driven via `handleMutations(records)` | node:test has no live MutationObserver; the callback logic is identical and fully covered, `observe()` is the thin browser shim. |
