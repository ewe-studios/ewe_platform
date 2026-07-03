---
feature: "HTTP/2 server + client multiplexers, ALPN/h2c×3 (D12 §5 phase 2)"
description: "Both multiplexers in one phase; all three h2 entry paths; ConnectionHandler branch; per-stream dispatch to the Router"
status: "pending"
priority: "high"
phase: 2
depends_on: ["29-http2-substrate", "05-http11-part-iterators", "22-router-dispatch"]
estimated_effort: "large"
created: 2026-07-03
---
# Feature 30-http2-multiplexers: HTTP/2 server + client multiplexers, ALPN/h2c×3 (D12 §5 phase 2)

## Description

The HTTP/2 connection layer: both multiplexers landed together, dispatching multiplexed streams to the same Router with all three entry paths and real trailing HEADERS.

## Normative sources (single source of truth — read before writing code)

- decisions/12-foundation-enablement.md — Decided Details #3 (one-phase decision) and #4 (all three entry paths, normative)

## Scope

- Server + client multiplexers over the substrate; each h2 stream dispatches independently to the shared Router
- Entry paths: TLS-ALPN (rustls set_protocols), h2c prior-knowledge (PRI preface peek ≤24B), Upgrade: h2c (101 + request replay as stream 1)
- Pseudo-header mapping inside the module (T8); RST_STREAM → CancelSignal; PUSH_PROMISE rejected (T6)
- HTTPStreams factory branch (B8); Transport impl reporting h2 capabilities (full_duplex, h2_trailers, multiplexed)

## Out of scope

- Flow-control tuning (feature 32)

## Acceptance criteria

- Client multiplexer conformance-tests against our own server AND grpcurl/connect-go peers
- All three entry paths serve the same Router; cert-free h2c covers the full test matrix
- Full-duplex bidi works end-to-end (capability matching now admits it)
