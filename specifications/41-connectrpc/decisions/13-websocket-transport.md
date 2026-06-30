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

We already own the WebSocket layer in `foundation_netio/src/websocket/`:

- `shared::frame` — `WebSocketFrame`, `Opcode` (RFC 6455 framing + masking)
- `shared::message` — `WebSocketMessage` (Text/Binary/Ping/Pong/Close)
- `shared::handshake` — `compute_accept_key`, `generate_websocket_key`,
  `build_upgrade_request`, `validate_upgrade_response`
- `shared::assembler` — fragmented-frame reassembly
- `native::connection` — `WebSocketConnection` (`send`/`recv`/`messages()`/`close`/`flush`),
  `MessageDelivery` (whose `queue() -> &Arc<ConcurrentQueue<WebSocketMessage>>` **is already an
  outbound seam queue**), `WebSocketClient`
- `native::server` — server-side upgrade/accept

So this decision is mostly *wiring* an existing carrier into the RPC stack, not building a
WebSocket implementation from scratch.

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
- **Direction.** Inbound RPC messages come from `WebSocketConnection::recv()` /
  `messages()`; outbound go through `MessageDelivery` / its `ConcurrentQueue`. This is the
  Decision 11 `MessageSource` / `MessageSink` pair with a WebSocket-shaped backing — no new
  seam concept.
- **Trailers / end-of-stream.** Connect streaming end-of-stream is an `EndStreamResponse`
  envelope (Decision 05), carried as a final binary message; the WebSocket `Close` frame is
  the transport-level teardown, mapped to/from cancellation (Decision 04, Option B).
- **Control frames.** `Ping`/`Pong` are handled by the WebSocket layer for keepalive and are
  **not** surfaced to handlers. `Close` (with code/reason) maps to stream completion or
  cancellation.

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
- **Sequencing:** implemented **last** among transports; nothing in Phases 1–3 depends on it.

## Open Questions

- **OQ#13.1** — one envelope per WS binary message vs. batched envelopes per message.
  *Tentative:* one-per-message.
- **OQ#13.2** — whether to expose a WebSocket transport for **unary/server-stream** too (for
  uniformity on HTTP/1.1) or restrict it to bidi/client-stream where it actually adds
  capability. *Tentative:* allow all four kinds over WS for uniformity, but bidi is the
  motivating case.
- **OQ#13.3** — subprotocol negotiation: advertise a `Sec-WebSocket-Protocol` token (e.g.
  `connect-rpc`) so peers can detect compatibility during the handshake.

## Features

- **13-F1 — `WebSocketTransport` (server + client).** Wire the `foundation_netio` WebSocket
  connection into the Decision 11 seam (`MessageSource`/`MessageSink`), perform the upgrade,
  and carry Connect envelopes as binary WS messages. Full-duplex bidi end-to-end. *(Last
  feature in the implementation order.)*
