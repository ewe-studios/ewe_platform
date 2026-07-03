---
feature: "WebSocketTransport: Connect-over-WS bidi (D13 F1)"
description: "The opt-in WS transport: upgrade routing, subprotocol negotiation, batch framing, full-duplex bidi on h1 deployments"
status: "pending"
priority: "medium"
phase: 4
depends_on: ["37-ws-server-task", "22-router-dispatch"]
estimated_effort: "large"
created: 2026-07-03
---
# Feature 39-ws-transport: WebSocketTransport: Connect-over-WS bidi (D13 F1)

## Description

The last transport: Connect envelopes over WebSocket messages, giving bidi to HTTP/1.1 deployments — non-standard, opt-in, our-stack-to-our-stack, validating the seam's thesis.

## Normative sources (single source of truth — read before writing code)

- decisions/13-websocket-transport.md — §Wire mapping + Decided Details OQ#13.2/13.3 (normative rules)

## Scope

- Upgrade path routing + ewe.connectrpc(.batch).v1 subprotocol negotiation (exact refusal rules); call metadata via Connect GET query params
- Batch framing (BE u32 count; opportunistic, never timed; size caps normative) + 1:1 interop mode
- Server + client Transport impls over the enhanced tasks; one RPC per connection; Close ↔ cancellation mapping
- Capabilities: request_streaming + full_duplex true, h2_trailers false

## Acceptance criteria

- Full-duplex bidi end-to-end on an HTTP/1.1-only deployment (our client ↔ our server)
- Negotiation refusal rules verified (unknown subprotocol → 400; missing echo → client protocol error)
