# LEARNINGS — ConnectRPC Proxy

_No implementation yet — this file is seeded at spec creation and updated after
each milestone/task. See `.agents/skills/specifications-management/skill.md`._

## Key finding (creation)

A reverse proxy does **not** need to decode protobuf/RPC payloads to route RPC
traffic effectively — but it **does** need to be HTTP/2-faithful: no H2→H1
downgrade, `te: trailers` preserved, and HTTP/2 trailing HEADERS forwarded.
"Just forward it like raw TCP" is only correct for a single opaque backend; the
moment you want routing, load balancing, health, or TLS termination, you need
the HTTP layer to be correct first, and only then consider RPC awareness.

## Decision (2026-10-18)

Ship **B + D**, with D resolving C. B is the transparent, H2-correct HTTP
forward (`http`/`https` endpoints); D embeds `foundation_connectrpc` as the RPC
engine (`connect`/`grpc`/`grpc-web` endpoints) for method routing, stream-level
LB, gRPC health, deadline pass-through, and gRPC-Web ↔ gRPC translation. The
operator selects the path **per endpoint** by declaring the backend protocol,
extending `BackendProtocol` in `foundation_proxy`'s `config.rs`. D is chosen
over C because it reuses connectrpc's existing handlers rather than
re-implementing the wire formats inside the proxy.

## Grounding (2026-10-18)

Wire-format claims are confirmed by the external [Connect protocol
spec](https://connectrpc.com/docs/protocol/) (fetched via `curl`; the page is
static/SSR) and the [gRPC over HTTP/2
spec](https://github.com/grpc/grpc/blob/master/doc/PROTOCOL-HTTP2.md). Key
confirmations: Connect uses *no* HTTP trailers (renders trailing metadata as
`trailer-`-prefixed headers), frames streaming as `Envelope-Flags Message-Length
Message`, and requires H2 only for client-streaming/bidi. gRPC mandates H2,
`te: trailers`, and delivers status in trailing HEADERS.

## Pre-existing gap (2026-10-18) — **closed (T1, 2026-10-20)**

`foundation_connectrpc`'s H2 transport now forwards HTTP/2 trailing HEADERS to
its caller: `TransportStream::trailers` pipes the decoded trailers into the
seam's `ClientConn` / `Response::trailers()`, and `tests/grpc_tests.rs` asserts
`grpc-status` directly in three tests (`grpc_trailers_readable_from_transport_stream`,
`grpc_unary_response_carries_trailers`, `grpc_error_trailers_readable_from_response`).
The stale in-file NOTE that called trailers "not yet verifiable" was corrected.
The proxy's missing-trailer gap (§3 item 3) remains and is now the only side of
the pipe outstanding — it is the Layer-3 red test (T4).

## Milestone — Workstream 1, T1 + T2 (2026-10-20)

- **T1 (verified, no code change needed):** the connectrpc H2 trailer plumbing was
  already in place ahead of the spec's `pending` status; its gate
  (`cargo test -p foundation_connectrpc --test grpc_tests`) is 11/11 green.
  Folded in by correcting the stale NOTE and marking the gap closed above.
- **T2 (done):** added `strip_hop_by_hop_request` in
  `foundation_proxy/src/forward.rs`, used by `forward_request_headers`. It strips
  hop-by-hop headers **except** `te: trailers` (value-gated to the `trailers`
  token), `trailer`, and `trailers` — the trailer handshake gRPC requires
  (RFC 9113 §8.2.2). Gate: `forward_headers_tests` is 4/4 green, including the
  two new tests (`test_request_strip_preserves_grpc_trailer_headers`,
  `test_request_strip_drops_non_trailers_te`).

**Learning — separate the request and response hop-by-hop policy.** `te` /
`trailer` / `trailers` must be preserved on the *request* path (gRPC depends on
`te: trailers`), but a response `Trailer` header is a *sender* announcement that
becomes stale once the body is re-framed on the way back, so the response path
keeps the plain `strip_hop_by_hop`. This is why T2 added a distinct
`strip_hop_by_hop_request` rather than weakening the shared `strip_hop_by_hop`.

## Finding — no production H2 *upstream* client (resolved in T3/T4, 2026-10-20)

T3/T4 assume the proxy can "add an H2 upstream path" by switching clients, but
the H2 client substrate is not there yet:

- `foundation_netio::http2::client::H2Client` (the Feature 29/30 frame-codec
  layer) is synchronous and buffers the whole response body into a `Vec<u8>`
  before returning it (`connection.rs::recv_response`); `H2Request`/`H2Response`
  have no `trailers` field, and a trailing HEADERS frame after DATA is re-parsed
  as a fresh response or dropped.
- `foundation_netio`'s `NativeHttpClient` / `ClientRequestBuilder` is HTTP/1.1
  only — no ALPN, no H2. `foundation_http` is server-side only (`H2Serve`).
- `foundation_connectrpc`'s `H2Transport` is the only streaming + trailer H2
  client (T1 verified its trailer pipe), but it is RPC-typed
  (`Transport`/`TransportStream`) and `foundation_proxy` does not yet depend on
  connectrpc.

So the B leg's H2→H2 forward needs one of: (a) upgrade netio's `H2Client` into a
streaming, trailer-forwarding client, (b) reuse connectrpc's `H2Transport` for
the transparent leg (making connectrpc a real proxy dependency), or (c) a
dedicated H2 client. This is a decision, not a mechanical client swap; it is
recorded before T3 is started so the dependency is explicit (AGENTS.md: don't
work a feature whose dependency isn't met — update the feature instead).

**Decision (2026-10-20): reuse connectrpc's `H2Transport`.** `foundation_proxy`
gains a real dependency on `foundation_connectrpc` and drives the H2 upstream
through its streaming + trailer transport, rather than upgrading netio's
frame-codec `H2Client` or writing a third client. The transparent B leg therefore
rides connectrpc's `Transport`/`TransportStream` types — accepted coupling, since
the D leg already embeds connectrpc.

## Milestone — Workstream 1, T3 + T4 (2026-10-20)

- **T3 (done):** `foundation_proxy/src/h2_proxy.rs` gained
  `async fn forward_upstream_h2`, driven through connectrpc's `H2Transport`. It is
  gated on `is_grpc_request` — `content-type` is `application/grpc` or
  `application/grpc+*` (explicitly excluding `application/grpc-web`, which is
  H1-transparent). The `Connection: close` header that `forward_request_headers`
  inserts is removed on this leg (`headers.remove(&CONNECTION)`) — meaningless
  and wrong on H2. Gate: `h2_integration_tests` green.
- **T4 (done):** the same `forward_upstream_h2` relays trailing HEADERS as
  `H2Frame::Trailers`; a body error relays `H2Frame::Reset{InternalError}` rather
  than a clean EOF; a trailer-less end emits an empty `END_STREAM` DATA frame
  (mirroring the H1 path). The inline `#[cfg(test)] mod tests` was moved out of
  `h2_proxy.rs` into `tests/` (the `h2_status` cases), per AGENTS.md.

**Learning — `is_grpc_request` is the H2-leg trigger, not "front end is H2".**
The spec's phrase "whenever the front end is H2 and the backend is http(s)" cannot
be honoured without silently breaking H2 clients that talk to H1 backends (plain
HTTP/2 through the proxy still lands on an H1 origin). Content-type gating —
"unconditionally for `application/grpc`" — is the concrete, correct trigger: gRPC
*requires* H2 end-to-end, so the media type is a faithful signal. Non-gRPC H2
requests keep the existing H1 path unchanged (the `h2_echo_roundtrip` test proves
it is not regressed).

**Learning — `H2Transport` drops the query string.** `open()` builds `:path` from
`request_uri.path()`, not `path_and_query()`, so a query-bearing H2 forward would
silently lose its query. Invisible for gRPC POST (no query), but a latent trap for
extending the H2 leg to plain HTTP. Tracked in requirements.md Known Issues; fix
belongs in connectrpc, not the proxy.

**Learning — the H2 leg is async-native, the H1 leg is thread-per-stream.** The
two relays (`request_relay` + `response_relay`) run concurrently under
`futures_util::future::join`, so a bidi/streaming call makes progress in both
directions. This is why `forward_upstream_h2` takes `&PipeSender` and awaits
`send` directly, unlike the H1 `forward_upstream` which parks a dedicated OS
thread and uses `try_send`/backoff.

## Milestone — Workstream 1, T5 (2026-10-20)

- **T5 (done):** `forward.rs` gained `is_streaming_request` (true for
  `application/grpc`, `application/grpc+*`, `application/connect+*`,
  `application/grpc-web*`) and `forward_request_headers` now inserts
  `Connection: close` **only** for non-streaming requests. The header is
  unnecessary for RPC streaming responses (always length- or chunk-delimited,
  never EOF-delimited) and hostile to long-lived server-streaming (it forces the
  backend to drop the connection after each stream, defeating reuse).
  `forward_request_headers` became `pub` so the H2 leg shares the policy, and the
  H2 leg's now-redundant `headers.remove(&CONNECTION)` was deleted. Gates:
  `forward_headers_tests` 6/6 (`test_is_streaming_request`,
  `test_forward_headers_relax_connection_close_for_streams`) and
  `h2_integration_tests` 7/7 (`h2_grpc_streams_without_buffering`).

**Learning — time-based sleeps are the wrong way to prove streaming; signal-gate
instead.** The first cut of `h2_grpc_streams_without_buffering` had the backend
send a head + `"first"`, then sleep, then `"second"`. Both `std::thread::sleep`
and valtron's `sleep_async` were wrong for this test:
`std::thread::sleep` blocks a shared valtron worker (no yield), stalling the whole
async pipeline; `sleep_async` (via `execute(SleepingTask, None).into_ready_future()`)
turned out **flaky** — in roughly half the runs it didn't free the worker, and the
client saw the head only after the full gap elapsed (1.5 s), tripping the
"first frame must stream" assertion non-deterministically (seed-dependent pool
scheduling). The robust fix is a **release signal**: the backend holds the stream
open on a `valtron::Pipe<()>` `receive().await` instead of sleeping, the client
asserts `"first"` arrived, *then* sends the release. Deterministic (6/6 runs) and
faster — no wall-clock sleep, so the streamed byte is observable while the stream
is still open. When a test must assert "data arrives *before* the stream ends,"
gate the continuation on an explicit signal, never on a timer.

## Milestone — Workstream 1, T6 (2026-10-20)

- **T6 (done):** new `foundation_proxy/tests/rpc_proxy_e2e_tests.rs` composes the
  real pieces — a `foundation_connectrpc` `Router` + dual-protocol backend
  (`ConnectRpcServe` H1 + `ConnectRpcServeH2` through `ServerApp::both`),
  `foundation_proxy` fronting it, and real typed `Client`s driving the calls
  through the proxy. Gate: 8/8 green. Coverage: Connect unary + server-streaming
  over both the H1 and H2 legs, gRPC unary + server-streaming with `grpc-status`
  intact, and gRPC-Web unary + server-streaming.

**Learning — the e2e suite surfaced two latent bugs, not proxy-logic bugs.** The
per-protocol conformance tests (Layer 1) could not catch these, because they only
manifest when a *streamed* body crosses a real seam end-to-end:

1. **`PipeSender::drop` closed the pipe on any clone's drop (fixed).** `Pipe`'s
   `PipeInner` is shared by sender and receiver behind one `Arc`, and
   `PipeSender::clone` was a shallow `Arc` clone whose `Drop` called
   `queue.close()` unconditionally. `read_grpc_response` clones `tx` to read the
   message frames, then appends the terminal `Frame::EndStream` on the *original*
   `tx`; the clone's drop closed the pipe first, so the trailing `grpc-status`
   frame was written into a closed pipe and lost — the client saw a clean EOF and
   decoded `Ok(default)` (the exact failure §2 warns against). Fix: a dedicated
   `sender_count: AtomicUsize` on `PipeInner` (not `Arc::strong_count`, which is
   wrong because the receiver shares the same `Arc`); `Clone` increments it and
   `Drop` closes only when the count reaches zero (the last sender).

2. **`Http11ResponseIterator::ChunkedBodyStreaming` emitted malformed chunked
   framing (fixed).** It called `ChunkedData::into_bytes()`, whose `Data` arm
   produced `{len}{bytes}` with no `\r\n` delimiters and whose `DataEnded` arm
   produced a bare `0\r\n`. A strict chunked parser therefore saw an empty body
   (the H1-leg server-streaming tests failed with `left: []`). The request
   renderer already did this correctly, so the response renderer was rewritten to
   mirror it: `ChunkedData::Data` → `Http11Chunk::ChunkedExt(bytes, ext).render()`
   (`{len:x}\r\n{data}\r\n`), `DataEnded`/exhaustion → `0\r\n\r\n`.

**Latent bug tracked (not fixed here):** `ChunkedData::into_bytes()` itself is
still the malformed encoder above; the response-renderer fix made it dead code
(no remaining callers in the crate), but it is `pub` and a footgun for any future
or external caller. Scheduled for removal/correction — see requirements.md Known
Issues.

**Pre-existing failures confirmed unrelated.** `proxy_features_tests.rs`
(`sticky_cookie_pins_requests_to_one_backend`,
`websocket_upgrade_relays_and_tunnels_bytes`) fail identically at clean `HEAD`
(verified via a tagged `git stash` round-trip), so they are environment /
raw-socket-timing issues, not T6 regressions. Tracked in requirements.md Known
Issues.

## Milestone — Workstream 2, T7–T13 (2026-10-20)

- **T7 (done):** `BackendProtocol` gained `Connect`/`Grpc`/`GrpcWeb` with
  `Display` strings, inferred from `connect://` / `grpc://` / `grpc-web://`.
  Gate: `backend_target_tests`.
- **T8/T9/T10 (done):** the codec-agnostic frame relay. Inbound
  `GrpcWebHandler` / `ConnectHandler` de-frame opaque message frames; outbound
  `GrpcClient` re-envelopes them over `H2Transport`. T8 is the unary keystone,
  T9 the server-stream + `grpc-web-text` base64 + status fidelity, T10 all four
  Connect↔gRPC stream types (closing the T6 bidi gap). Gate:
  `rpc_proxy_e2e_tests` (17/17).
- **T11 (done):** stream-level LB — each relayed RPC picks a backend at `open()`
  time (one `H2Transport` connection per call), so concurrent streams spread.
  Gate: `rpc_proxy_e2e_tests`.
- **T12 (done):** minimal `grpc.health.v1.Health/Check` client (hand-encoded
  messages) wired into `probe_once()` for `Grpc` backends. Gate:
  `rpc_health_tests`.
- **T13 (done):** deadline pass-through + cancellation. `translate_connect_deadline_to_grpc`
  re-encodes `connect-timeout-ms` → `grpc-timeout`; the streaming relay races a
  shared `CancelSignal` (fired on client reset) against its pump/reader/writer
  tasks; the outbound H2 pump aborts the backend stream with `RST_STREAM` when
  the response consumer goes away. Gate: `rpc_deadline_tests` (2/2).

**Decision — the D leg relays frames, not typed `Client<Req,Res>`.** The proxy
has no upstream protobuf types, so it cannot build connectrpc's generic
`Client<Req,Res>` per method. It forwards at the **frame level**: decode the
inbound protocol into opaque message frames, re-encode them in the backend's
protocol (re-enveloping only — for gRPC-Web-text, base64-decode/encode; for
gRPC, move status between H2 trailing HEADERS and the in-body `0x80` frame).
Method identity is the URL path, which the proxy's `Router` already matches.

**Learning — cancellation has to reach the backend through two hops, each with
its own gap.** A client abort is a wire-level `RST_STREAM`; turning it into a
backend teardown surfaced three independent gaps, all fixed or tracked:

1. **The proxy's body feeder never saw the reset.** For a server-stream, the
   request body already ended cleanly at `END_STREAM`, so `on_data` closed the
   body pipe and the later `Reset` was skipped. The reset instead surfaces as the
   *response* pipe closing, so `relay_connect_streaming_h2` now races the relay
   against `tx.closed()` and fires the shared `CancelSignal`.
2. **The backend's H2 server never saw the socket close.** `SharedByteBufferStream::peek`
   (foundation_core) maps `PeekState::EndOfFile` to `Ok(0)` — indistinguishable
   from `WouldBlock` — so the server parks forever on a peer that half-closed its
   socket. The relay's outbound pump now sends an explicit `RST_STREAM(CANCEL)`
   when its response consumer goes away. The underlying EOF/`WouldBlock`
   conflation is a separate foundation_core bug (tracked in bug-backlog).
3. **The handler can't `await` its own cancellation.** `exchange_until_aborted`
   drops the response side (holding the handler stream) *before*
   `run_streaming_h2` fires `cancel.cancel()`, so a streaming handler's
   `ctx.cancelled().await` never completes on peer disconnect — only
   `ctx.is_canceled()` (read from a parked `Ctx`) is observable. The test
   therefore parks the handler's `Ctx` in a `Slot` and polls `is_canceled()`,
   mirroring `foundation_connectrpc/tests/cancellation_tests.rs`. The drop-before-cancel
   ordering is a connectrpc limitation tracked in bug-backlog.

**Learning — the connectrpc typed client's `cancel()` does not emit a wire
reset.** It cancels the local signal but keeps the H2 socket open, so it cannot
stand in for "the client aborted" in a cancellation test. The test drives a raw
`H2Channel` client that sends a genuine `RST_STREAM` — the faithful HTTP/2 abort
signal. Tracked in bug-backlog (client-side cancellation does not propagate).
