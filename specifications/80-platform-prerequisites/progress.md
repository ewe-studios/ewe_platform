# Spec 80 Progress: Platform Prerequisites

_Last updated: 2026-11-15._

## Status at a glance

| Feature | State | Source |
|---|---|---|
| 00 — transport prerequisites | ⏳ 0/2, design complete | Moved from spec 83 feature 09 |
| 01 — migration groups | ⏳ 0/1, design complete | Moved from spec 83 feature 10 |

## What left this specification

The `Env` and side-effect bundle used to be feature 00 here. The owner moved it to
[spec 84](../84-context-injection/requirements.md) on 2026-11-15, because the bundle must
agree with the landed `Ctx` of spec 76, and the two designs had diverged. Spec 84 lands first.

| Old feature | New home |
|---|---|
| 00 env and side effects (`feature.md`, `start.md`) | Deleted; spec 84 replaces them |
| `00-env-and-side-effects/env.md` | `specifications/84-context-injection/env.md` |
| `00-env-and-side-effects/deterministic-simulation-testing.md` | `specifications/84-context-injection/deterministic-simulation-testing.md` |
| 01 transport prerequisites | Feature 00 of this specification |
| 02 migration groups | Feature 01 of this specification |

## Baseline from spec 76

Spec 76 is in progress and has landed `foundation_context::Ctx`, `RpcCtx`, the injected clock
and RNG, and the migration of about 600 `dyn Clock` sites. Each feature in this spec starts with
a baseline task that verifies the 76 state on master and records the remaining delta.

## Why this specification exists

The agent RPC server (83), the proxy (82), the auth library (81) and mutual TLS (87) need the
same transport facts and the same migration groups. Keeping them inside 83 made the proxy
depend on the agent RPC spec. They now have one owner.

## Next steps

1. Run the baseline task of feature 00 against spec 76 and spec 84.
2. Implement feature 00 and feature 01. They are independent, so they may run in parallel.
3. Point the consumers at the peer facts once feature 00 lands: spec 87 (`MtlsAuth`), spec 86
   (`CallCredentials`), spec 82 (mTLS enrollment).

## Open questions

- See each feature's open questions. The migration-group questions are in
  `features/01-migration-groups/migrations.md`.

## Documentation correction (2026-11-15)

- **`MigrationSource` (feature 01, `migrations.md`).** `Directory(PathBuf)` was a
  `cfg(not(wasm))` variant, so the public enum changed shape per target. The variant now exists
  on every target; `plan` reports the new `MigrationProblem::NoFileSystem { group }` on wasm;
  the directory readers stay native only.

No code changed. Issue #556 records it, and it is closed.
