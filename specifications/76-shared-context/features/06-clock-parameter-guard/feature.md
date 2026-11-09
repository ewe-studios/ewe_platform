---
feature: "Clock parameter guard"
status: "planned"
priority: "medium"
model: "haiku"
depends_on: ["03-ctx-in-foundation-ai", "04-ctx-in-io-crates", "05-ctx-in-remaining-crates"]
crates: []
---

# Feature 06: Clock parameter guard

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** continuous integration (CI); AeroSpace and Defence Industries
Association of Europe (ASD), the owner of the STE standard ASD-STE100.

## Goal

Stop new `Arc<dyn Clock>` parameters and fields after features 03 to 05 remove them.

## Work

1. Add `scripts/ci/check_clock_params.py`, in the style of `scripts/ci/check_time_sources.py`
   (a `--root` argument and an allowlist file, `scripts/ci/clock_params_allowlist.txt`, with
   one `path: reason` line per allowed site).
   The check scans `backends/*/src`, `bin/*/src` and `apps/*/src` for `dyn Clock` and fails on
   each site that is not in the allowlist. It skips `foundation_context` and the
   `foundation_core/src/valtron` files that build or hold the clock.
   Also fail on `MonotonicClock::new()` outside host entry points (the allowlist names them),
   so #463 does not come back.
2. Add a unittest, `scripts/ci/tests/test_check_clock_params.py`: one case passes, one case
   fails on a new site, and one case passes on an allowlisted site.
3. Run the check in `.github/workflows/check.yaml`. Issue #462 says the repository guards
   run only inside the foundation_core job. If #462 is fixed first, add the check to its
   guards job. If not, run it where `check_time_sources.py` runs.
4. Change root rule 11 in `CLAUDE.md` and the matching text in `.agents/AGENTS.md`. Code reads
   side effects from `foundation_context::Ctx` (`ctx.clock()`, `ctx.rng()`, `ctx.get::<T>()`),
   and it takes a `Ctx`, not a bare clock.

## Verify

```bash
python3 scripts/ci/check_clock_params.py
python3 -m unittest discover -s scripts/ci/tests
```

## Done when

- The guard runs in CI and passes on master.
- The rule text names `foundation_context::Ctx` as the one context.
