---
feature: "Runtime backend selection + functional probe (D14 F3)"
description: "NativeAPI::IOUring actually builds io_uring; Auto walks the ladder; explicit request fails hard on probe failure"
status: "complete"
priority: "medium"
phase: 2
depends_on: ["41-uring-selector"]
estimated_effort: "small"
created: 2026-07-03
---
# Feature 42-uring-selection-probe: Runtime backend selection + functional probe (D14 F3)

## Description

Fix the current lie (IOUring advertised, UnsupportedPlatform returned): probe-gated selection with the no-silent-defaults rule for explicit backend requests.

## Normative sources (single source of truth — read before writing code)

- decisions/14-io-uring-reactor-backend.md — Scope #2 + Decided Details OQ#14.3 (normative probe + semantics)

## Scope

- Functional opcode probe (setup → REGISTER_PROBE → tier checks) — never version-string gating
- Auto: completion → readiness → epoll, surfaced; explicit IOUring: hard error with probe detail (no-silent-defaults)
- reactor.backend() observability + init tracing

## Acceptance criteria

- Explicit IOUring on an unsupported host errors with the probe reason; Auto on old kernels lands on epoll and says so
