# Decision 12: Foundation Enablement — Streaming Writes, Trailers, Header Casing, HTTP/2

## Context

`gaps.md` §1 lists foundation_netio / foundation_http blockers (B1–B5) that ConnectRPC
streaming and gRPC depend on. **We own this code**, so the resolution is to extend the
platform correctly rather than work around it. This decision records the chosen approach
for each blocker. It is the foundation-side counterpart to Decision 11 (which defines the
transport-seam API these capabilities must satisfy).

Guiding constraint throughout: **preserve existing HTTP/1.1 tests and behaviour.** New
capability is added by decomposition and new variants, with the current whole-response
path expressed as a composition of the same parts.

## Decision

### 1. Per-message flush: the handler owns the stream (resolves B1)

No new mechanism is required. The `Serve` handler **owns the connection writer** and may
call `writer.flush()` at any point. Streaming handlers — and the transport-writer task
defined in Decision 11 — flush after each envelope frame so clients receive messages
incrementally. The earlier framing of B1 as a blocker was based on the
render-once-at-the-end path; using the ownership `Serve` already grants removes it.

### 2. Decompose `Http11ResponseIterator` into per-part iterators (resolves B2, B5c#3)

`Http11ResponseIterator` is refactored into a set of smaller **per-part iterators** —
status line, header block, each body chunk, trailers — that compose back into
`Http11ResponseIterator`. Add new `Http11` variants that build a response from this
per-part iterator set, so a streaming handler can:

1. emit status + headers immediately,
2. emit envelope-framed body chunks one at a time (flushing per chunk, per #1),
3. emit a trailers part at the end.

The existing atomic whole-response rendering becomes one composition of these same parts,
so current behaviour and tests are preserved. This same decomposition resolves
foundation_http's `render_response()` HTTP/1.1 hardcoding (B5c#3): the protocol chooses
which part-iterators to compose instead of calling a single fixed renderer.

### 3. Trailers as a response part (resolves B3)

Add a **trailers part-iterator** (new `Http11` variant) and a trailers field on
`SimpleOutgoingResponse`. Each protocol emits trailers through the mechanism it needs,
all via the part composition:

| Protocol | Trailer mechanism |
|---|---|
| Connect | `Trailer-`-prefixed headers in the initial header block (works on HTTP/1.1) |
| gRPC-Web | in-body trailer frame (flag `0x80`) emitted as the final body chunk |
| gRPC | real HTTP/2 trailing HEADERS, emitted by the `http2/` module's writer |

### 4. Stop uppercasing header keys; add protocol headers as known variants (resolves B4)

`SimpleHeader::from(String)` no longer uppercases — header keys preserve canonical case
(HTTP/2 mandates lowercase on the wire). Add the gRPC and Connect protocol headers as
first-class `SimpleHeader` variants with their canonical lowercase wire form:
`grpc-status`, `grpc-message`, `grpc-encoding`, `grpc-accept-encoding`, `grpc-timeout`,
`grpc-status-details-bin`, `te`, `connect-protocol-version`, `connect-timeout-ms`,
`connect-content-encoding`, `connect-accept-encoding`. Audit existing header lookups so
HTTP/1.1 case-insensitive matching (RFC 9110) still holds for inbound parsing.

### 5. New, owned `http2/` module (resolves B5, uses B5a, per B5b/B5d)

Build a dedicated HTTP/2 layer in foundation_netio (shared + native), a **sibling to
`simple_http/`**, taking what is useful as reference but owning the implementation
end-to-end (no tokio). It reuses only the genuinely protocol-agnostic shared types
identified in B5a — `SimpleIncomingRequest`, `SimpleOutgoingResponse`, `SimpleHeaders`,
`SendSafeBody`, `Proto` (`HTTP20` variant), `RawStream`, TLS/SSL wrappers — and owns its
own frame codec, HPACK, stream multiplexer, flow control, and settings (module structure
in the "HTTP/2 module structure" section below). foundation_http's `ConnectionHandler` gains a protocol branch after
negotiation (ALPN / h2c magic prefix); each HTTP/2 stream dispatches independently to the
same `Router`. Handlers see the same types regardless of HTTP version.

### 6. Pseudo-header mapping lives in the `http2/` module (relates B4, T8)

The `http2/` module maps `:method` / `:path` / `:scheme` / `:authority` / `:status` onto
the `SimpleIncomingRequest` / `SimpleOutgoingResponse` fields before producing the
universal types, so handlers never see pseudo-headers, and regular headers stay lowercase.

### HTTP/2 module structure (replicated from h2, tokio-free)

`h2` is the most mature Rust HTTP/2 implementation but depends on tokio
(`AsyncRead`/`AsyncWrite`, runtime context), which conflicts with Valtron. We use `h2`
as a **reference** and reimplement the parts we need tokio-free — not a fork or vendored
copy.

Replicate from h2: HPACK encoder/decoder; frame codec (9-byte headers, frame-type
dispatch, CONTINUATION); flow-control arithmetic (per-stream + per-connection); stream
state machine (idle → open → half-closed → closed); SETTINGS negotiation. Priority tree
optional / deferred.

Key substitutions: `tokio::io::AsyncRead`/`Write` → `std::io::Read`/`Write` on
`SharedByteBufferStream<RawStream>`; `tokio::sync` channels →
`ConcurrentQueueStreamIterator` / crossbeam; `Future`/`Poll` state machines →
iterator-based state machines like `HttpRequestReader`; connection handshake →
synchronous SETTINGS exchange in `ConnectionHandler`.

```
backends/foundation_netio/src/
├── simple_http/          # existing HTTP/1.1 text parser
├── http2/                # new, owned HTTP/2 binary-frame module (sibling to simple_http/)
│   ├── frame/            # 9-byte frame codec, frame types
│   ├── hpack/            # header compression/decompression
│   ├── stream/           # per-stream state machine, multiplexer
│   ├── flow_control.rs   # per-stream + per-connection windows
│   ├── settings.rs       # SETTINGS negotiation
│   └── connection.rs     # connection state, GOAWAY, PING
└── netcap/               # shared TCP/TLS/RawStream (used by both)
```

`ConnectionHandler` in foundation_http branches after protocol detection (ALPN or h2c
magic prefix); both paths produce `SimpleIncomingRequest` / consume
`SimpleOutgoingResponse`. ALPN wiring (rustls `set_protocols(&[b"h2", b"http/1.1"])`) and
h2c detection are part of this module (closes B7); the `HTTPStreams` factory branches on
negotiated protocol (closes B8). Estimated 5–8 features, phaseable: (1) frame codec +
HPACK, (2) server multiplexer, (3) client multiplexer, (4) flow-control tuning.

## Consequences

- **B1, B2, B3, B5c#3 resolved** by handler-owned-stream flush + per-part `Http11`
  iterators + a trailers part. No streaming buffering; trailers work per protocol.
- **B4 resolved** by removing the uppercase transform and adding protocol headers as
  known lowercase variants.
- **B5 / B5b resolved in approach** by the owned `http2/` module; B5c#1 (per-stream
  multiplexing) and B5c#2 (interim-response semantics) are owned by the new HTTP/2
  connection-handler branch, not the HTTP/1.1 state machine.
- **Existing HTTP/1.1 behaviour preserved** — the whole-response path is a composition of
  the new parts; tests must continue to pass unchanged.
- Satisfies the API Decision 11 requires: the writer task emits one body-chunk part and
  flushes per message; the trailers part carries end-of-stream metadata.

## Review-Gap Coverage

### 7. Push-able client request body (resolves B6)
`SendSafeBody::Stream` wraps an iterator; back it with a `ConcurrentQueueStreamIterator`
so the client can **push** request frames after the request has started sending — the
producer half is held by the client-stream / bidi task, the consumer half is drained by
the request renderer. This reproduces connect-go's `io.Pipe` (concurrent request-write
while reading the response) without buffering the whole request, and pairs with Decision
11's `MessageSink`/`MessageSource` pipes. No new body type is needed; the existing
`Stream` variant gains a pushable backing.

### 8. B7 / B8 subsumed by the owned `http2/` module
ALPN wiring and the `HTTPStreams` protocol branch are implemented inside the new `http2/`
module (and its `ConnectionHandler` branch), not retrofitted onto the HTTP/1.1 parser —
so B7 and B8 are not standalone foundation tasks.

### 9. Multi-transport server on netcap's existing `Listener` (resolves Q12 / T11)
We already own centralized transport types in `foundation_netio::netcap::connection`:
- `Connection` enum (`Tcp` / `Unix` / `Tls`) — a unified byte stream (`Read`/`Write`,
  timeouts, `peer_addr`, `try_clone`, `split`, `peek`, `shutdown`), with `netcap/ssl`
  (rustls / openssl / native-tls) behind the `Tls` variant.
- `Listener` enum (`Tcp` / `Unix`) with `accept() -> (Connection, Option<SocketAddr>)`,
  and `ConfigListenAddr` (`IP` / `Unix`) → `bind() -> Listener`.
- `Endpoint<I>` carries a generic **identity** (`WithIdentity`) — the natural home for an
  iroh public key (ties to Q13 / T10).

**Decision:** do **not** introduce a new `Listener` trait at the connectrpc /
foundation_http layer. Instead:
1. **Extend netcap** — add `Quic` and `Iroh` variants to `Connection` / `Listener` /
   `ConfigListenAddr` so every transport is accepted through the one centralized
   abstraction.
2. **Route `HttpServer` through netcap `Listener`.** Today `HttpServer` binds a raw
   `std::net::TcpListener` and wraps each socket via `Connection::from(tcp)` — bypassing
   netcap's `Listener` / `ConfigListenAddr`. Switch it to bind/accept via netcap
   `Listener`, so adding QUIC/iroh is a new variant rather than a new server.
3. **Byte-stream vs multiplexed:** `Connection` remains the byte stream for HTTP/1.1, TLS,
   and individual QUIC/iroh streams. For HTTP/2 and HTTP/3 the multiplexer (`http2/` /
   `http3/`) sits above the accepted connection and fans streams out to the shared
   `Router`. A single server with one `Arc<Router>` serves all listeners; multiple
   listeners (TCP + UDP/QUIC + iroh) share that router.

This keeps the connection types usable and centralized, and turns Q12/T11 into incremental
variant additions on an abstraction we already have.

### 10. Graceful shutdown / connection draining — resolves Decision 08 OQ#3

**Verified today:** foundation_http's `serve_loop` checks `OnSignal::probe()` and **stops
accepting** on shutdown, but does **not drain** — there is no active-connection tracking,
no join after the accept loop, and `ConnectionHandler::new(...)` is not given the shutdown
signal, so keep-alive connections keep serving new requests after shutdown begins. Add
graceful drain in three pieces:

1. **Active-connection tracking:** a shared counter / `WaitGroup` incremented when a
   `ConnectionHandler` is submitted to valtron and decremented when it completes (on drop).
2. **Share the shutdown signal with the valtron tasks (task-decided policy):** clone the
   `Arc<OnSignal>` into every `ConnectionHandler` task so each task *knows* shutdown was
   requested and **decides its own drain policy** at its checkpoints (between keep-alive
   requests, and at safe points within a streaming RPC). A task may choose to **finish all
   work it already accepted** before ending, or to **stop at the next check** — finishing
   the in-flight response with `Connection: close` and not reading another keep-alive
   request. The framework does not forcibly abort the task; shutdown is cooperative. (A
   handler/protocol can therefore let a long stream complete, or cut at the next message
   boundary, as appropriate.)
3. **Drain phase:** after the accept loop breaks, wait for the active count to reach zero
   with a configurable grace timeout (e.g. `ServerConfig::shutdown_grace`), then return;
   force-close whatever remains past the deadline.

This keeps in-flight RPCs (including streaming) running to completion within the grace
window — what ConnectRPC needs for clean shutdown. It is a foundation_http feature,
sequenced with the other Decision 12 enablers.

## Open Questions

1. Exact public shape of the per-part iterator API and the new `Http11` variants
   (deferred to the foundation feature spec). Must not regress the existing atomic path.
2. Is `SimpleOutgoingResponse.trailers` an `Option<SimpleHeaders>` or always present and
   empty by default?
3. HTTP/2 multiplexer scope/phasing (server-side first, then client-side) — tracked with
   B5d's estimate (5–8 features).
4. h2c (cleartext HTTP/2) support in Phase 2, or TLS-ALPN only first?
