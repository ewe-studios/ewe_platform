---
feature: "AsRawFd on netcap RawStream/Connection (D12 §12)"
description: "Reactor prerequisite: the raw fd reachable from above netio for RegisteredFd parking"
status: "complete"
priority: "high"
phase: 1
depends_on: []
estimated_effort: "small"
created: 2026-07-03
---
# Feature 09-raw-stream-as-raw-fd: AsRawFd on netcap RawStream/Connection (D12 §12)

## Description

Expose AsRawFd/AsFd on netcap RawStream/Connection — the single missing seam between netio streams and the foundation_nativeapis reactor. Additive; no behavior change.

## Normative sources (single source of truth — read before writing code)

- decisions/12-foundation-enablement.md — §12 (normative)

## Scope

- AsRawFd + AsFd impls delegating to the inner socket; TLS variants → the underlying TCP fd; native-socket only (no wasm impl)

## Acceptance criteria

- A live RawStream yields its fd and a RegisteredFd builds from it (integration test)
