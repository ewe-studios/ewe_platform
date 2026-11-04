---
feature: "Delete stale backlog entries; fix spec 70 frontmatter; record review findings"
status: "pending"
lane: "B"
model: "haiku"
branch: "fix/io-51-60-lane-b"
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

- [ ] No backlog entry describes a fixed bug. Spec 70 frontmatter correct.

## E-drop. Features 57 and 58 are dropped (owner decision 2026-11-04)

Move the backlog entries for `crates/channels` (§18), `crates/domain` (§19), `examples/todo`
(§23) and the `bin/platform` web-app templates (§24) to the "Paused" section of
`docs/bug-backlog.md`, with the line "Dropped by the owner 2026-11-04: old code, not
scheduled." Do not fix them. Do not add new entries for those crates.

