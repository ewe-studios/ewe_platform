# Decision 13: WebSocket Transport (Bidi over HTTP/1.1)

> **Status: deferred — scheduled last.** This is the final transport to implement, after
> Phase 1 (HTTP/1.1), Phase 2 (HTTP/2), and the HTTP/3 / iroh work in Decision 01. It is
> additive: nothing else depends on it, and it does not change the default HTTP/1.1 behavior.

## Context

Decision 01 / S3 / OQ#4 reject **bidirectional streaming over plain HTTP/1.1** with `505`,
because a raw HTTP/1.1 request/response body is **half-duplex**: the client must finish
sending its body before the server may stream its response. Full-duplex bidi there is
genuinely impossible without changing the framing.

A **WebSocket `Upgrade`** (RFC 6455) sidesteps this. The connection *starts* as an HTTP/1.1
request with `Upgrade: websocket` + `Connection: Upgrade` + `Sec-WebSocket-Key`; the server
replies `101 Switching Protocols`, and from that point the same TCP connection is a
**full-duplex, message-framed channel**. Both peers may send frames at any time. That is
exactly the shape of bidirectional RPC.

Crucially, **a WebSocket connection is our transport seam** (Decision 11): an inbound message
stream and an outbound `ConcurrentQueue`. So this slots in as just another `Transport`
implementation — it validates the spec's central thesis that the Connect/gRPC protocols are
abstracted from the transport and plug-and-play on top of any full-duplex carrier.

We already own the WebSocket layer in `foundation_netio/src/websocket/`. A detailed read
(2026) shows it is **strong but not turnkey** for our needs — what exists, and what must be
enhanced, is recorded precisely below so the feature work is scoped honestly.

**Ready and RFC-correct (reuse as-is):**
- `shared::frame` — `WebSocketFrame`, `Opcode`, encode/decode, masking, extended lengths,
  `validate`, `to_message`.
- `shared::assembler` — `MessageAssembler`: full fragmentation with size limits and
  *incremental* UTF-8 validation across fragments; tolerates interleaved control frames.
- `shared::handshake` — `compute_accept_key`, `generate_websocket_key` (wasm-clean RNG via
  `foundation_compact`, not `getrandom`), `build_upgrade_request`, `validate_upgrade_response`.
- `native::task::WebSocketTask` (**client**) — a complete progress-driven `TaskIterator`:
  drains an outbound `delivery_queue`, auto-Pongs, runs the assembler, handles Close,
  zero-copy `decode_with_buffer`. `MessageDelivery::queue() -> &Arc<ConcurrentQueue<…>>` **is
  already an outbound seam queue**. `ReconnectingWebSocketTask` forwards the full `TaskStatus`
  set incl. `Depends`/`Wait`.
- `native::server::WebSocketUpgrade` — `is_upgrade_request`/`extract_key`/
  `extract_subprotocols`/`accept` (and `accept` **echoes the negotiated
  `Sec-WebSocket-Protocol`**).

**Gaps that block a robust RPC server transport (enhanced by this decision):**

1. **Read model is timeout-poll, not reactor-parking.** `WebSocketTask` sets a read timeout,
   does a blocking `decode_with_buffer`, and on `WouldBlock`/`TimedOut` returns
   `TaskStatus::Delayed(..)` — it never emits `Depends`/`QueueReadiness`. So a worker is tied
   up for up to `read_timeout` per poll and the stack does **not** use Decision 00. *(Works
   today; we enhance it to `Depends` where it pays off — see E2.)*
2. **The frame decoder cannot resume a partial frame.** `decode`/`decode_with_buffer` tolerate
   `WouldBlock`/`TimedOut` only on the **first header byte**; once consumed, any such error is
   converted to `ProtocolError("stream corrupted")`. It assumes the whole frame arrives within
   one read-timeout window (kernel-buffered). On a *true* non-blocking fd a large frame splits
   across reads → false "corruption". A **resumable/buffered decoder** is required before WS
   can ride the Decision 00 reactor path (E1).
3. **Server side is blocking convenience only.** `WebSocketServerConnection::recv()` is
   `recv_frame()? .to_message()` — **no assembler** (a `Continuation` frame errors by design in
   `to_message`, so multi-frame messages fail; the "handles frame assembly" doc comment is
   wrong), **no auto-Pong on read**, **no delivery-queue seam**, and only partial Close
   handling. There is **no progress-driven server task** mirroring `WebSocketTask`. (E3.)

So this decision is **wiring + three targeted enhancements** (E1–E3), not a from-scratch
implementation. We own the code; we enhance the existing types and add a robust server task.

## Decision

**Add `WebSocketTransport` as an additional, opt-in transport that carries Connect-style
streaming (including full-duplex bidi) over a WebSocket connection — implemented last.**

### Wire mapping

- **Handshake / routing.** The upgrade request's path selects the procedure (same routing as
  Decision 08). The server performs the `101` upgrade via the existing
  `foundation_http` upgrade path + `foundation_netio` handshake helpers.
- **Framing.** Each RPC message is a Connect **enveloped frame** (Decision 05) carried in a
  **binary** `WebSocketMessage`. We reuse the existing envelope reader/writer — the WebSocket
  layer only supplies message boundaries; we do **not** invent a new envelope.
  - Open question OQ#13.1: whether to use one WS binary message per envelope (simplest) or
    let a single WS message contain a batch of envelopes. Default: **one envelope per WS
    binary message.**
- **Direction.** On the **client**, outbound goes through `MessageDelivery` / its
  `ConcurrentQueue` and inbound arrives from the `WebSocketTask` stream. On the **server**,
  the `WebSocketServerTask` (E3) exposes the `inbound`/`outbound` `ConcurrentQueue` pair
  directly. Either way this is the Decision 11 `MessageSource` / `MessageSink` pair with a
  WebSocket-shaped backing — no new seam concept. (The blocking `recv()`/`messages()` APIs are
  *not* the RPC path.)
- **Trailers / end-of-stream.** Connect streaming end-of-stream is an `EndStreamResponse`
  envelope (Decision 05), carried as a final binary message; the WebSocket `Close` frame is
  the transport-level teardown, mapped to/from cancellation (Decision 04, Option B).
- **Control frames.** `Ping`/`Pong` are handled by the WebSocket layer for keepalive and are
  **not** surfaced to handlers. `Close` (with code/reason) maps to stream completion or
  cancellation. Auto-Pong, the delivery-queue seam, and Close-handshake completion are
  **config-gated** on the server (see E3 / `WsServerConfig`).

### Foundation enhancements (E1–E3) — designed here, built as features

These upgrade the existing `foundation_netio` WebSocket layer; we own it and enhance in place
rather than vendoring or forking.

**E1 — Resumable frame decoder (`WebSocketFrameDecoder`).**
This is the **WebSocket implementation of the generalized `IncrementalDecoder` primitive**
defined in **Decision 12 §11** (the same partial-read seam used by the `http2/` frame codec,
HTTP/3 framing, and the Connect envelope reader). It replaces the one-shot
`decode`/`decode_with_buffer` assumption ("whole frame within one read-timeout window") with a
**stateful decoder that holds partial-frame progress across polls**:

```rust
pub enum FrameStep { Pending, Complete(WebSocketFrame) }

pub struct WebSocketFrameDecoder {
    // phase: reading header / extended-len / mask-key / payload(remaining)
    // carries a growable buffer of bytes received so far for the in-flight frame
}
impl WebSocketFrameDecoder {
    /// Feed whatever bytes are currently available (may be empty). Never errors on a
    /// short read — returns `Pending` and keeps state. Errors only on protocol violations.
    pub fn step(&mut self, src: &mut impl Read) -> Result<FrameStep, WebSocketError>;
}
```

- On a non-blocking fd, `WouldBlock` mid-frame is **normal**: the decoder saves what it has
  and returns `Pending` (no "stream corrupted"). When more bytes arrive it resumes.
- Backwards compatible: the existing blocking `decode` becomes a thin
  `loop { step }`-until-`Complete` wrapper for callers that want the simple API.
- Enforces the same size limits and control-frame rules as today.

**E2 — `Depends` read model via the existing native reactor (opt-in, enhances E1).**
With a resumable decoder, both client and server tasks can stop timeout-polling and **park**:
when `step` returns `Pending` because the socket has no bytes, the task parks on socket
readiness and is woken when readable. The reactor and the bridge **already exist** in
`foundation_nativeapis` — `native::poll` (epoll/kqueue) + `native::fd::RegisteredFd<T:
AsRawFd>` / `FdRegistration`, which implement `EventReadiness`. So the task simply holds an
`Arc<RegisteredFd<…>>` and returns `TaskStatus::Depends(registered_fd)`; no new
`ReadinessSource` abstraction is required (Decision 00 reconciliation).
- **Prerequisite:** expose `AsRawFd` on `netio`'s `RawStream`/`Connection` (**Decision 12
  §12** — delegates to the inner `TcpStream`; for TLS, the underlying socket fd) so the upper
  layer can register it. The unused optional `foundation_netio` dep was removed from
  `foundation_nativeapis`, so the reactor is reachable either via `netio → nativeapis` or by
  wiring at the ConnectRPC crate.
- **Backend:** epoll/kqueue today; **io_uring on Linux** for high connection counts via
  **[Decision 14](14-io-uring-reactor-backend.md)** — same `EventReadiness` seam, same task
  code.
- Gated by availability: if no reactor is reachable (e.g. wasm), the task falls back to the
  **existing timeout-poll + `Delayed`** behavior — no regression. This is the "enhance with
  `Depends` if it makes sense" path, tied to **Decision 00**.

**E3 — Robust server task (`WebSocketServerTask`) + `WsServerConfig`.**
Add a progress-driven server-side `TaskIterator` (mirroring `WebSocketTask`'s `Open` state,
but with **server** semantics: don't mask outgoing, reject unmasked client data frames — that
check already exists in `recv_frame`):

```rust
pub struct WsServerConfig {
    pub auto_pong: bool,            // default true — answer client Ping with Pong
    pub max_message_size: usize,    // assembler limit
    pub graceful_close: bool,       // default true — complete the Close handshake
    pub read_model: ReadModel,      // Depends (if reactor present) | TimeoutPoll
    pub inbound: Arc<ConcurrentQueue<WebSocketMessage>>,   // seam in
    pub outbound: Arc<ConcurrentQueue<WebSocketMessage>>,  // seam out
}
```

- Uses `MessageAssembler` so multi-frame messages assemble (fixes the server gap).
- Drains `outbound` to send, pushes assembled inbound messages to `inbound` — the dual
  `ConcurrentQueue` pair *is* the Decision 11 seam on the server side.
- Auto-Pong, delivery-queue seam, and full Close handshake are **enabled per
  `WsServerConfig`** (you asked for config-gating). The existing blocking
  `WebSocketServerConnection` stays as the simple convenience API; the new task is the robust
  RPC path. We may also retrofit `WebSocketServerConnection::recv` to use the assembler for
  correctness even in the blocking API.

### What it enables

- **Full-duplex bidi on HTTP/1.1 deployments** (browsers, HTTP/1.1-only proxies) without
  HTTP/2 — the one capability the `505` denies on plain HTTP/1.1.
- A second validation point for the transport seam: the same handlers, codecs, interceptors,
  and router run unchanged; only the `Transport` impl differs.

### Scope boundary — non-standard

This is **not** part of the official Connect or gRPC-Web wire specs (neither defines a
WebSocket mapping; the `improbable-eng` grpc-web-over-WS scheme is its own non-standard
protocol and is **not** adopted here). Consequences:

- WebSocket bidi **interoperates only when both peers are our stack** (our generated client +
  our server). It is **not** a way to talk to upstream connect-go / connect-web clients.
- It is **additive**: the default for standard HTTP/1.1 remains the `505` rejection of bidi
  (Decision 01 / S3). WebSocket is selected explicitly (distinct transport / endpoint), never
  as a silent fallback.
- Codegen (Decision 10) is unaffected: handler and client signatures are identical; the
  transport is chosen at wiring time.

## Consequences

- bidi becomes available on HTTP/1.1-only environments, at the cost of a non-standard wire —
  acceptable because it is opt-in and our-stack-to-our-stack.
- Reuses `foundation_netio` WebSocket framing and `foundation_http` upgrade; no new external
  dependency.
- Sits cleanly behind the Decision 11 seam, so it does not perturb the protocol/codec/handler
  layers.
- **E1 (resumable decoder) is generalized, not WS-private:** it is the WebSocket impl of the
  shared `IncrementalDecoder` primitive (Decision 12 §11), reused by `http2/`, HTTP/3, and the
  Connect envelope reader — one tested partial-read seam instead of per-protocol handlers.
- **E2 ties WS to the native reactor:** true parking on native uses `foundation_nativeapis`'s
  reactor via `RegisteredFd: EventReadiness` (Decisions 00/14) once `RawStream: AsRawFd`
  (Decision 12 §12) is wired; until then WS uses the existing timeout-poll fallback with no
  regression.
- **Sequencing:** implemented **last** among transports; nothing in Phases 1–3 depends on it.
  Within this decision, order is **E1 → E3 → (E2 when Decision 00 reactor lands) → 13-F1**.

## Open Questions

- **OQ#13.1** — one envelope per WS binary message vs. batched envelopes per message.
  *Tentative:* one-per-message.
- **OQ#13.2** — whether to expose a WebSocket transport for **unary/server-stream** too (for
  uniformity on HTTP/1.1) or restrict it to bidi/client-stream where it actually adds
  capability. *Tentative:* allow all four kinds over WS for uniformity, but bidi is the
  motivating case.
- **OQ#13.3** — subprotocol negotiation: advertise a `Sec-WebSocket-Protocol` token (e.g.
  `connect-rpc`) so peers can detect compatibility during the handshake.
- **OQ#13.4** — should the blocking `WebSocketServerConnection::recv` be retrofitted with the
  assembler (correctness for the simple API), or left blocking-convenience-only with the
  robust path being `WebSocketServerTask`? *Tentative:* retrofit `recv` for correctness; steer
  RPC users to the task.

## Features

| # | Feature | Depends on |
|---|---|---|
| 13-E1 | **Resumable frame decoder** `WebSocketFrameDecoder` — WS impl of the Decision 12 §11 `IncrementalDecoder`; partial-frame state across reads; `WouldBlock` mid-frame → `Pending`, not "corrupted"; blocking `decode` becomes a `step`-loop wrapper | Decision 12 §11 |
| 13-E3 | **`WebSocketServerTask` + `WsServerConfig`** — progress-driven server `TaskIterator`: assembler, dual `ConcurrentQueue` seam, config-gated auto-Pong / graceful Close; retrofit blocking `recv` to assemble (OQ#13.4) | 13-E1 |
| 13-E2 | **`Depends` read model** — client + server tasks park on socket readiness via the native reactor (`RegisteredFd: EventReadiness`, Decisions 00/14) when no bytes; fall back to timeout-poll + `Delayed` when no reactor is reachable | 13-E1, **Decisions 00/14**, Decision 12 §12 |
| 13-F1 | **`WebSocketTransport` (server + client)** — wire the enhanced WS conn/task into the Decision 11 seam, perform the upgrade, carry Connect envelopes as binary WS messages; full-duplex bidi end-to-end | 13-E1, 13-E3 |

*(Implementation order: 13-E1 → 13-E3 → 13-F1, with 13-E2 layered in once the reactor is wired
(`RawStream: AsRawFd`, Decision 12 §12). This whole decision is still scheduled last among
transports.)*
