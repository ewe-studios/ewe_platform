---
feature: "Two concurrent Turso reads both succeed on the shared connection"
status: "in-progress"
priority: "high"
depends_on: []
crates: ["foundation_db"]
issues: [357]
model: "sonnet"
branch: "fix/db-turso-concurrent-reads"
---

# Feature 13: Concurrent Turso reads

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** structured query language (SQL); GitHub (GH); AeroSpace and
Defence Industries Association of Europe (ASD), the owner of the STE standard ASD-STE100.

## 1. Goal

Make two statements that run at the same time on one `TursoStorage` both succeed. Close issue
[#357](https://github.com/ewe-studios/ewe_platform/issues/357).

This feature was added to the spec on 2026-11-07 by the owner. It is `severity: high`, and it
was found while fixing feature 10. It is not one of the 17 medium issues the spec started
with.

## 2. Problem

`backends/foundation_db/src/native/conn_gate.rs` shares one connection between tasks. A single
statement takes the **read** side of the gate, so two statements run together by design.

Turso 0.8.3 does not allow that. Every statement clones the connection's `ConcurrentGuard`, and
`TursoStatement::execute` and `step` call `try_use`. A second statement that runs while the
first is still live returns `TursoError::Misuse("concurrent use forbidden")`.

Production symptom: two concurrent `POST /identity/accounts/prelogin` requests to
`foundation_keychain` return
`{"error":"internal_error","error_description":"Backend error: concurrent use forbidden"}`. The
server has two workers, so normal load reaches the failure.

## 3. Requirements

1. Two concurrent reads on the same store both return their rows. No `Misuse` error.
2. A batch still holds the connection from `BEGIN` to `COMMIT` or `ROLLBACK`.
3. A row stream cannot be joined by another statement.
4. The documented lock timeout stays (30 s default) and still returns `StorageError::LockTimeout`.
5. No spin and no sleep. A waiting task parks.
6. The public API does not change.
7. The chosen shape and its throughput trade-off are recorded in the PR body and on #357.

## 4. Design

Two shapes are acceptable. Pick the smaller one that is correct.

- **Serialize statements on the shared connection.** A statement holds exclusive use from its
  start to the end of its row stream. One connection stays. Correct first, parallel second.
- **One connection per concurrent reader.** A small pool, each connection with its own gate.
  More parallel and a bigger change.

Do not redesign the storage layer. If neither shape is correct without a redesign, stop and
report.

## 5. Tasks

- [ ] Read `conn_gate.rs`, `turso_backend.rs` and the `LibsqlStore` equivalent. Confirm the
      failure with a test first.
- [ ] Choose the shape and record the reason.
- [ ] Implement it and keep the three invariants.
- [ ] Add the concurrency regression test.
- [ ] Run the verify commands. Record the results.
- [ ] Close #357 in the PR body.

## 6. Tests

In `backends/foundation_db/tests/`:

1. `two_concurrent_reads_both_succeed`: run two reads at the same time on one store and assert
   both return their rows. On the old code one fails with `concurrent use forbidden`.
2. The existing row-stream and batch tests still pass, and the lock-timeout test still returns
   `LockTimeout`.

The new test must fail on the old code. Run it 20 times in a row.

## 7. Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
cargo fmt -p foundation_db
cargo clippy --profile uat -p foundation_db --all-targets --no-deps -- -D warnings
cargo test --profile uat -p foundation_db
cargo test --profile uat -p foundation_keychain
```

## 8. Done when

- Two concurrent reads both succeed.
- The batch, row-stream and lock-timeout rules still hold.
- The regression test failed on the old code and passes on the new code.
- Issue #357 is closed by the PR.
- `progress.md` and `learnings.md` record the shape and the trade-off.
