---
feature: "Shared reactor + Token→wake map (D14 F1)"
description: "One process-level reactor instead of per-fd epoll instances; is_ready reads drained readiness"
status: "complete"
priority: "medium"
phase: 2
depends_on: ["10-reactor-parking"]
estimated_effort: "medium"
created: 2026-07-03
---
# Feature 40-shared-reactor: Shared reactor + Token→wake map (D14 F1)

## Description

The bigger near-term win of Decision 14: kill the per-fd epoll instance and per-check syscall for ALL native fd parking, on every Unix platform, backend-agnostic.

## Normative sources (single source of truth — read before writing code)

- decisions/14-io-uring-reactor-backend.md — Scope #3 + Decided Details OQ#14.1 (normative)

## Scope

- OnceLock singleton reactor (selector + concurrent Token→wake map); RegisteredFd/FdRegistration register into it
- is_ready consults drained readiness — zero syscalls on the check path; drain strategy is an implementation detail

## Acceptance criteria

- N idle connections: no private epoll fd each, no per-check syscall (fd/syscall count test — D14 success criteria)
