# Spec 50 To-do: Valtron Async Readiness

_Updated: 2026-10-22. Written in ASD-STE100 Simplified Technical English (STE)._

**Abbreviations in this file.** pull request (PR); continuous integration (CI); application
programming interface (API); Structured Query Language (SQL); identifier (ID); AeroSpace and
Defence Industries Association of Europe (ASD), the owner of the STE standard ASD-STE100.

This file lists the work that is still open in this spec, in order. `progress.md` says what is
done. `learnings.md` says what we learned.

## Now

| # | Item | State | Done when |
|---|---|---|---|
| 1 | Merge PR #134 (rename the async storage traits to plain names) | CI and review | #134 is merged and CI on `master` is green |

## Next (this spec)

| # | Item | Detail | Done when |
|---|---|---|---|
| 2 | F02: readiness-source seam | Not implemented (checked 2026-10-22). Add the `ReadinessSource` trait in `foundation_core` and the in-tree test reactor, as `features/02-readiness-source-seam/feature.md` says. | The F02 tests pass; `foundation_netio` can plug a reactor in. |
| 3 | Feature front matter | F01 and F03 exist on `master` but their front matter says `pending`. Set F01 and F03 to `done` with evidence. Keep F02 `pending` until item 2. | Front matter matches the code. |
| 4 | Storage method suffixes | The storage traits keep `_async` on each method (`get_async`, `query_async`, …). The owner did not decide on these. Ask: drop them in one rename PR, or keep them. Some stores have inherent methods with the plain names, so a rename needs a clash check per type. | The owner decides; if "drop", a mechanical PR like #134. |
| 5 | `AsyncDeltaStore`, `AsyncVectorStore` | Their sync twins (`DeltaStore` in foundation_nativeapis, `VectorStore` in foundation_vectors) still exist. Move their last callers to async, delete the sync traits, then rename. | No sync twin remains; the plain names are free. |

## Later (other specs)

| # | Item | Where |
|---|---|---|
| 6 | Agent-rpc pluggable design (spec 65 feature 07), starts after item 1 | `specifications/65-agent-rpc-serving-system/todo.md` |
| 7 | Migration runner: directory and embedded sources, duplicate-number checks, missing migration 023 | spec 65 feature 07 task T25 |
| 8 | A Postgres backend for `foundation_db` (owner: a separate feature) | a new storage feature after spec 65 feature 07 |
| 9 | The open bugs in `docs/bug-backlog.md` §10–§15 | `docs/bug-backlog.md` |
