---
feature: "Lane B crates use valtron's self-waking constructs (event queue, ResumeSignal) instead of raw queues with polling readiness"
status: "pending"
lane: "B"
model: "sonnet"
branch: "fix/io-51-60-lane-b"
crates: ["foundation_toolings", "foundation_ai", "foundation_wasm", "foundation_wasm_ui"]
catalogue_rows: [107]
---

# Feature 81: lane B crates on valtron's wake constructs

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

Depends on feature 80 (lane A), which adds the event queue and makes the executor wake private
to valtron (owner rule 2026-11-04: executor wake does not leak out of valtron).

## Sites (audit 2026-11-04)

Raw queues with `QueueReadiness::new` / `QueueVacancyReadiness::new` or `BoolSignal` raw access:
`foundation_toolings` `src/builder/mod.rs` (2), `src/watcher/mod.rs` (1), `src/runner/mod.rs`
(1); `foundation_ai` `src/agentic/steering.rs` (2); `foundation_wasm` `src/base.rs` (3) and
`integration/module/src/lib.rs` (3); `foundation_wasm_ui` `integration/module/src/lib.rs` (2).
Re-grep at start; convert every site found.

## Work

- Move each site to valtron's event queue (producer `push`/`close`, consumer `readiness()`)
  or `ResumeSignal`. No site calls the executor wake.
- Then delete the deprecated raw-queue readiness constructors in `foundation_core` if feature
  80 left them for this feature (coordinate through the main session; that file is lane A's).
- Tests: per crate, the existing tests pass; add one test per converted producer that a
  parked consumer wakes promptly (check-count bound from feature 77).

## Done when

- [ ] No lane B crate uses a raw queue with polling readiness or touches the executor wake;
  the deprecated constructors are gone; CI green.
