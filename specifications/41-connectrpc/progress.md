# Spec 41 Progress

Last updated: 2026-07-12 (reconciled against git + feature frontmatter)

## Completed features (committed + tested)

| # | Feature | Key decision |
|---|---------|-------------|
| 01-03 | Valtron core | F02 Pipe<T> is the seam primitive |
| 04-11 | Netio/HTTP enablers | IncrementalDecoder, reactor parking, graceful drain |
| 12-28 | ConnectRPC crate (167 tests) | Unary bypasses seam; streaming uses join! |
| 29-30 | HTTP/2 substrate + multiplexers | H2Channel, H2Transport, h2c detection |
| 31 | gRPC protocol over h2c (11 tests) | h2 trailer plumbing: H2StreamEvent, TransportStream.trailers |
| 32 | h2 flow tuning | — |
| 33 | QUIC backend | quinn-proto sans-IO + valtron driver + netcap variants |
| 36 | WS resumable frame decoder (8 tests) | IncrementalDecoder impl, byte-level state machine |
| 37 | WS server task + Pipe migration | auto-Pong, Close handshake, assembler (redesign landed) |
| 38 | WS Depends read model | caller-injected EventReadiness (redesign landed) |
| 39 | WS transport (Connect-over-WS bidi) | Transport seam thesis validated (redesign landed) |
| 40-43 | io_uring line (D14 F1-F4) | shared reactor, uring selector, selection probe, completion-mode read path |
| 44 | Walking skeleton | ConnectRpcServe + H1Transport; 5 real-socket tests |
| 45 | Delivery/fanout backpressure | valtron delivery/split backpressure + Part D transport seam shrink |
| 46 | Executor verb builders | per-verb builders + Mapper deletion, zero per-fn cfg |
| 47 | h2 server integration | all 4 RPC modes green over h2c; pipe-based H2Serve |

## Implemented — with a documented tail

| # | Feature | Status |
|---|---------|--------|
| 48 | Transport completion read path | Phases 0-3 implemented; `foundation_iogate` bridge crate breaks netio↔nativeapis cycle. **Phase 4 (proxy splice) designed out.** |
| 51 | Unified HTTP+WebSocket client | Stages 1-5 landed + Stage 6 largely done: `SimpleHttpClient` alias-only, `H1Transport` holds `Arc<dyn HttpClient>`, no `NoSpawner`, `open_websocket` auto-switches native/wasm. **Tail deferred:** native WS connect path still carries `R: DnsResolver + Clone` (28 sites); full `foundation_connectrpc` wasm32 build not yet verified end-to-end (only netio checked). |
| 52 | Browser test harness | Implemented, **diverged from design**: shipped `#[valtron_bindgen]` + `#[valtron_wasm_test]` macros (valtron-pool-aware) instead of a bare `#[wasm_bindgen_test]` re-export. `foundation_wasm_testbed` deleted, tests relocated to `foundation_netio/tests/wasm/`, docs updated. **To confirm:** whether the 4-way CLI (`test` auto-detect) + version check + `bindgen-web` deprecation all landed. |

## Design proposals — not built

| # | Feature | Status |
|---|---------|--------|
| 49 | Write-side completion (IORING_OP_SEND) | proposed — design doc only |
| 50 | Client completion + zero-syscall proxy relay | proposed — design doc only |

## Pending

| # | Feature | Depends on | Effort |
|---|---------|-----------|--------|
| 34 | HTTP/3 module | 33, 29 | large |
| 35 | HTTP/3 transport | 34, 22 | large |

## Truly-remaining work (next steps)

1. **F51 Stage-6 tail** — drop the `R: DnsResolver + Clone` generic from the native
   WS connect path (28 sites) so the "no `DnsResolver + Clone` anywhere" criterion
   is met; then verify `foundation_connectrpc` compiles for `wasm32-unknown-unknown`
   end-to-end (headline acceptance criterion, only netio verified so far).
2. **F52 reconciliation** — confirm/flip the 4-way CLI table, `test` auto-detect,
   wasm-bindgen version check, and `bindgen-web` deprecation notice against what
   actually shipped.
3. **F49 / F50** — write-side + client completion are pure design proposals; decide
   whether to build now or after HTTP/3.
4. **F34 / F35** — HTTP/3 module + transport, still unstarted (large).
