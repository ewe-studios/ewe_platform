---
feature: "Pipe<T>/FramePipe + vacancy readiness + AnyReadiness (00-F4)"
description: "Two-sided waker-hooked bounded pipe; producer parking on full pipes; composite readiness"
status: "pending"
priority: "high"
phase: 1
depends_on: ["01-waker-queue-bridge"]
estimated_effort: "medium"
created: 2026-07-03
---
# Feature 02-pipe-primitive: Pipe<T>/FramePipe + vacancy readiness + AnyReadiness (00-F4)

## Description

The generic bounded pipe primitive Pipe<T> (FramePipe = Pipe<Frame>): bounded ConcurrentQueue + two waker stashes (push wakes stashed consumer, pop wakes stashed producer) + QueueReadiness/QueueVacancyReadiness accessors + the AnyReadiness combinator. Async ends await; valtron-task ends return Depends. Bare Pending is never the answer to pipe-full/pipe-empty; awaits/parks compose the call's CancelSignal.

## Normative sources (single source of truth — read before writing code)

- decisions/00-valtron-async-readiness.md — Level 1b (normative)
- decisions/11-transport-seam.md — §Core primitive (Pipe<T>, layering)

## Scope

- QueueVacancyReadiness (ready = has capacity); AnyReadiness (one Depends, composite logic inside)
- Pipe<T> with PipeSender<T>/PipeReceiver<T> halves; waker stashes fired by the opposite end's push/pop
- Depth default 4, configurable; awaits/parks compose an external signal (cancellation)
- Replaces ConcurrentQueueStreamIterator's polling handoff wherever the seam uses a pipe

## Out of scope

- The Frame enum itself (feature 17 — a connectrpc crate type)

## Acceptance criteria

- Producer awaiting a full pipe parks and is woken by the consumer's pop (turn count flat while full)
- Consumer/empty case symmetric; AnyReadiness fires when any child is ready
- No max_turns/park_duration spinning in any pipe path
