---
feature: "#[valtron]/#[valtron_test] accept async fn (00-F3)"
description: "Async entrypoint/test macro support via block_on_future; sync path untouched"
status: "pending"
priority: "high"
phase: 1
depends_on: ["01-waker-queue-bridge"]
estimated_effort: "small"
created: 2026-07-03
---
# Feature 03-async-valtron-macros: #[valtron]/#[valtron_test] accept async fn (00-F3)

## Description

#[valtron] / #[valtron_test] accept async fn bodies by wrapping them in block_on_future (from_future + run-to-completion). Enables plain-async handlers and conformance tests. Supersedes spec-50 feature 03-async-valtron-macros.

## Normative sources (single source of truth — read before writing code)

- decisions/00-valtron-async-readiness.md — Level 3 (normative expansion sketch)
- backends/foundation_macros/src/valtron_entry.rs

## Scope

- block_on_future<F> in foundation_core (single/wasm cfg variant without Send)
- Macro branches on sig.asyncness; ?/return semantics preserved; sync expansion byte-for-byte unchanged

## Acceptance criteria

- #[valtron_test] async fn compiles and runs on single and multi; sync forms unchanged
