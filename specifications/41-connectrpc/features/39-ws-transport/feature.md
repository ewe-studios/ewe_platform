---
feature: "WebSocketTransport: Connect-over-WS bidi via ReconnectingWebSocketTask (D13 F1)"
description: "The opt-in WS transport built on ReconnectingWebSocketTask + Pipe; full-duplex bidi on h1 deployments with automatic reconnection"
status: "in-progress"
priority: "medium"
phase: 4
depends_on: ["37-ws-server-task", "22-router-dispatch", "02-pipe-primitive"]
estimated_effort: "large"
created: 2026-07-03
updated: 2026-07-10
---
# Feature 39: WebSocketTransport — Connect-over-WS bidi (D13 F1)

## Description

The last transport: Connect envelopes over WebSocket messages, giving bidi to
HTTP/1.1 deployments — non-standard, opt-in, our-stack-to-our-stack, validating
the seam's thesis.

## Design revision: built on ReconnectingWebSocketTask + Pipe (2026-07-10)

### Before (as initially committed)

`WsBytePump` in `pump.rs` duplicated the WS handshake and frame I/O from
scratch using raw TCP — bypassing the proven `WebSocketTask` and
`ReconnectingWebSocketTask` that already handle connect, upgrade, Ping→Pong,
Close handshake, message assembly, and reconnection with exponential backoff.

The reason was `WebSocketTask<R: DnsResolver>` being generic — but
`SystemDnsResolver` is a concrete type; `ReconnectingWebSocketTask<SystemDnsResolver>`
is fully monomorphized and can be spawned via `valtron::send(task)` without
any boxing.

### After (target)

`WsTransport::open()` spawns `ReconnectingWebSocketTask<SystemDnsResolver>` on
the valtron pool, bridges its Pipe-based message seam into the standard
`TransportStream { send_body, head, recv_body, trailers }`:

```
                      ┌─────────────────────────────────┐
                      │     ReconnectingWebSocketTask     │
                      │                                  │
 caller's tx ────────►│ PipeReceiver<WebSocketMessage>    │──► TCP (WS Binary frames)
 (PipeSender)   send  │  (outbound: caller → task)       │
                      │                                  │
 caller's rx ◄────────│ PipeSender<WebSocketMessage>      │◄── TCP (WS frames → decode → message)
 (PipeReceiver) recv  │  (inbound: task → caller)        │
                      │                                  │
                      │  Auto-reconnect on disconnect.   │
                      │  Pipe halves are swapped atomically│
                      │  on each reconnect cycle.         │
                      └─────────────────────────────────┘
```

The task's inbound `Ready(WebSocketMessage)` values are bridged to `body_tx`
by a thin collector adapter. The 101 head is emitted once on handshake
completion (the task surfaces `ConnectionEstablished`).

Reconnection status is already surfaced via `type Pending =
ReconnectingWebSocketProgress` — `Connecting` / `Handshaking` / `Reading` /
`Reconnecting`. The transport can expose this if desired, but the default is
transparent: the caller just sees a `TransportStream`.

### Why Pipe instead of ConcurrentQueue

See [F37 design revision](../37-ws-server-task/feature.md#design-revision-concurrentqueue--pipe) —
the same argument applies. On an idle connection the task parks on
`Depends(pipe.readiness())` — zero polls, zero syscalls. On the client side,
the task's outbound `PipeReceiver` is empty → parks on `QueueReadiness`; the
caller's `PipeSender.send()` wakes it. On the server side, the inbound
`PipeSender` is full → parks on `QueueVacancyReadiness`; the caller's
`PipeReceiver.receive()` wakes it.

### Reconnection model

`ReconnectingWebSocketTask` handles disconnect detection, exponential backoff,
and re-handshake. On each successful reconnect:

1. Old pipe halves are closed (drop → the old receiver sees `None`).
2. New `Pipe<WebSocketMessage>` pair is created.
3. The task swaps in the new `PipeReceiver` (outbound) and `PipeSender` (inbound).
4. The caller's ends are atomically replaced: the old `PipeSender`/`PipeReceiver`
   are closed, and the new halves are returned to the caller via a notification
   channel (or the caller re-reads them from a shared `Arc<Mutex<...>>`).

The caller sees `send()` → `Err(Closed)` during the reconnect window, retries,
and the new `PipeSender` is available once the task completes the handshake.

## Scope

- [ ] Delete `pump.rs` (raw TCP `WsBytePump`) — replaced by `ReconnectingWebSocketTask`
- [ ] Delete `ws.rs` in connectrpc (current `WsTransport` wrapping `WsBytePump`)
- [ ] Migrate `WebSocketTask` from `Arc<ConcurrentQueue>` to `Pipe` (F37)
- [ ] Migrate `ReconnectingWebSocketTask` to Pipe with reconnect-safe caller ends
- [ ] `WsTransport::open()`: spawn `ReconnectingWebSocketTask<SystemDnsResolver>`,
      bridge Pipe ends into `TransportStream`
- [ ] Inbound collector: bridge `TaskStatus::Ready(WebSocketMessage::Binary)` →
      `body_tx` pipe; `WebSocketMessage::Close` → `trailer_tx` close
- [ ] Head: emit `Status::SwitchingProtocols` + response headers on `ConnectionEstablished`
- [ ] Upgrade path routing + `ewe.connectrpc(.batch).v1` subprotocol negotiation
- [ ] Batch framing (BE u32 count; opportunistic, never timed; size caps normative) + 1:1 interop mode
- [ ] Capabilities: `request_streaming + full_duplex: true, h2_trailers: false`

## Acceptance criteria

- [ ] Full-duplex bidi end-to-end on an HTTP/1.1-only deployment (our client ↔ our server)
- [ ] Negotiation refusal rules verified (unknown subprotocol → 400; missing echo → client protocol error)
- [ ] `WsTransport::open()` returns in < 1 poll cycle (handshake runs on pool, not inline)
- [ ] Disconnect + reconnect: caller's `PipeSender` is replaced atomically; messages sent during the reconnect window are not lost (buffered in the pipe or rejected with `Closed` — caller retries)
- [ ] Idle connection: task parks on `Depends`, zero polls
