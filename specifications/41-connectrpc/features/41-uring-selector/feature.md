---
feature: "io_uring readiness selector (D14 F2)"
description: "Multishot-poll io_uring backend behind the existing Selector interface, with the uring↔epoll parity suite"
status: "pending"
priority: "medium"
phase: 2
depends_on: ["40-shared-reactor"]
estimated_effort: "large"
created: 2026-07-03
---
# Feature 41-uring-selector: io_uring readiness selector (D14 F2)

## Description

The real io_uring selector — readiness mode first, slotted behind the existing Poll/Registry API so no caller changes, gated by the parity suite.

## Normative sources (single source of truth — read before writing code)

- decisions/14-io-uring-reactor-backend.md — Scope #1 + Decided Details OQ#14.2 (normative)

## Scope

- poll/sys/unix/selector/uring.rs (IORING_OP_POLL_ADD multishot) behind cfg(linux, feature=uring)
- uring↔epoll behavioral parity test suite (same events for same stimuli, edge triggers, close-mid-poll, drain order)

## Out of scope

- Completion mode (feature 43)

## Acceptance criteria

- Parity suite green on a ≥5.13 kernel; identical task code across backends (D14 success criteria)
