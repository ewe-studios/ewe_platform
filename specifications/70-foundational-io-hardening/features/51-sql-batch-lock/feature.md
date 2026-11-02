---
feature: "foundation_db: libsql and Turso stores keep one connection per store instance behind an RwLock: many readers, one writer (writes and batches exclusive)"
status: "pending"
lane: "B"
model: "sonnet"
branch: "fix/io-51-60-lane-b"
crates: ["foundation_db"]
catalogue_rows: [17]
---

# Feature 51: one connection per store, a lock for batches

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

**Owner decision (2026-11-02, final): keep it simple.** Each store instance owns one
connection. An `RwLock` guards it: many readers, one writer. Every write, and every batch from `BEGIN`
to the end of `COMMIT`/`ROLLBACK`, has exclusive use. **No connection pool.**

## Problem (review finding)

`LibsqlStorage` (`src/native/libsql_store.rs`, `conn: Arc<libsql::Connection>`) and
`TursoStorage` (`src/native/turso_backend.rs`, `conn: Arc<turso::Connection>`) run
`execute_batch_statements` as `BEGIN`, statements, `COMMIT` with awaits between them. While it
waits, another task can run a statement on the same connection. A transaction belongs to the
connection, so that statement joins the batch:

1. A rollback of the batch also erases the other task's write (that task was told "OK").
2. A commit makes the other write atomic with the batch.
3. Other tasks read the batch's uncommitted rows.
4. A second concurrent batch fails: "cannot start a transaction within a transaction".
5. `let _ = conn.execute_batch("ROLLBACK")` ignores a failed rollback; the connection then
   stays inside a transaction and later writes are never committed.

## Design

- Add a lock next to the connection: `Arc<RwLock<()>>` using an async RwLock that is already
  in the dependency tree and does not need tokio (check `Cargo.lock`: `async-lock`,
  `futures-util`'s lock, or a `foundation_core` primitive). It parks while it waits (no spin,
  no sleep). Clones of the store share the connection and the lock (both in `Arc`).
- **RwLock rule: many readers, one writer.** Reads take the **read** guard: every
  operation that only reads (`get`, `list`, `query`/`SELECT`, existence checks). Writes take
  the **write** guard: every operation that changes data or schema (`set`, `delete`,
  `execute`, `INSERT`/`UPDATE`/`DELETE`, DDL, migrations, `INSERT … RETURNING` even when it goes
  through a query method) and every batch. Reads still run concurrently with each other; a
  write waits for current reads and blocks new ones until it ends. Classify each store method in
  a short table in the PR body; if a method can do both (a raw `execute`/`query` taking any SQL),
  it takes the write guard.
- **Batches** (`execute_batch_statements`, and any other `BEGIN` sequence: search both files
  for `BEGIN` and `execute_batch(`) hold the write guard from before `BEGIN` until after
  `COMMIT`/`ROLLBACK`.
- **Row streams** (`libsql_rows` and the Turso equivalent) hold a read guard inside the stream
  until it ends or is dropped. Document on each store type: "a batch waits until open row
  streams from this store end; do not start a batch on the same task while you hold a row
  stream from this store (it would wait for itself)."
- **Failed rollback:** never `let _ =` a rollback. If `ROLLBACK` fails, return the statement
  error with the rollback error attached, and mark the store so the next operation first checks
  for an open transaction (`SELECT` on `sqlite_master` is not enough; use the backend's
  autocommit/`in_transaction` check if it has one, else issue `ROLLBACK` again and ignore only
  "no transaction is active"). Say what each backend offers.
- Constructors stay as they are (one connection per instance).

## Tests (`backends/foundation_db/tests/`, run for libsql and Turso; the first two fail on master)

- `batch_rollback_does_not_erase_other_writes`: task A starts a batch whose 2nd statement waits
  (a test barrier or a long batch), task B inserts a row, A's 3rd statement fails; assert B's
  row exists.
- `concurrent_batches_both_commit`: two batches at once both commit.
- `reads_run_concurrently`: two slow reads overlap in time (both start before either ends).
- `write_waits_for_reads_and_blocks_new_reads`: a write issued during a read starts after
  it; a read issued during the write starts after the write.
- `batch_waits_for_open_row_stream`: hold a row stream, start a batch on another task, assert
  the batch completes only after the stream is dropped (and does not spin: thread CPU time
  under 20 ms while it waits 300 ms).
- `failed_rollback_leaves_store_usable`: inject a rollback failure (a test hook); the next
  write commits and is visible from a new store instance on the same file.
- Existing foundation_db store and contract tests pass.

## Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
python3 scripts/ci/ci.py run foundation_db --profile dev
cargo check --workspace --all-targets --profile dev
rg -n 'let _ = .*ROLLBACK' backends/foundation_db/src   # no output
```
Flake check: 20 runs of each new test.

## Done when

- [ ] Reads concurrent; writes and batches exclusive; method table in the PR body; failed
      rollback handled.
- [ ] First two tests fail on master and pass now (show both). 20/20 flake runs. CI green.
