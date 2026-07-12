# Spec 41 Progress

Last updated: 2026-07-12 (F35 server half, F49 Phase C, F50 Part C tail, io_uring config gate)

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
| 49 | Write-side completion (IORING_OP_SEND) | **All phases implemented 2026-07-12.** Phases A+B: `SendPool`/`SendBuf`, SEND-CQE drain, zero write-syscalls (ptrace). **Phase C:** `CompletionSocket::split()` → `ReadHalf`/`WriteHalf` on dup'd fds, each with independent reactor registration. |
| 51 | Unified HTTP+WebSocket client | **Wasm criterion MET** (commit fb3d86744): `foundation_connectrpc` compiles clean for `wasm32-unknown-unknown`. `SimpleHttpClient` alias-only, `H1Transport` holds `Arc<dyn HttpClient>`, no `NoSpawner`, `open_websocket` auto-switches native/wasm. **`+ Clone` criterion WAIVED** as cosmetic (28 inert bounds; stack monomorphises at `BoxedDnsResolver` where `Clone` = `Arc::clone`). |
| 51b | connectrpc shared/native restructure | Crate split into `shared/` (codec, compression, context, envelope, error, error_writer, interceptor, message, protocol, client, router, transport incl. H1+WS) + `native/` (server, h2_serve, transport h2/h3). h2 dispatch method cfg-gated in place. foundation_http H2Serve moved shared/serve → native/serve (native http2 leak). 180/180 tests pass; native + wasm warning-free. |
| 52 | Browser test harness | Implemented, **diverged from design**: shipped `#[valtron_bindgen]` + `#[valtron_wasm_test]` macros (valtron-pool-aware) instead of a bare `#[wasm_bindgen_test]` re-export. `foundation_wasm_testbed` deleted, tests relocated to `foundation_netio/tests/wasm/`, docs updated. CLI shipped as top-level `deno`/`browser`/`bindgen` commands + wasm-bindgen version check (CLI ≥ crate). **Not shipped:** the design's 4-way `test` auto-detect (still legacy mode-based); `bindgen-web` kept with an INTEROP warning, not a formal deprecation. **Not re-verified:** end-to-end Chromium browser run (blocked earlier on a testbed feature-combo build issue). |

| 50 | Client completion + zero-syscall proxy relay | **All phases implemented 2026-07-12.** Parts A+B1+B2: `connect_completion` + proxy dial wiring + reactor `wait_for_events` + readiness-aware splice. **Part C mechanism + tail:** `submit_send_zc` two-phase + `submit_send_zc_direct` (`ProvidedBuf`-direct handoff — no user-memory copy). **Config gate:** `CompletionSocket` with explicit `ReadMode`/`WriteMode` enums (Std/Send/SendZc), every combination testable. |

## HTTP/3 (F34 done, F35 client-side done)

| # | Feature | Status |
|---|---------|--------|
| 34 | HTTP/3 module (framing + QPACK over QUIC traits) | **complete** — `foundation_netio::http3` (varint/frame/qpack); wire behaviour proved e2e in netio `tests/http3/connection_tests.rs`. |
| 35 | HTTP/3 ConnectRPC transport | **Server half implemented 2026-07-12.** Client transport + capability matrix already done. **Server half:** `H3Serve` trait (foundation_http, `quic` feature), `ConnectRpcServeH3` (connectrpc, `h3` feature), `dispatch_h3` (poll-based adapter over `H3Request::poll_*`), `ServerApp::Http3`/`Any` variants. Unary works through shared dispatch pipe; body streamed chunk-by-chunk via `AsyncSendSafeBody` (no buffering). `AsyncSendSafeBody::Item` uses `SendableBoxedError` (Send+Sync) for await-safe streaming. Streaming returns 501. H2 dispatch similarly streams one DATA frame per chunk via `SendSafeBodyBytesIterator`. **Remaining:** netcap `Quic` listener variants, Alt-Svc + `ConnectionContext`, conformance suite over real H3 server. |

## Truly-remaining work (next steps)

1. **F35 streaming** — H3 server/bidi/client-stream handlers need a QUIC-aware pump
   (server analogue of `H3Pump`). Currently returns 501.
2. **F35 netcap Quic listener** — `Quic` variants on `Connection`/`Listener`/
   `ConfigListenAddr`, routing `HttpServer` through netcap's listener, Alt-Svc +
   `ConnectionContext` population, Connect+gRPC conformance suites over a real H3
   server.
3. **F52 tail** — optionally add the 4-way `test` auto-detect + formal `bindgen-web`
   deprecation; re-run the end-to-end Chromium browser test to close verification.
