# Decision 12: Foundation Enablement — Streaming Writes, Trailers, Header Casing, HTTP/2

## Context

The review identified foundation_netio / foundation_http blockers (**B1–B5**) that ConnectRPC
streaming and gRPC depend on (originally tracked in the now-removed `gaps.md`; the resolutions
are folded in here). **We own this code**, so the resolution is to extend the
platform correctly rather than work around it. This decision records the chosen approach
for each blocker. It is the foundation-side counterpart to Decision 11 (which defines the
transport-seam API these capabilities must satisfy).

Guiding constraint throughout: **preserve existing HTTP/1.1 tests and behaviour.** New
capability is added by decomposition and new variants, with the current whole-response
path expressed as a composition of the same parts.

> **Hard prerequisite: Decision 00 (valtron async readiness).** The async handler model
> (Decisions 04/11) only parks (instead of busy-polling) once Decision 00 Level 1 lands.
> Sequence Decision 00 first. Relatedly, **native real-I/O parking uses the existing
> `foundation_nativeapis` reactor** (epoll/kqueue, io_uring per Decision 14) via
> `RegisteredFd: EventReadiness` — the bridge already exists (Decision 00 reconciliation); the
> only wiring is `RawStream: AsRawFd` (§12). Without a reachable reactor, native sockets
> re-poll cooperatively rather than parking. On wasm the browser drives wakers, so Level 1
> suffices.

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
per-part iterator set (exact shape in §Decided Details #1), so a streaming handler can:

1. emit status + headers immediately,
2. emit envelope-framed body chunks one at a time (flushing per chunk, per #1),
3. emit a trailers part at the end.

The existing atomic whole-response rendering becomes one composition of these same parts,
so current behaviour and tests are preserved. This same decomposition resolves
foundation_http's `render_response()` HTTP/1.1 hardcoding (B5c#3): the protocol chooses
which part-iterators to compose instead of calling a single fixed renderer.

### 3. Trailers as a response part (resolves B3)

Add a **trailers part-iterator** (new `Http11` variant) and a trailers field on
`SimpleOutgoingResponse` (plain `SimpleHeaders`, empty by default — §Decided Details #2).
Each protocol emits trailers through the mechanism it needs,
all via the part composition:

| Protocol | Trailer mechanism |
|---|---|
| Connect | `Trailer-`-prefixed headers in the initial header block (works on HTTP/1.1) |
| gRPC-Web | in-body trailer frame (flag `0x80`) emitted as the final body chunk |
| gRPC | real HTTP/2 trailing HEADERS, emitted by the `http2/` module's writer |

### 4. Fix uppercased header rendering; add protocol headers as known variants (resolves B4)

The uppercase leak is in the **`Display`/wire-rendering of known `SimpleHeader` variants**
(e.g. `CONTENT-ENCODING`), **not** in `SimpleHeader::from(String)` — `From` already
preserves case for `Custom(..)` and only matches known names case-insensitively. So the fix
targets the variant rendering: emit canonical **lowercase** on the wire (HTTP/2 mandates
lowercase). Add the gRPC and Connect protocol headers as first-class lowercase variants:
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
optional / deferred. The 9-byte-header frame codec is built on the shared
`IncrementalDecoder` primitive (§11) so partial reads on a non-blocking fd resume cleanly.

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

`ConnectionHandler` in foundation_http branches after protocol detection — ALPN, h2c
magic prefix, or `Upgrade: h2c` (all three entry paths, §Decided Details #4); every path
produces `SimpleIncomingRequest` / consumes `SimpleOutgoingResponse`. ALPN wiring (rustls
`set_protocols(&[b"h2", b"http/1.1"])`) and h2c detection/upgrade are part of this module
(closes B7); the `HTTPStreams` factory branches on negotiated protocol (closes B8).
Estimated 5–8 features, phaseable: (1) frame codec + HPACK + SETTINGS + flow-control
arithmetic, (2) server + client multiplexers including the h2c entry paths (one phase —
§Decided Details #3/#4), (3) flow-control tuning.

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

## Decision (continued: §7–§13)

### 7. Push-able client request body (resolves B6)
`SendSafeBody::Stream` wraps an iterator; back it with the 00-F4 pipe primitive
(`Pipe<Bytes>` — waker-hooked, both readiness accessors; NOT `ConcurrentQueueStreamIterator`,
whose polling handoff 00-F4 exists to remove) so the client can **push** request bytes after
the request has started sending — the producer half is held by the client-stream / bidi
task, the consumer half is drained by the request renderer. This reproduces connect-go's `io.Pipe` (concurrent request-write
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

### 10. Graceful shutdown / connection draining (required by Decision 08's router)

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

### 11. Resumable frame decoding — a shared foundation primitive (generalizes Decision 13 E1)

Every frame-oriented protocol we run on a non-blocking fd hits the same wall: a frame can
span multiple reads, so a decoder that consumes part of a header/payload and then sees
`WouldBlock` must **save its progress and resume**, not error. The existing WebSocket
`decode` tolerates a short read only on the first header byte and otherwise raises
`ProtocolError("stream corrupted")` (Decision 13 gap E1). The owned `http2/` frame codec
(9-byte headers + payload, §5) has the *identical* requirement, as will HTTP/3 framing and
the Connect/gRPC **envelope reader** (5-byte prefix + body, Decision 05). So this is not a
WebSocket concern — it is a foundation primitive.

**Decision: define one generic incremental-decoder seam in foundation, and implement each
wire codec on top of it.**

```rust
/// Drives a frame/record decoder one readable-chunk at a time. Holds partial state
/// across calls; never errors on a short read.
pub trait IncrementalDecoder {
    type Frame;
    /// Feed currently-available bytes (may be empty). `Pending` = need more bytes,
    /// state retained. Errors are reserved for genuine protocol violations.
    fn step(&mut self, src: &mut impl std::io::Read) -> Result<DecodeStep<Self::Frame>, DecodeError>;
}
pub enum DecodeStep<F> { Pending, Frame(F) }
```

- Lives in foundation (alongside the `SharedByteBufferStream` IO utilities) so
  `simple_http`, `http2/`, the WebSocket layer, and the Connect envelope reader all share it.
- A small **accumulating buffer** type (carry leftover bytes between `step`s, expose a
  contiguous view) is the reusable core; each codec is a state machine over it.
- Composes with Decision 00: when `step` returns `Pending` on an empty socket, the driving
  task parks on the native reactor — `Depends(Arc<RegisteredFd>)` via `foundation_nativeapis`
  (`RegisteredFd: EventReadiness`), or the L1 wake-queue (`Depends(QueueReadiness)`) for an
  in-process producer; falls back to timeout-poll if no reactor is reachable. Same parking
  story for **all** wire codecs, not just WS.
- **Backward compatible:** the existing blocking `decode` / envelope reads become
  `loop { step }`-until-`Frame` wrappers, so today's callers are unaffected.

Decision 13 E1 (`WebSocketFrameDecoder`) becomes *the WebSocket implementation of this
trait*; the `http2/` frame codec and the Decision 05 envelope reader adopt it too. This
removes three bespoke partial-read handlers in favor of one tested primitive.

### 12. Expose `AsRawFd` on `netio` streams (reactor prerequisite)

The native parking model (Decision 00 L2 / Decision 13 E2 / Decision 14) registers a
connection's socket fd with the `foundation_nativeapis` reactor to obtain a
`RegisteredFd: EventReadiness`. That requires the raw fd, but `netcap::RawStream` currently
exposes none — it wraps `Connection` inside `BufferedReader<BufferedWriter<…>>` with no fd
accessor. Since the reactor lives in a sibling crate (`foundation_nativeapis`), the fd must
be reachable from **above** `netio` (the ConnectRPC crate) to register it.

**Decision: add `AsRawFd` (and `AsFd`) to `netcap::RawStream` / `Connection`**, delegating to
the inner socket:

- `Connection::Tcp(std::net::TcpStream)` → the stream's `as_raw_fd()`.
- TLS variants (`AsServerTls`/`AsClientTls`) → the fd of the **underlying** TCP socket the TLS
  session wraps (readiness is a property of the socket, not the TLS layer).
- Non-socket / wasm variants → no impl (the fd path is native-socket only; wasm parks via the
  browser, Decision 00 L1).

This is additive (a trait impl, no behavior change) and is the single missing seam between
`netio`'s streams and the `nativeapis` reactor. With it, the ConnectRPC layer can build a
`RegisteredFd` from any live `RawStream` and hand the task an `EventReadiness` to `Depends`
on — no dependency inversion required.

### 13. `ConnectionContext` on `SimpleIncomingRequest` (enabler for Decision 04 Q13)

Decision 04 Q13 decides a typed `ConnectionContext` — peer identity via netcap `Endpoint<I>`
(incl. the iroh Ed25519 key, T10), TLS/mTLS peer certificates (Decision 09 R15), negotiated
ALPN / HTTP version, 0-RTT flag, QUIC connection id — that `SimpleIncomingRequest` carries
and `RequestContext` references. The type and the carrying field are **netio changes**, so
they are owned here as a foundation enabler:

- define `ConnectionContext` in `foundation_netio` (netcap — it is transport-scoped, built
  where the connection is accepted/negotiated);
- each connection front end populates it **once per connection**: `simple_http/` at
  accept/TLS-handshake time, `http2/`/`http3/` at connection setup (shared across that
  connection's multiplexed requests; the per-request stream id stays request-scoped), and
  the WebSocket upgrade path at `101` time (Decision 13);
- `SimpleIncomingRequest` gains an `Arc<ConnectionContext>` field, defaulting to an empty
  context for callers that construct requests directly (tests, wasm client rendering) —
  additive, no behavior change for existing HTTP/1.1 tests.
- **`Extensions` values become `Arc<dyn Any + Send + Sync>`** (from `Box<dyn Any + …>`), so
  the map is cheaply clonable — required by Decision 04's owned-`Clone` `Ctx` / COW write
  model (`Ctx::with_extension` clones the map by pointer bumps). `insert<T>` wraps in
  `Arc::new` internally, so call sites are unchanged, and `Arc::from(Box)` migrates any
  boxed values for free. This is the same map the auth middleware writes into and dispatch
  **moves** into `RequestContext` (Decision 04 §Extensions pathway / Decision 09). No
  boxed-insertion API is needed: auth stores the one concrete `AuthInfo` type via a normal
  typed `insert` (Decision 09 A6 contract).

## Decided Details

1. **Per-part `Http11` variants — resolved.** Fine-grained parts *and* a combined head:
   users compose part-by-part or use the convenience variant.

   ```rust
   pub enum Http11 {
       // ── existing (unchanged) ──
       Request(SimpleIncomingRequest),
       RequestDescriptor(RequestDescriptor),
       RequestBody(SimpleIncomingRequest),
       Response(SimpleOutgoingResponse),      // atomic; now composed from the parts below

       // ── new per-part variants ──
       ResponseStatusLine(Status),            // "HTTP/1.1 200 OK\r\n"
       ResponseHeaders(SimpleHeaders),        // header block + terminating CRLF
       ResponseHead(SimpleResponse<()>),      // convenience: status line + header block in one
       ResponseBodyChunk(Http11Chunk),        // one body chunk
       ResponseTrailers(SimpleHeaders),       // last-chunk marker + trailer block
   }

   pub enum Http11Chunk {
       Chunked(Vec<u8>),  // iterator emits {len:x}\r\n…\r\n framing
       Raw(Vec<u8>),      // content-length / close-delimited passthrough
   }
   ```

   - `ResponseHead` carries **`SimpleResponse<()>`** — the same head type the client seam
     reads (Decision 07 §Transport — verified netio types), so head shape is symmetric across client/server. Interim 1xx
     responses are just `ResponseHead` emitted more than once before the final head.
   - **Chunked wire framing lives in the part-iterator**, not the protocol: protocols pick
     `Http11Chunk::Chunked` vs `::Raw`; the `{len:x}\r\n…\r\n` syntax stays in netio (the
     B5c#3 goal — no protocol hand-rolls HTTP/1.1 wire format).
   - `ResponseTrailers` emits the `0\r\n` last-chunk marker + trailer block + final CRLF
     (HTTP/1.1 trailers exist only in chunked mode, so the stream terminator belongs to this
     part; a raw/no-trailer stream just ends). Connect and gRPC-Web never use it (§3 table);
     it exists for protocol completeness.
   - Per-part iterators (`Http11ResponseHeadIterator`, `Http11ChunkIterator`,
     `Http11TrailersIterator`, …) are the composition units; the existing
     `Http11ResponseIterator` becomes their composition — the atomic path and its tests are
     preserved, not regressed.
2. **`SimpleOutgoingResponse.trailers` — resolved: plain `SimpleHeaders`, empty by default**
   (matches the `headers` field convention on the same struct). Writers emit the trailer
   part iff `!trailers.is_empty()`. `Option` was rejected: "no trailers" vs "empty trailers"
   is wire-indistinguishable in every protocol here (HTTP/1.1 chunked ends `0\r\n\r\n`
   either way; h2 sends no trailing HEADERS frame when empty; gRPC trailers are never empty
   — `grpc-status` is mandatory and supplied by the protocol layer), so the extra state
   encodes a distinction nothing consumes.
3. **HTTP/2 multiplexer phasing — resolved: server + client multiplexers built in one
   phase** on top of the shared substrate. Phases: (1) frame codec + HPACK + SETTINGS +
   flow-control arithmetic (direction-neutral substrate), (2) server **and** client
   multiplexers together — the client multiplexer conformance-tests against our own server
   in addition to external peers (grpcurl, connect-go), and gRPC client + server land in the
   same phase, (3) flow-control tuning; priority tree stays deferred. Tracked with B5d's
   estimate (5–8 features).
4. **h2c — resolved: full h2c support, all three HTTP/2 entry paths ship with the `http2/`
   module.**
   - **TLS-ALPN:** rustls `set_protocols([b"h2", b"http/1.1"])`; the negotiated protocol
     decides the branch during the handshake — no application bytes needed.
   - **h2c prior-knowledge:** on cleartext connections `ConnectionHandler` peeks ≤ 24 bytes
     via netcap's existing `Connection::peek()`; prefix `PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n`
     → h2 connection state machine, else → HTTP/1.1 parser undisturbed (`PRI` is not a real
     HTTP method — no collision). Client side needs no detection: it writes the preface +
     SETTINGS on a plain socket (`http://` URL + preferred version ≥ h2 via `ClientOptions`
     / Decision 11 capability matching). This is what `grpcurl -plaintext`, connect-go h2c,
     and the conformance runner's `TLS: false` matrix speak — the full gRPC test matrix runs
     cert-free on a bare TCP socket, and the wire is inspectable without TLS keylogging
     while bringing up the from-scratch frame codec / HPACK / multiplexer.
   - **`Upgrade: h2c` (RFC 7540 §3.2):** the HTTP/1.1 Upgrade handshake is also supported —
     client sends `Connection: Upgrade, HTTP2-Settings` + `Upgrade: h2c`; server replies
     `101 Switching Protocols`, switches to h2 framing, and replays the initiating HTTP/1.1
     request as **h2 stream 1**, answering it through the multiplexer. Although RFC 9113
     deprecated this mechanism and modern gRPC/Connect tooling is prior-knowledge-only, we
     keep full-parity coverage of the h2c surface; the h1→h2 bridge (h1-parsed request
     entering h2 stream accounting, response rendered by the h2 writer) is part of the
     server-multiplexer feature. Same limits (SETTINGS, header-list size, flow-control
     windows) enforced on all three paths.
