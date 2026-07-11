# Spec 41 Progress

Last updated: 2026-07-12 (connectrpc wasm restructure landed, commit fb3d86744)

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
| 49 | Write-side completion (IORING_OP_SEND) | **Phases A+B implemented + verified 2026-07-12** (commit d17ec5634): `SendPool`/`SendBuf`, Selector `submit_send`/SEND-CQE drain, threaded to `CompletionSocket::write`/`flush`. Zero write-family syscalls proven via ptrace; 4 real-socket + 5 pool + 1 syscall tests. **Deferred:** Phase C (`split_read_write` write half), Phase D SEND_ZC (→ F50). |
| 51 | Unified HTTP+WebSocket client | **Wasm criterion MET** (commit fb3d86744): `foundation_connectrpc` compiles clean for `wasm32-unknown-unknown`. `SimpleHttpClient` alias-only, `H1Transport` holds `Arc<dyn HttpClient>`, no `NoSpawner`, `open_websocket` auto-switches native/wasm. **`+ Clone` criterion WAIVED** as cosmetic (28 inert bounds; stack monomorphises at `BoxedDnsResolver` where `Clone` = `Arc::clone`). |
| 51b | connectrpc shared/native restructure | Crate split into `shared/` (codec, compression, context, envelope, error, error_writer, interceptor, message, protocol, client, router, transport incl. H1+WS) + `native/` (server, h2_serve, transport h2/h3). h2 dispatch method cfg-gated in place. foundation_http H2Serve moved shared/serve → native/serve (native http2 leak). 180/180 tests pass; native + wasm warning-free. |
| 52 | Browser test harness | Implemented, **diverged from design**: shipped `#[valtron_bindgen]` + `#[valtron_wasm_test]` macros (valtron-pool-aware) instead of a bare `#[wasm_bindgen_test]` re-export. `foundation_wasm_testbed` deleted, tests relocated to `foundation_netio/tests/wasm/`, docs updated. CLI shipped as top-level `deno`/`browser`/`bindgen` commands + wasm-bindgen version check (CLI ≥ crate). **Not shipped:** the design's 4-way `test` auto-detect (still legacy mode-based); `bindgen-web` kept with an INTEROP warning, not a formal deprecation. **Not re-verified:** end-to-end Chromium browser run (blocked earlier on a testbed feature-combo build issue). |

## Design proposals — not built

| # | Feature | Status |
|---|---------|--------|
| 50 | Client completion + zero-syscall proxy relay | proposed — design doc only (Part C needs F49; in progress next) |

## Pending

| # | Feature | Depends on | Effort |
|---|---------|-----------|--------|
| 34 | HTTP/3 module | 33, 29 | large |
| 35 | HTTP/3 transport | 34, 22 | large |

## Truly-remaining work (next steps)

1. **F49 / F50** — write-side completion (`IORING_OP_SEND`) + client completion /
   zero-syscall proxy relay. Both are pure design proposals (design docs only).
   **In progress next.**
2. **F52 tail** — optionally add the 4-way `test` auto-detect + formal `bindgen-web`
   deprecation; re-run the end-to-end Chromium browser test to close verification.
3. **F34 / F35** — HTTP/3 module + transport, still unstarted (large).
