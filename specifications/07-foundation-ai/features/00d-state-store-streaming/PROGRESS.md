# Progress - 00d State Store Streaming

_Last updated: 2026-04-12_

**Status:** ⬜ Pending — 0 / 12 tasks (0%)

Fix all state stores (D1, R2, SQLite) to use `run_future_iter` so rows
stream correctly through the Valtron `StreamIterator` contract instead of
collecting upfront.

## Blocked On

- **00a foundation-db** — depends on the completed backend surfaces; can
  begin as soon as 00a's remaining work lands (arguably can start in
  parallel since it only touches already-landed backend code)

## Next Action

Candidate to pick up in parallel with the tail end of 00a — the 12 tasks
are a targeted refactor and don't require 00b/00c. Start at `start.md`.

See [`feature.md`](./feature.md) for the task list and the
`PROPOSAL_ROWS_STREAMING.md` design note in the 00a feature directory.
