# Spec 76 progress

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** pull request (PR); AeroSpace and Defence Industries
Association of Europe (ASD), the owner of the STE standard ASD-STE100.

## Landed

- 2026-11-09: the spec, with six features. The owner chose a new crate (`foundation_context`),
  the name `RpcCtx`, and one PR per feature.

- 2026-11-09: feature 01. The new crate `backends/foundation_context` owns `Clock`,
  `MonotonicClock`, `Config`, `MinimumElement` and `Ctx`. `foundation_core::valtron`
  re-exports them. `SimulatedClock` stays in `foundation_core`. The crate is in
  `ci/packages.toml`, and the time-source allowlist points at the new path.

## In progress

- Nothing. Feature 01 is done and waits for review.

## Next

- Feature 02 after feature 01 is reviewed. Then features 03, 04 and 05 can run in parallel.
  Feature 06 is last.
