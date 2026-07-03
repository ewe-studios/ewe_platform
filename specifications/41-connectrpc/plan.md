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
3. **Abstract codec system (`CodecFor<M>`)** — Three first-class codecs: buffa (protobuf), serde_json (JSON), foundation_arrow (Arrow IPC). Message type on the trait (`dyn CodecFor<M>` object-safe); registration-time per-procedure `ProcedureCodecs` tables; no unified `Message` trait, no `dyn Any` (Decision 02).
4. **Auth on foundation_auth** — Port authn-go's middleware pattern, delegate verification to foundation_auth's JWT/OAuth/session infrastructure.
5. **errstacks everywhere** — every public signature returns `ConnectResult<T> = Result<T, ErrorTrace<ConnectError>>`; domain errors are custom contexts mapped in via `change_context` (Decision 03).
6. **`Ctx` context handle** — Arc-backed, by-value in all four RPC kinds; bundles foundation_http's `ContextBag` (app-scoped shared deps) + `Arc<RequestContext>` (per-RPC state) (Decision 04).
7. **Async-canonical surfaces** — client stream handles, `Transport::open`, and the response head are async; sync wraps via valtron off-pool (Decisions 07/11; platform norm).

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

## Reference Material

- **Connect protocol spec**: connectrpc.com/docs/protocol.md — full ABNF grammar for wire formats
- **connect-go source**: The mature reference implementation (v1.20.0-dev)
- **buffa**: Pure Rust protobuf with editions, zero-copy views, no_std capable
- **Conformance tests**: 33+ YAML test suites covering all protocols, codecs, compression, streaming, errors, timeouts, TLS

## Crate Structure (Planned)

```
backends/foundation_connectrpc/          # Runtime library

# Codegen is NOT a separate crate (see Decision 10):
#   - one unified generator (messages + service traits + clients) — no split tooling
#   - generation logic lives in  backends/foundation_macros/
#   - a binary in              backends/foundation_netio/  exposes it (CLI / protoc plugin)
```

## Open Questions (Aggregated)

**All design-blocking questions are resolved.** Each decision doc records its resolutions
inline (the Review-Gap Coverage and Open Questions sections are retained as resolution
records) — the questions that used to be listed here (netio HTTP/2 support, buffa canonical
JSON, zero-copy views, error details/debug, sync-vs-async handlers, cancellation, bidi,
gRPC-Web text mode, `google.rpc.Status`, HTTP/1.1 trailers, client types/pooling/streaming
bodies, prefix routing, middleware ordering, mTLS/introspection/CORS, unified codegen,
default `unimplemented` impls, view handler variants) are all decided in Decisions 00–14.
A second cross-consistency pass (2026-07) additionally settled: `CodecFor<M>` typed-codec
dispatch, the `ConnectResult` error sweep, the `Ctx` handle, async-canonical client
surfaces, R1 leading-slash paths, R11 `grpc-web` naming, R12 auth feature-gating, the
frame-level `EnvelopeReader`/`Writer`, and `ConnectionContext` plumbing (Decision 12 §13).

Remaining implementation-time tunables (not design blockers; revisit inside the named
feature):
- Whether the **owned-decode** path reuses a buffer pool (Decision 11 OQ#3) — tune during
  implementation.
- HTTP/2 **flow-control tuning** (Decision 12 §5, phase 3 of the http2 module).
- WASM Fetch **bridge-surface confirmation** (Decision 11 OQ#4) — a foundation_wasm
  implementation detail; capabilities are already fixed (`Duplex::None`, no trailers).
