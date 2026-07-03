---
feature: "WebSocket resumable frame decoder (D13 E1)"
description: "WebSocketFrameDecoder as the WS impl of IncrementalDecoder; partial-frame state across polls"
status: "pending"
priority: "medium"
phase: 4
depends_on: ["04-incremental-decoder"]
estimated_effort: "medium"
created: 2026-07-03
---
# Feature 36-ws-resumable-decoder: WebSocket resumable frame decoder (D13 E1)

## Description

Replaces the one-shot decode assumption: stateful partial-frame decoding so WS can ride non-blocking fds and the reactor path.

## Normative sources (single source of truth — read before writing code)

- decisions/13-websocket-transport.md — E1 (normative); decisions/12-foundation-enablement.md — §11

## Scope

- WebSocketFrameDecoder::step (Pending on short reads, never 'stream corrupted' mid-frame); size limits + control-frame rules preserved
- Blocking decode becomes a step-loop wrapper (backward compatible)

## Acceptance criteria

- A frame split across N reads decodes; existing WS tests pass unchanged
