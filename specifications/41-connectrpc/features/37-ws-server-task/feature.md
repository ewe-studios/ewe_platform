---
feature: "WebSocketServerTask + WsServerConfig + recv retrofit (D13 E3, OQ#13.4)"
description: "Progress-driven server task: assembler, dual-queue seam, config-gated auto-Pong/Close; assembler retrofit for blocking recv"
status: "complete"
priority: "medium"
phase: 4
depends_on: ["36-ws-resumable-decoder"]
estimated_effort: "medium"
created: 2026-07-03
---
# Feature 37-ws-server-task: WebSocketServerTask + WsServerConfig + recv retrofit (D13 E3, OQ#13.4)

## Description

The robust server-side WS task mirroring the existing client task, plus the correctness retrofit for the blocking convenience API.

## Normative sources (single source of truth — read before writing code)

- decisions/13-websocket-transport.md — E3 + Decided Details OQ#13.4 (normative)

## Scope

- WebSocketServerTask (server semantics: no masking out, reject unmasked in) with MessageAssembler + inbound/outbound queue seam
- WsServerConfig { auto_pong, max_message_size, graceful_close, read_model, queues }
- Retrofit blocking WebSocketServerConnection::recv with the assembler (fragmented messages; interleaved control frames)

## Acceptance criteria

- Multi-frame messages assemble on the server; Ping auto-answered per config; Close handshake completes per config
