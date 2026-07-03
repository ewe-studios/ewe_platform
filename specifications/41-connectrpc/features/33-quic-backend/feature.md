---
feature: "QUIC backend: quinn-proto + valtron driver + netcap variants (D01 T14, D12 §9)"
description: "Sans-IO quinn-proto as a Cargo dep, our UDP plumbing + event-loop task, netcap Quic variants"
status: "pending"
priority: "medium"
phase: 3
depends_on: ["10-reactor-parking"]
estimated_effort: "large"
created: 2026-07-03
---
# Feature 33-quic-backend: QUIC backend: quinn-proto + valtron driver + netcap variants (D01 T14, D12 §9)

## Description

Phase-3 groundwork: QUIC via the sans-IO quinn-proto state machine driven by valtron tasks, exposed through our progress-returning trait set and netcap's centralized listener.

## Normative sources (single source of truth — read before writing code)

- decisions/01-transport-and-runtime.md — T14 (quinn-proto decision), §QUIC trait abstraction (normative traits)
- decisions/12-foundation-enablement.md — §9 (netcap Connection/Listener/ConfigListenAddr Quic variants)

## Scope

- quinn-proto (feature-gated Cargo dep, no vendoring) + valtron-driven event loop + UDP socket plumbing in netcap (optional quinn-udp GSO/GRO)
- Our QuicConnection/QuicSendStream/QuicRecvStream/QuicBidiStream traits (progress-returning, TaskStatus-driven, no Poll/Waker) implemented over quinn-proto
- netcap Listener/Connection Quic variants; HttpServer accepts via netcap Listener (D12 §9 items 1–2)

## Out of scope

- iroh (deferred — D01: do not build (b) speculatively)

## Acceptance criteria

- QUIC connect/accept + bidi stream echo over the trait set, parking on the reactor (no busy-poll)
