# Feature 18 — Status: COMPLETE (2026-06-12)

## Review answers adopted (§6 — defaults, revisit any time)

1. **Stream policy**: v1 streams are UNBOUNDED (state reads belong on
   `snapshot()`; a latest-wins ring is a recorded follow-up — std mpsc has no
   drop-oldest primitive and building one wasn't worth blocking v1).
2. **`HubBusy` vs blocking**: unbounded command queue (decision 009 stance);
   no shedding mode in v1.
3. **Naming**: as designed — `SignalHub`/`HubHandle`/`RemoteGetter`/
   `RemoteSetter`/`SignalStream`/`HubDriver`.
4. **Placement**: in-crate `feature = "valtron"` (optional `foundation_core`
   dep only for the `TaskIterator` impls; the hub itself is plain std and
   compiled on all non-wasm targets).

## What shipped (`foundation_signals::hub`, non-wasm32)

- `SignalHub` — command MPSC + exposure registry + publisher effects;
  `pump()` = drain FIFO → apply → ONE `stabilize()` → publishers (ordinary
  effects) copy post-stabilize values into `Arc<RwLock>` watch cells and fan
  out to subscriber channels.
- `HubHandle::run_on_hub` (Send closures cross once, then run
  single-threaded with full `HubScope` access: context, runtime, minting
  remote handles — the §3.4 remote-creation pattern works verbatim) +
  `invoke_callback` for worker-side event sources.
- `RemoteSetter<T>`: `set`/`update` — VALUES cross threads, never the
  `Rc`-based setters (type-erased apply fns live in the hub registry);
  `PartialEq` dedup happens on the signal thread.
- `RemoteGetter<T>`: `snapshot()` (never touches the graph), `version()`
  (bumps per ACTUAL change), `changes()` streams.
- Stream END semantics: hub disposal clears the subscriber senders (context
  `on_cleanup`), so `SignalStream` reports `HubGone` after draining — found
  by the drop test; the shared subscriber vec would otherwise keep senders
  alive via `RemoteGetter` clones.
- `valtron` feature: `SignalStream` + `HubDriver` as `TaskIterator`s — never
  blocking in `next_status` (empty ⇒ `Pending`, disconnected ⇒ `None`),
  honoring the project's async-iterator rule.

## Verification

8 tests (worker threads, no sleeps): FIFO set+update with one-stabilize
proof, 100 updates across 4 workers coalescing into one pump, **the headline
glitch-freedom property** (a watcher thread hammering `snapshot()` of a
5a-diamond mirror across 198 pumps never observes a non-multiple of 5),
stream + version semantics incl. `PartialEq` dedup, remote creation via
`run_on_hub` with handles sent back over a oneshot, hub-drop observability
(`HubGone` from setters/handles/streams; snapshot frozen), idle pump no-op,
and the valtron leg (driver Ready-with-report then Pending; stream
Ready/Pending). Zero clippy (`--features valtron --all-targets`); full
workspace check green.
