# Feature 03 — Status: COMPLETE (2026-06-13)

## What shipped

- **`App`** (foundation_wasm_ui::app): `new()` = THE ARROW-FAMILY DEFAULT
  (wire v1 columnar), `columnar()` alias, `json()`/`mock()` opt-in
  debug/test presets, `arrow()` (wire v2) behind the `arrow` cargo
  feature, `with_protocol` escape hatch; `context() -> (Context,
  SharedInstructionReceiver)`, `ctx()/receiver()/signals()/runtime()/
  stabilize()/scope()`; `Default`; drop disposes the root scope.
- **Server presets** (review addition): `server()` / `server_with(encoder)`
  over the new generic `FrameSink<E>` protocol — every flush captured as an
  envelope-framed `Vec<u8>` (native `host_apply` is a no-op stub, so
  ordinary presets silently drop flushes server-side). Works with ANY
  Layer-1 encoder; verified for v1 columnar, JSON, and Arrow IPC v2.
- **`ArrowIpcV2`** protocol (byte 1, VERSION 2) wrapping the existing
  `foundation_arrow::ArrowIpcEncoder`, behind the `arrow` feature
  (dependency-weight gate: arrow-rs + chrono-tz are heavyweight and force
  std into wasm artifacts; server code needs NO feature — it passes the
  encoder to `server_with` directly).
- **`Context: Clone`** (foundation_signals): handle-counted — a clone is
  another handle to the SAME scope; disposal at the LAST handle.
  `Rc::strong_count` could not decide this (parents hold child inners in
  `children`), so `ContextInner.handles` counts explicitly.
- README: §2 rewritten around `App`; new §10 "Rendering on the server"
  (first-paint via to_markup with zero runtime; live UI via server frames →
  WS/SSE → mount-stream; encoder choice; one-App-per-connection lifecycle;
  html-patch morphing).

## Verification

6 app tests (mock loop e2e, presets, drop-disposal, independent child
scopes, server frames v1 byte-exact [protocol 1, version 1], JSON encoder
swap [protocol 2, readable payload]) + 2 more under `--features arrow`
(Arrow IPC v2 frames [protocol 1, version 2], preset construction) + 3
Context-clone contract tests (clone-keeps-alive, last-drop-disposes,
child/parent independence). Full signals+wasm_ui suites green; zero clippy
default AND `--features arrow`; wasm32 check clean.
