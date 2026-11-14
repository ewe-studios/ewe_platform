# Spec 80 Progress: Platform Prerequisites

_Last updated: 2026-11-14._

## Status at a glance

| Feature | State | Source |
|---|---|---|
| 00 — env and side effects | ⏳ 0/3, design complete | Moved from spec 83 feature 08 |
| 01 — transport prerequisites | ⏳ 0/2, design complete | Moved from spec 83 feature 09 |
| 02 — migration groups | ⏳ 0/1, design complete | Moved from spec 83 feature 10 |

## Baseline from spec 76

Spec 76 is in progress and has landed `foundation_context::Ctx`, `RpcCtx`, the injected clock
and RNG, and the migration of about 600 `dyn Clock` sites. Each feature in this spec starts with
a baseline task that verifies the 76 state on master and records the remaining delta.

## Why this specification exists

The agent RPC server (65) and the proxy (82) need the same three seams. Keeping them inside 65
made the proxy depend on the agent RPC spec. They now have one owner.

## Next steps

1. Finish the move: add the nine-section story wrapper to each feature.
2. Run the baseline task of feature 00 against spec 76.
3. Implement feature 00, then feature 01; feature 02 in parallel.

## Open questions

- See each feature's open questions. The migration-group questions are in
  `features/02-migration-groups/migrations.md`.
