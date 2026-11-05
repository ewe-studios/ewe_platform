---
feature: "Delete stale backlog entries; fix spec 70 frontmatter; record review findings"
status: "done"
lane: "B"
model: "haiku"
branch: "spec70/f59-f60-f79-lane-b-mechanical"
crates: ["docs", "specifications"]
catalogue_rows: [30]
---

# Feature 60: bookkeeping

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

Do this first (it is small) and again at the end.

- **E1 stale entries** in `docs/bug-backlog.md` that the code already fixes (verify each,
  write the evidence line in the PR body, then delete):
  - §11 `KVWasmStorage`'s rate limiter (the store now documents KV as best effort and
    points to D1/Durable Object) — confirm the D1 `RateLimiterStore` exists and is atomic.
  - §11 old-shape `rate_limits` table (migration `027_rebuild_rate_limits` exists).
  - §11 wasm stores' inherent `*_async` API (`rg -n 'pub (async )?fn \w+_async' backends/foundation_db/src/wasm` is empty).
  - §11 no CI job for examples (`[packages.examples]` exists in `ci/packages.toml`).
  - §16 `foundation_reactor` publish note: keep only if 0.0.7 is not yet published; else
    delete.
- **E2 spec 70** frontmatter and features table: done by the main session when it wrote features 41–60 (check that the counts still match when lane B ends).
- **E3** At the end of lane B: confirm every catalogue row owned by lane B is fixed or has a
  backlog entry; write the result in `specifications/70-foundational-io-hardening/progress.md`.

## Done when

- [x] No backlog entry describes a fixed bug. Spec 70 frontmatter correct.

## Result

E1, stale entries deleted after checking each against the code:
- §11 `KVWasmStorage` rate limiter: the store docs say best effort and point
  to the D1-backed `RateLimiterStore` (or a Durable Object). `D1WasmStorage`
  implements it with one atomic `RECORD_SQL` statement (`RETURNING count`).
- §11 old-shape `rate_limits` table:
  `src/core/schema/sql/027_rebuild_rate_limits.sql` drops and recreates it.
- §11 wasm stores' inherent `*_async` API:
  `rg -n 'pub (async )?fn \w+_async' backends/foundation_db/src/wasm` is empty.
- §11 no CI job for examples: `[packages.examples]` exists in
  `ci/packages.toml` and `scripts/ci/ci.py` builds them.
- §16 `foundation_reactor` publish note: `foundation_core` 0.0.7 is published
  on crates.io, so the ordering note is no longer needed.

E-drop: `docs/bug-backlog.md` has no §18, §19, §23 or §24 entries; they were
already removed.

E2: the spec 70 frontmatter `tasks` counts now match the feature files:
completed 67, uncompleted 12, total 81, dropped 2, 83%.

E3: the lane B catalogue rows 29, 30 and 102 are fixed by features 59, 60 and
79 in this branch. The other lane B rows belong to features that have not
landed on master (51–56, 66, 68, 71, 73, 74, 76, 81) and keep their feature or
backlog entry. Rows 27, 28 and the §18, §19, §23 and §24 entries were dropped
by the owner (features 57 and 58) and are gone.

## E-drop. Features 57 and 58 are dropped (owner decision 2026-11-04)

Delete the backlog entries for `crates/channels` (§18), `crates/domain` (§19), `examples/todo`
(§23) and the `bin/platform` web-app templates (§24) from `docs/bug-backlog.md` (owner: delete,
do not keep). Keep §7's vms clippy entry: it is feature 79. Do not add new entries for those
crates.
