---
feature: "Native fd parking via the nativeapis reactor (00-F2)"
description: "Sockets park on Depends(RegisteredFd) instead of cooperative re-poll; epoll today, io_uring later"
status: "complete"
priority: "high"
phase: 1
depends_on: ["01-waker-queue-bridge", "09-raw-stream-as-raw-fd"]
estimated_effort: "medium"
created: 2026-07-03
---
# Feature 10-reactor-parking: Native fd parking via the nativeapis reactor (00-F2)

## Description

Wire native real-I/O parking through the EXISTING foundation_nativeapis reactor: a task holds Arc<RegisteredFd> (which is EventReadiness) and returns Depends. Prove with an in-tree test readiness; document how tasks obtain the reactor Registry.

## Normative sources (single source of truth — read before writing code)

- decisions/00-valtron-async-readiness.md — Level 2 (the superseded-sketch note IS the decision), Consequences
- backends/foundation_nativeapis/src/native/fd/mod.rs (RegisteredFd/FdRegistration)

## Scope

- Registry-access pattern for transport tasks; Depends(Arc<RegisteredFd>) end-to-end
- In-tree test EventReadiness driving a future to completion via Depends
- NO new ReadinessSource trait / OnceLock slot (explicitly superseded)

## Out of scope

- io_uring backend + shared-reactor refactor (features 40–43)

## Acceptance criteria

- Native fd parking works with no new foundation_core dependency and no new abstraction (Decision 00 success criteria)
