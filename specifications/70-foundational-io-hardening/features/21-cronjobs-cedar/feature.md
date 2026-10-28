---
feature: "foundation_cronjobs runs jobs on their cron expression; foundation_cedar loads policies in one query and builds warning-free"
status: "pending"
priority: "high"
depends_on: ["01-valtron-parking"]
branch: "fix/io-21-cronjobs-cedar"
crates: ["foundation_cronjobs", "foundation_cedar"]
backlog: "docs/bug-backlog.md §14 (2 entries), §15 (3 entries)"
---

# Feature 21: `foundation_cronjobs` and `foundation_cedar`

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** Structured Query Language (SQL); pull request (PR); AeroSpace and
Defence Industries Association of Europe (ASD), the owner of the STE standard ASD-STE100.

Use the shared protocol in [`requirements.md`](../../requirements.md) sections 3 and 4.

## T0. Check that every entry is still open (table in the PR body)

## E1. `CronJob::next_fire` ignores the cron expression (cronjobs)

- **Where:** `src/scheduler.rs`: always five minutes from now (TODO).
- **Fix:** parse `cron_expr` with a maintained crate (`cron` or `saffron`; pick one, say why);
  compute the real next fire time from the valtron clock; reject an invalid expression at
  `register` with a typed error.
- **Tests:** `next_fire_follows_expression` (table: `*/15 * * * *`, `0 9 * * 1-5`, month ends,
  leap day); `register_rejects_invalid_expression`.

## E2. `CronScheduler` never runs a job (cronjobs)

- **Problem:** the crate docs describe tasks parked on `Delayed` until each fire time, execution,
  result recording and missed-job catch-up (`MissedJobPolicy`); none of it exists.
- **Fix:** implement the run loop: one valtron task per job, parked (`TaskStatus::Delayed`) until
  its next fire time, runs the job, records the result, computes the next time; the missed-job
  policy at start (catch up once, skip, or run all, as `MissedJobPolicy` defines). Use the valtron
  clock so tests can move time.
- **Tests:** `scheduler_runs_job_at_fire_time` (simulated clock); `scheduler_records_result`;
  one test per `MissedJobPolicy` value; `scheduler_parks_between_fires` (no polls between fires).

## E3. `SqlPolicyStore` reads its row once per field (cedar)

- **Where:** `load_from_store` calls `load_schema`, `load_policies`, `load_entities`; each runs
  the same `SELECT` (`src/storage/sql_store.rs`).
- **Fix:** `PolicyStore::load_all` (default: the three calls); `SqlPolicyStore` overrides with one
  query. **Test:** `sql_policy_store_loads_in_one_query` (a counting `QueryStore`).

## E4. `derive_more` is unused (cedar) — drop it.

## E5. `unreachable call` warnings in `src/core/request.rs` (cedar)

- **Fix:** drop the `map_err` on the three `EntityId::from_str(id)` calls (`Infallible`) or use
  `EntityId::new`. **Verify:** `cargo check -p foundation_cedar --all-targets 2>&1 | rg warning` is empty.

## Verification (run exactly)

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
python3 scripts/ci/ci.py run foundation_cronjobs --profile dev
python3 scripts/ci/ci.py run foundation_cedar --profile dev
cargo check --workspace --all-targets --profile dev
```

## Done when

- [ ] T0 table; every open entry fixed; backlog entries deleted.
- [ ] Scheduler tests use a simulated clock and are deterministic (20 runs).
- [ ] Crate docs describe only what exists. CI green.
