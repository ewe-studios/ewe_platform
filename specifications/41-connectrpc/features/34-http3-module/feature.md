---
feature: "HTTP/3 module: framing + QPACK over the QUIC traits (D01 roadmap)"
description: "h3-design replicated tokio-free: frame codec, QPACK, connection/stream mapping over QuicConnection"
status: "pending"
priority: "medium"
phase: 3
depends_on: ["33-quic-backend", "29-http2-substrate"]
estimated_effort: "large"
created: 2026-07-03
---
# Feature 34-http3-module: HTTP/3 module: framing + QPACK over the QUIC traits (D01 roadmap)

## Description

The third transport: HTTP/3 framing replicated from h3's design over our valtron-native QUIC traits — same Router, same handler types.

## Normative sources (single source of truth — read before writing code)

- decisions/01-transport-and-runtime.md — §Future-Phase Transport Roadmap (HTTP/3) — normative deviations list

## Scope

- http3/ module: frame codec (IncrementalDecoder), QPACK, request/response mapping to Simple types; quic.rs backend-agnostic trait boundary
- Alt-Svc advertisement (T2); ConnectionContext population at connection setup

## Acceptance criteria

- HTTP/3 request/response round-trip to the shared Router; produces/consumes the universal Simple types
