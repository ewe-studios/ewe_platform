# Feature 01 — Status: COMPLETE (2026-06-13)

## What shipped

- **`Runtime::untracked` / `Context::untracked`** (foundation_signals): the
  §5 risk was REAL — `tracked` evaluations assert non-nesting, so mounting
  reactive content inside a watcher effect needed a save/restore escape.
  Also guarantees content signals never become watcher dependencies
  (toggling a counter inside `<Show>` re-renders the counter's slot, never
  remounts the region — pinned by test).
- **`mount_before`** (slots): `InsertBefore`-attached splicing returning
  EVERY top-level id (multi-root content unmounts correctly); internal
  build walk generalized over an `Attach` enum.
- **`reactive::{mount_show, mount_for}`**: anchored regions (empty
  id-bearing end-anchor `<span>`), per-instance child scopes (feature 03's
  handle-counted Context), unmount = RemoveNode per tracked id + scope
  drop. `mount_for` reconciliation: removals → forward-order mounting of
  new items (renders + initial effects run in DOCUMENT order — the first
  cut mounted end→start and the acceptance suite caught the reversed
  SetText stream) → end→start positioning pass ONLY when kept-key order
  changed (append/remove/update emit ZERO moves, op-stream asserted).
  Duplicate keys tracing::error!'d, last wins. LIS minimal-move pass =
  documented future work.
- **Macro built-ins**: `<Show when={expr}>{impl Render}</Show>` and
  `<For each={expr} key={fn} render={fn} />` join the capitalized
  namespace; thin arms emitting one runtime call each; attribute/shape
  validation with spanned errors; PURE form = compile error (a pure tree
  is a value — conditionals/lists there are plain Rust around the macro);
  neither consumes runtime indexes.

## Verification

7 acceptance tests: Show mount/unmount edges with anchored InsertBefore +
exact removed ids; interior-effect disposal on hide; content-signal
isolation (no remount, SetText flows); fresh-instance re-show with static
Html content; For initial order ("a","b" SetText stream), append = ONE
insert + zero moves, remove without moves, reorder creates nothing +
restores via InsertBefore, empty→filled→empty round trip. Full
signals/ui_traits/wasm_ui suites green; zero clippy; wasm32 check clean.
