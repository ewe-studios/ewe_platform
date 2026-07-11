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

| 50 | Client completion + zero-syscall proxy relay | **Parts A + B1 + B2 + C-mechanism implemented + tested 2026-07-12** (commits 813d29e7b, b113e706a, 8bd9e5810, 96c2c0f28). A/B1: `iogate::connect_completion` + proxy dial wiring (`ProxyConfig::io_mode`). B2: reactor `wait_for_events` event-generation+condvar primitive (the design's `is_ready`-blocks assumption was false — `is_ready` is non-blocking; built the primitive) + readiness-aware `splice_bidirectional` (parks instead of the 1ms sleep) + WouldBlock-tolerant relay writes. C: `Selector::submit_send_zc` — `IORING_OP_SEND_ZC` two-phase (F_MORE/F_NOTIF) completion, verified on TCP. **Remaining Part C:** wire SEND_ZC into `CompletionSocket` + the true `ProvidedBuf`-direct-handoff relay (measurement-gated). |

## HTTP/3 (F34 done, F35 client-side done)

| # | Feature | Status |
|---|---------|--------|
| 34 | HTTP/3 module (framing + QPACK over QUIC traits) | **complete** — `foundation_netio::http3` (varint/frame/qpack); wire behaviour proved e2e in netio `tests/http3/connection_tests.rs`. |
| 35 | HTTP/3 ConnectRPC transport | **in-progress — client half done.** `H3Transport` + capability matrix + 6–7 capability tests landed. **Remaining = the server half:** `H3Serve` + `ServerApp::Http3` branch, netcap `Quic` variants on `Connection`/`Listener` + routing `HttpServer` through netcap's listener, Alt-Svc + `ConnectionContext` population, then the Connect+gRPC conformance suites over a real HTTP/3 server. |

## Truly-remaining work (next steps)

1. **F50 Part C tail** — wire `submit_send_zc` into `CompletionSocket` as a
   zero-copy write mode, and the true `ProvidedBuf`-direct-handoff relay (send an
   inbound RECV buffer via SEND_ZC with no user-memory copy). Measurement-gated per
   the design. The SEND_ZC *mechanism* is done + tested.
2. **F49 Phase C** — `split_read_write`'s write half as a second-registration
   `CompletionSocket` (the SEND mechanism it needs is done).
3. **F35 server half** — `H3Serve` + `ServerApp::Http3`, netcap `Quic` listener
   variants, Alt-Svc + `ConnectionContext`, then Connect+gRPC conformance over a
   real HTTP/3 server (large; the client transport + F34 module are done).
4. **F52 tail** — optionally add the 4-way `test` auto-detect + formal `bindgen-web`
   deprecation; re-run the end-to-end Chromium browser test to close verification.
