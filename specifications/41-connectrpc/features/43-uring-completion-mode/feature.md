---
feature: "io_uring completion-mode read path (D14 F4)"
description: "CompletionSource seam + buffer rings: kernel-filled reads, zero read() syscalls on the hot path"
status: "pending"
priority: "low"
phase: 3
depends_on: ["41-uring-selector", "42-uring-selection-probe"]
estimated_effort: "large"
created: 2026-07-03
---
# Feature 43-uring-completion-mode: io_uring completion-mode read path (D14 F4)

## Description

The committed second io_uring mode: completion-based reads with kernel-filled buffer rings, sequenced strictly after the readiness parity suite is green.

## Normative sources (single source of truth — read before writing code)

- decisions/14-io-uring-reactor-backend.md — Decided Details OQ#14.2 (F4 scope; starts only after F2 parity is green)

## Scope

- CompletionSource seam next to EventReadiness (take_completions + buffer recycle protocol)
- RECV/SEND with registered buffer rings (kernel ≥5.19/6.0 probes; fallback to readiness mode)
- Transports opt read paths into inbox-pop; decoder unchanged (step over &buf[..]); per-worker-ring re-evaluation (the OQ#14.1 trigger)

## Acceptance criteria

- Hot-path reads show zero read() syscalls under strace on a supporting kernel; sub-matrix kernels fall back cleanly
