# Spec 41: foundation_connectrpc — ConnectRPC for the EWE Platform

## Origin

A friend introduced connectrpc to me and i love this:

The site: /home/darkvoid/Boxxed/@formulas/src.rust/src.RPC/src.connect-protocol/
Conformance: /home/darkvoid/Boxxed/@formulas/src.rust/src.RPC/src.connect-protocol/conformance/
The go implementation: /home/darkvoid/Boxxed/@formulas/src.rust/src.RPC/src.connect-protocol/connect-go/
Examples: /home/darkvoid/Boxxed/@formulas/src.rust/src.RPC/src.connect-protocol/examples-go/
Authn: /home/darkvoid/Boxxed/@formulas/src.rust/src.RPC/src.connect-protocol/authn-go/

## Goal

Port ConnectRPC to Rust as `foundation_connectrpc`, built on top of the platform's existing foundation crates (foundation_core, foundation_http, foundation_netio, foundation_auth, foundation_arrow, foundation_errstacks). The connect-go implementation is the primary design reference.

## Key Decisions

1. **Independent implementation** — not a wrapper around connect-rust. connect-go is the sole design reference.
2. **Port to foundation types** — Replace tower/hyper/tokio with foundation_http handlers, foundation_netio HTTP types, valtron execution.
3. **Abstract codec system (`CodecFor<M>`)** — Three first-class codecs: buffa (protobuf), serde_json (JSON), foundation_arrow (Arrow IPC). Message type on the trait (`dyn CodecFor<M>` object-safe); registration-time per-procedure `ProcedureCodecs` tables as the **single codec authority** — no request-time `CodecRegistry`, codec names are the Content-Type wire tokens, options select by name only (`ProcedureCodecs::only(...)` covers single-codec extension endpoints); no unified `Message` trait, no `dyn Any` (Decision 02).
4. **Auth on foundation_auth** — Port authn-go's middleware pattern, delegate verification to foundation_auth's JWT/OAuth/session infrastructure.
5. **errstacks everywhere** — every public **RPC-surface** signature returns `ConnectResult<T> = Result<T, ErrorTrace<ConnectError>>`; domain layers below the surface keep typed errors (`CodecError`/`CompressionError`/`TransportError`) mapped in via `change_context`/`From` at the boundary (Decisions 03/11).
6. **`Ctx` context handle** — owned + cheap-`Clone` (Arc-backed internals; write = COW `with_*` rebuild-and-move, read = share), by-value in all four RPC kinds; bundles foundation_http's `ContextBag` (app-scoped shared deps) + `RequestContext` (per-RPC state) + `CancelSignal` (a clone shares the call's signal; detach/link is explicit, never an implicit tree) (Decision 04).
7. **Async-canonical surfaces** — client stream handles, `Transport::open`, the response head, **and the seam itself** (`HandlerConn`/`ClientConn` via dyn-safe `BoxFuture`; `MessageSink`/`MessageSource` as async fns) are async; flow control is **awaited** (two-sided pipe wake, Decision 00 L1b/00-F4), never surfaced as an error; sync wraps via valtron off-pool (Decisions 00/03/07/11; platform norm).

## Design Documents

| # | Decision | Status |
|---|---|---|
| [00](decisions/00-valtron-async-readiness.md) | Valtron Async Readiness (foundation — async handlers park, not spin) | ready |
| [01](decisions/01-transport-and-runtime.md) | Transport Layer & Runtime Model | ready |
| [02](decisions/02-codec-and-serialization.md) | Codec System & Serialization | ready |
| [03](decisions/03-error-model.md) | Error Model | ready |
| [04](decisions/04-handler-and-interceptor-model.md) | Handler & Interceptor Model | ready |
| [05](decisions/05-protocol-wire-formats.md) | Protocol Wire Formats | ready |
| [06](decisions/06-compression.md) | Compression System | ready |
| [07](decisions/07-client-architecture.md) | Client Architecture | ready |
| [08](decisions/08-router-and-dispatch.md) | Router & Multi-Protocol Dispatch | ready |
| [09](decisions/09-auth-middleware.md) | Authentication Middleware | ready |
| [10](decisions/10-codegen.md) | Code Generation | ready |
| [11](decisions/11-transport-seam.md) | Transport Seam & Streaming Handler Model | ready |
| [12](decisions/12-foundation-enablement.md) | Foundation Enablement (Streaming, Trailers, HTTP/2) | ready |
| [13](decisions/13-websocket-transport.md) | WebSocket Transport (Bidi over HTTP/1.1) — *deferred, implemented last* | ready |
| [14](decisions/14-io-uring-reactor-backend.md) | io_uring Reactor Backend (Linux) — efficient fd listening for native parking | ready |
| [15](decisions/15-bounded-delivery-and-sequenced-lifting.md) | `sequenced` delivery is bounded (backpressured interleave); `lift` stays unbounded — narrow fix, realized in Feature 45 | ready |

## Reference Material

- **Connect protocol spec**: connectrpc.com/docs/protocol.md — full ABNF grammar for wire formats
- **connect-go source**: The mature reference implementation (v1.20.0-dev)
- **buffa**: Pure Rust protobuf with editions, zero-copy views, no_std capable
- **Conformance tests**: 33+ YAML test suites covering all protocols, codecs, compression, streaming, errors, timeouts, TLS

## Crate Structure (Planned)

```
backends/foundation_connectrpc/          # Runtime library
                                          #   features: `rpc` (full single-threaded surface),
                                          #   `rpc_multi` (+ multi executor, default)

# Proto codegen (Decision 10 Modes 1–2) IS a separate build-time crate — the
# tonic/tonic-build split; depends only on the prost toolchain, no edge back to
# the runtime crate; consumed under [build-dependencies]:
backends/foundation_connectrpc_codegen/  # generate_services + build.rs helper (Mode 1)
                                          #   + rpc-gen-proto binary (Mode 2)

# Code-first codegen (Decision 10 Mode 3):
#   - #[connectrpc::service] / generate! proc-macros live in backends/foundation_macros/
#     and are re-exported from foundation_connectrpc
```

## Resolution Record (Aggregated) & Implementation-Time Tunables

**All design-blocking questions are resolved.** Each decision doc records its resolutions
inline under a single **`## Decided Details`** section (no doc has an "Open Questions" or
"Review-Gap Coverage" section anymore — anything so titled would be a regression; pure
pointer/duplicate items were deleted, and every retained item is normative decided
behaviour with a stable label other docs may cite) — the questions that used to be listed
here (netio HTTP/2 support, buffa canonical
JSON, zero-copy views, error details/debug, sync-vs-async handlers, cancellation, bidi,
gRPC-Web text mode, `google.rpc.Status`, HTTP/1.1 trailers, client types/pooling/streaming
bodies, prefix routing, middleware ordering, mTLS/introspection/CORS, unified codegen,
default `unimplemented` impls, view handler variants) are all decided in Decisions 00–14.
A second cross-consistency pass (2026-07) additionally settled: `CodecFor<M>` typed-codec
dispatch, the `ConnectResult` error sweep, the `Ctx` handle, async-canonical client
surfaces, R1 leading-slash paths, R11 `grpc-web` naming, R12 auth feature-gating, the
frame-level `EnvelopeReader`/`Writer`, and `ConnectionContext` plumbing (Decision 12 §13).
A third pass (2026-07) settled: async seam traits (`HandlerConn`/`ClientConn` via dyn-safe
`BoxFuture`), producer-side pipe wake (`QueueVacancyReadiness` + two-sided waker hooks,
Decision 00 L1b/00-F4), owned-`Clone` `Ctx` with `with_*` COW derivations + `CancelSignal`
(clone shares; detach/link explicit), the Extensions travel pathway (move at dispatch;
Arc-valued netio `Extensions`, Decision 12 §13), the client-side `Ctx` contract (Decision 07),
`ProcedureCodecs` as the single codec authority (request-time `CodecRegistry` deleted;
`only()`/`with_codec` value installation; options select by name), zero-copy `Bytes`
envelope decode, the buffer-pool ownership model incl. `freeze`/`PooledFrame` origin-lane
recycling (Decision 06), and the BE batch header (Decision 13).
A fourth pass (2026-07, driven by a cold-context review agent) settled: **split conn
halves** (`ConnReceiver`/`ConnSender`, `ClientSender`/`ClientReceiver` — concurrent
read/write, deadlock-free on bounded pipes), **`Frame`-typed pipes + the named `FramePipe`
primitive** (enveloping/compression owned by the transport tasks only; normalized
`EndStream`), the **per-stream-type capability remodel**
(`request_streaming`/`full_duplex`/`h2_trailers` replace the `Duplex` lattice), the erased
`ProcedureMeta` dispatch view, proto/arrow-only zero-copy variants, the **async `AuthFunc`
+ concrete `AuthInfo` contract** (true async `SessionManager` upstream), `TransportError` +
its `Code` mapping, the `ConnectResult` norm scoped to RPC surfaces, `UnaryCall.codec_name`
threading, cancel-composed pipe parking, and the missing client GET/version options.

A fifth pass (2026-07, driven by a connection-ownership review) settled the **connection
owner** contract, previously implicit: the seam's `ByteSink`/`ByteSource` are only bounded
queues, so the component owning the raw fd must **explicitly spawn the byte pump** that holds
the socket-facing pipe halves and moves bytes ⇄ fd (the caller keeps only `send_body`/`recv_body`).
On the **client** this is `Transport::open` (it spawns the pump — the driven
`ClientRequest::send_async` task — before returning; a lazy drive inside the `response` future
deadlocks a request larger than the pipe depth). On the **server** it is the per-connection task
the listener spawns on `accept` (netio `Serve`/`ServeWriter`). This is the valtron (pull-based)
materialization of what connect-go got for free from `net/http`'s per-request goroutine; the pump
is distinct from the protocol reader/writer tasks. Recorded in Decision 11 (§Connection
ownership), Decision 07 (client), Decision 08 (server §0). It also motivates a **walking-skeleton
feature** — one unary + one streaming RPC over a real loopback socket, both sides — to be built
before finishing the transport/client breadth, so every layer slots into a proven end-to-end spine
(the `PushableRequestBody::into_sender()` netio addition lands here).

Remaining implementation-time tunables (not design blockers; revisit inside the named
feature):
- Whether the **owned-decode** scratch path reuses the buffer pool (Decision 06 §Buffer Pool) —
  write-path frame recycling is already decided (`pool.freeze` → `PooledFrame` origin
  return lane, Decision 06); decode scratch can adopt the same primitive if profiling
  wants it.
- HTTP/2 **flow-control tuning** (Decision 12 §5, phase 3 of the http2 module).
- WASM Fetch **bridge-surface confirmation** (Decision 11 §Decided Details) — a foundation_wasm
  implementation detail; capabilities are already fixed (`request_streaming: false`,
  `full_duplex: false`, `h2_trailers: false`).
