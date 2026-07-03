---
feature: "Pushable client request body over Pipe<Bytes> (D12 §7)"
description: "Client streaming request bodies via chunked TE with waker-hooked pipe backing (B6)"
status: "pending"
priority: "high"
phase: 1
depends_on: ["02-pipe-primitive"]
estimated_effort: "small"
created: 2026-07-03
---
# Feature 07-pushable-request-body: Pushable client request body over Pipe<Bytes> (D12 §7)

## Description

Back SendSafeBody::Stream with the Pipe<Bytes> primitive so the client can push request bytes after the request has started sending (io.Pipe parity, no whole-request buffering).

## Normative sources (single source of truth — read before writing code)

- decisions/12-foundation-enablement.md — §7 (normative; Pipe<Bytes>, NOT ConcurrentQueueStreamIterator)

## Scope

- Pushable backing for the existing SendSafeBody::Stream variant; no new body type
- Producer half held by the client-stream/bidi task; consumer drained by the request renderer

## Acceptance criteria

- Request bytes pushed after send-start reach the wire without full-request buffering; no polling handoff
