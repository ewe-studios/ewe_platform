# Spec 84 Progress: Context Injection

_Last updated: 2026-11-15._

## Status at a glance

| Task | State |
|---|---|
| T1 — `Timer`, `EntropySource`, `IdSource` traits and their implementations | ⬜ Pending |
| T2 — `Env`, `EnvError`, `Ctx::env`, `Ctx::require_env`; the host builders; `ValtronTimer` | ⬜ Pending |
| T3 — `#[valtron(env = …)]`, `#[valtron_test(env = …, seed = …)]`, `single::initialize_pool_with` | ⬜ Pending |
| T4 — The effect lint in six `clippy.toml` files | ⬜ Pending |
| T5 — `foundation_ai` reads IDs and time through `Env` | ⬜ Pending |
| T6 — The DST replay test and harness | ⬜ Pending |

## No product code exists

This specification is a plan in a pull request for the owner's review. No file under
`backends/` changes until the owner approves it.

## Where this specification came from

The design was spec 80 feature 00 ("Env and Side Effects"), which came from spec 83 feature 07.
On 2026-11-15 the owner moved it here, so that `Ctx` is the only injection mechanism and the
code sits in `foundation_context`:

- `env.md` and `deterministic-simulation-testing.md` moved from
  `80-platform-prerequisites/features/00-env-and-side-effects/`.
- `feature.md` and `start.md` of the old feature became this specification's
  `requirements.md` and `start.md`.
- Spec 80 keeps two features: transport prerequisites (now 00) and migration groups (now 01).

## What changed against the old design

| Item | Old design | This specification |
|---|---|---|
| Crate of `Env` | `foundation_core::env` | `foundation_context::env` |
| Members of `Env` | clock, entropy, IDs, timer | timer, entropy, IDs |
| The clock | a member of `Env` | the engine-minimum element of `Ctx` (spec 76) |
| `Timer` | trait in `foundation_core` | trait in `foundation_context`; `ValtronTimer` in `foundation_core` |
| Builder arguments | none | `&Ctx` (for the clock) |
| `Timer::sleep_until` | `(&self, deadline)` | `(&self, ctx, deadline)` — no stored `Ctx` |

## Next steps

1. Start T1 and T2: the traits and the `Env` type. They have no dependency on the macro work.
2. T3 next, because a test that writes `#[valtron_test(env = ..)]` needs the macro.
3. T4 (the lint) after T2, so the lint sees the new code.
4. T5 (`foundation_ai`) after T2 and T3.
5. T6 last, because the replay test uses T1 to T5.

## Open questions

None. Section 12 of `requirements.md` records the five decisions that were open.
