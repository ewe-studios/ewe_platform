---
feature: "WebSocketTransport: Connect-over-WS bidi via WebSocketClient + MessageDelivery (D13 F1)"
description: "WsTransport built on the existing WebSocketClient + MessageDelivery (Pipe-backed); full-duplex bidi on h1 with reconnection"
status: "in-progress"
priority: "medium"
phase: 4
depends_on: ["37-ws-server-task", "22-router-dispatch", "02-pipe-primitive"]
estimated_effort: "medium"
created: 2026-07-03
updated: 2026-07-10
---
# Feature 39: WebSocketTransport — Connect-over-WS bidi (D13 F1)

## Description

The last transport: Connect envelopes over WebSocket messages, giving bidi to
HTTP/1.1 deployments — non-standard, opt-in, our-stack-to-our-stack.

## Design revision: WebSocketClient + MessageDelivery (2026-07-10)

### The existing `WebSocketClient` already does everything

`WebSocketClient` in `connection.rs` already handles:

- TCP connect + WS upgrade handshake + 101 validation
- `WebSocketTask` spawning on valtron via `execute(task, None)`
- Frame encode/decode, masking, Ping→Pong, Close handshake
- Returns `(Self, MessageDelivery)` — caller polls `client.next()` for
  inbound messages, calls `delivery.send(msg)` for outbound

After the F37 Pipe migration, `MessageDelivery` wraps a `PipeSender` instead
of `Arc<ConcurrentQueue>`. The API is unchanged — `send(msg)`, `ping()`,
`pong()`, `close()` — but now backed by `Pipe` with wake + backpressure.

### `WsTransport::open()` — ~30 lines of glue

```
open(request):
  1. let (client, delivery) = WebSocketClient::connect(
       SystemDnsResolver::default(),
       ws_url,
       read_timeout,
       sleep_timeout,
     )?;

  2. Spawn collector on valtron: loop { client.next() }
       Stream::Next(Ok(Binary(data))) → body_tx.send(data).await
       Stream::Next(Ok(Close(..))) | exhausted → body_tx.close(), break

  3. Emit head: Status::SwitchingProtocols on ConnectionEstablished

  4. Return TransportStream {
       send_body: delivery.into_pipe(),    // PipeSender half
       head,                                // 101 + response headers
       recv_body: body_rx,                  // PipeReceiver half
       trailers: trailer_rx,
     }
```

**No custom task. No raw TCP. No handshake code.** The existing
`WebSocketClient` + `MessageDelivery` carry the entire client side.

### Generic resolved by concrete type

`WebSocketClient<SystemDnsResolver>` is fully monomorphized — spawns via
`valtron::send()` without boxing. The generic isn't a problem; it's the
same pattern `SimpleHttpClient<SystemDnsResolver>` already uses.

### Reconnection

`WebSocketClient` wraps the single-shot `WebSocketTask`. For reconnection,
`WsTransport` can optionally layer `ReconnectingWebSocketTask` (which wraps
`WebSocketTask` with exponential backoff). Both paths are valid:

| Strategy | Task | Reconnect | Use case |
|---|---|---|---|
| Simple | `WebSocketTask` via `WebSocketClient` | None — drop + reconnect at Transport level | Ephemeral calls, test fixtures |
| Reconnecting | `ReconnectingWebSocketTask` | Auto, exponential backoff | Long-lived connections |

The initial implementation uses the simple path. Reconnecting is a follow-on
config option (`WsTransport::with_reconnect(true)`).

### How it connects to the Pipe design (F37)

`WebSocketClient::connect()` returns `(client, MessageDelivery)`. After F37:
- `client` drains `DrivenStreamIterator<WebSocketTask>` — inbound messages
- `delivery` wraps `PipeSender` — outbound messages
- `delivery.pipe()` → `&PipeSender` — caller can bridge into `TransportStream::send_body`
- `delivery.send(msg)` → `tx.try_send(msg)` — same API as today
- `delivery.send_async(msg).await` → `tx.send(msg).await` — parks on backpressure

## Scope

- [ ] Delete `pump.rs` (raw TCP `WsBytePump`) — replaced by `WebSocketClient`
- [ ] Delete `ws.rs` in connectrpc (current `WsTransport` wrapping `WsBytePump`)
- [ ] Migrate `WebSocketTask` from `Arc<ConcurrentQueue>` to `Pipe`, update `WebSocketClient` accordingly (F37)
- [ ] Migrate `MessageDelivery`: replace `Arc<ConcurrentQueue>` with `PipeSender` (F37)
- [ ] `WsTransport::open()`: `WebSocketClient::connect()` → spawn collector → `TransportStream`
- [ ] Inbound collector: bridge `Stream::Next(Ok(WebSocketMessage::Binary))` → `body_tx`
- [ ] Head: emit `Status::SwitchingProtocols` on `ConnectionEstablished`
- [ ] Upgrade path routing + `ewe.connectrpc(.batch).v1` subprotocol negotiation
- [ ] Batch framing (BE u32 count; opportunistic, never timed; size caps normative) + 1:1 interop mode
- [ ] Capabilities: `request_streaming + full_duplex: true, h2_trailers: false`
- [ ] Optional: `ReconnectingWebSocketTask` behind `with_reconnect(true)` config

## Acceptance criteria

- Full-duplex bidi end-to-end on an HTTP/1.1-only deployment (our client ↔ our server)
- `WsTransport::open()` returns in < 1 poll cycle (handshake runs on pool, not inline)
- Idle connection: task parks on `Depends(pipe.readiness())`, zero polls
- `delivery.send_async(msg).await` parks caller when pipe is full, wakes when consumer drains
- Negotiation refusal rules verified (unknown subprotocol → 400; missing echo → client protocol error)
- `WebSocketClient::connect()` return signature unchanged — `(Self, MessageDelivery)`
