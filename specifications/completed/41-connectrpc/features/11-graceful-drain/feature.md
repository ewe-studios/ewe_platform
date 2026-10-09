---
feature: "Graceful shutdown / connection draining (D12 §10)"
description: "Stop accepting, let in-flight RPCs finish within a grace window, then force-close"
status: "complete"
priority: "medium"
phase: 1
depends_on: []
estimated_effort: "medium"
created: 2026-07-03
---
# Feature 11-graceful-drain: Graceful shutdown / connection draining (D12 §10)

## Description

foundation_http graceful shutdown per Decision 12 §10 — cooperative, task-decided drain with a bounded grace window; what ConnectRPC needs for clean streaming shutdown.

## Normative sources (single source of truth — read before writing code)

- decisions/12-foundation-enablement.md — §10 (normative three pieces)

## Scope

- Active-connection WaitGroup around ConnectionHandler task lifetimes
- Arc<OnSignal> cloned into every handler task — task-decided drain policy at checkpoints
- Bounded drain phase (ServerConfig::shutdown_grace) after the accept loop breaks; force-close past deadline

## Acceptance criteria

- Keep-alive conns stop taking new requests post-shutdown; in-flight streaming completes within grace; force-close past deadline verified
