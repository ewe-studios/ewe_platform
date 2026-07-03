---
feature: "WS Depends read model via the reactor (D13 E2)"
description: "Client + server WS tasks park on socket readiness (RegisteredFd) with timeout-poll fallback"
status: "pending"
priority: "low"
phase: 4
depends_on: ["36-ws-resumable-decoder", "10-reactor-parking"]
estimated_effort: "small"
created: 2026-07-03
---
# Feature 38-ws-depends-read-model: WS Depends read model via the reactor (D13 E2)

## Description

True parking for WebSocket reads on native — the same task code over epoll today and io_uring when Decision 14 lands.

## Normative sources (single source of truth — read before writing code)

- decisions/13-websocket-transport.md — E2 (normative; gating + fallback rules)

## Scope

- ReadModel::Depends when a reactor is reachable; fall back to timeout-poll + Delayed otherwise (no regression)

## Acceptance criteria

- Idle WS connection holds no worker (turn count flat) when the reactor is present; identical behavior via fallback without it
