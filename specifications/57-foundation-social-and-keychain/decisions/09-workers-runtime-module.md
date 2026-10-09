# Decision 09: Workers WebSocket — Reuse foundation_netio Framing

**Status:** Resolved (2026-07-18)

## Problem

SignalR notifications need WebSocket + MessagePack binary framing. foundation_netio (F51-complete) already has:
- Frame encoding/decoding (`shared/frame.rs`, `shared/decoder.rs`, `shared/assembler.rs`)
- Batch writer (`shared/batch_writer.rs`)
- Client/connector (`shared/client.rs`, `shared/connector.rs`)
- Native server (`native/server.rs`)

Workers uses `WebSocketPair` for accept and binary/text send/recv — different transport, same framing.

## Analysis

Reimplementing the framing logic would duplicate ~10 files of tested, working code. The only Workers-specific part is:
- Accepting a `WebSocketPair` (vs. TCP upgrade on native)
- Sending/receiving via `ws.send_with_bytes()` / `ws.send_with_str()` (vs. `TcpStream` on native)

The SignalR MessagePack framing (VarInt length prefix + MessagePack payload) is identical — it's in `core/notifications/mod.rs` (portable) and uses `rmpv` for encoding.

## Decision: Reuse foundation_netio, Workers transport only

`foundation_deployment_cloudflare::workers::websocket` only implements:
- `WebSocketPair` accept (Workers-specific)
- Binary/text send/recv bridge (Workers-specific)
- Handshake parsing (Workers-specific)

All framing logic comes from `foundation_netio::websocket::shared`. The SignalR MessagePack framing comes from `foundation_keychain::core::notifications` (portable).

## Consequences

- No code duplication — framing tested once in foundation_netio
- Workers module is thin: ~50 lines of transport glue
- If foundation_netio's framing gets a bug fix, Workers automatically benefits
