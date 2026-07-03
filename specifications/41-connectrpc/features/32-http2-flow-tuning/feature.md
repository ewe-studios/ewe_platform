---
feature: "HTTP/2 flow-control tuning (D12 §5 phase 3)"
description: "Window sizing/update strategy informed by profiling; the declared implementation-time tunable"
status: "pending"
priority: "low"
phase: 2
depends_on: ["30-http2-multiplexers"]
estimated_effort: "small"
created: 2026-07-03
---
# Feature 32-http2-flow-tuning: HTTP/2 flow-control tuning (D12 §5 phase 3)

## Description

The planned third phase of the http2 module: measure, then tune stream/connection windows.

## Normative sources (single source of truth — read before writing code)

- decisions/12-foundation-enablement.md — §5 (phase 3); plan.md — Implementation-Time Tunables

## Scope

- Window update strategy + defaults under load; benchmarks for streaming throughput vs memory

## Acceptance criteria

- Documented tuning rationale + benchmark results; no behavioural regressions in conformance
