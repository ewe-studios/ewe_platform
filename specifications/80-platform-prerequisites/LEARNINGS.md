# LEARNINGS — Platform Prerequisites

## Why the seams left spec 83 (2026-11-14)

Spec 83 bundled four missions. The Env bundle, the transport facts and the migration groups
change shared crates that the proxy also uses. Keeping them in the agent RPC spec made the proxy
spec depend on the agent RPC spec, which is the coupling the design forbids.

The three features now have one owner. Both 65 and 82 depend on this spec.

## Do not repeat spec 76

Spec 76 already built `foundation_context::Ctx` with the clock, config and a seeded RNG, and
moved about 600 sites. This spec adds the remaining delta:

- `EntropySource`, `IdSource` and `Timer` in the `Env` bundle.
- The macro `env` argument.
- foundation_ai IDs through `Env`.
- connectrpc request extensions and the Env clock.
- foundation_http TLS peer facts.
- Migration groups and `FleetRunner`.

A second context type is a defect. Verify the baseline before writing code.

## Moved design files

- `features/00-env-and-side-effects/env.md` — the full Env design.
- `features/00-env-and-side-effects/deterministic-simulation-testing.md` — DST in plain words.
- `features/02-migration-groups/migrations.md` — the migration-group design.
