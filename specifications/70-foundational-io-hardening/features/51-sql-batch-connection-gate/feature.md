---
feature: "foundation_db: libsql and Turso batches cannot mix with other operations on the shared connection"
status: "pending"
lane: "B"
model: "sonnet"
branch: "fix/io-51-60-lane-b"
crates: ["foundation_db"]
catalogue_rows: [17]
---

# Feature 51: batch transactions on a shared connection

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

## Problem (review finding)

`LibsqlStorage` (`src/native/libsql_store.rs`, field `conn: Arc<libsql::Connection>`) and
`TursoStorage` (`src/native/turso_backend.rs`, `conn: Arc<turso::Connection>`) use one
connection for every operation. `execute_batch_statements` runs `BEGIN`, the statements,
then `COMMIT`/`ROLLBACK` on that connection, awaiting between them. While it waits, another
task can run a statement on the same connection:

- that statement joins the open transaction (it is committed or rolled back with the batch,
  and it sees uncommitted rows);
- a second concurrent batch fails with "cannot start a transaction within a transaction".

The same applies to any other multi-statement sequence on these stores (search for
`BEGIN` and `execute_batch(` in both files).

## Design

Add a connection gate to both stores: `gate: Arc<async_lock::RwLock<()>>` (or the
`futures-util` / `foundation_core` async RwLock already in the dependency tree; do not add a
new crate if one exists — check `Cargo.toml`).

- Every single-statement operation takes a **read** guard for the time it uses the
  connection.
- A batch (and any other `BEGIN` sequence) takes the **write** guard from `BEGIN` until
  after `COMMIT`/`ROLLBACK`.
- Streaming queries (`libsql_rows`, the Turso equivalent): hold the read guard inside the
  stream until it ends or is dropped. Document on the store type: "a batch waits for open row
  streams to end; do not start a batch while you hold a row stream from the same store on the
  same task" (that would deadlock).
- Clones of the store share the gate (it is in an `Arc`).

Alternative you may choose if simpler and correct: a dedicated connection per batch from the
stored `Database` — **only** for file-backed databases (an in-memory SQLite database is
per-connection, so a new connection would see an empty database). If you choose it, you still
need the gate for in-memory and remote. Say which you chose.

## Tests (`backends/foundation_db/tests/`, fail on master)

- `batch_is_isolated_from_concurrent_writes` (libsql and Turso): start a batch whose second
  statement waits (use a statement list long enough, or a test hook) while another task inserts
  a row; make the batch fail; assert the other task's row still exists (on master it is rolled
  back with the batch).
- `concurrent_batches_both_succeed` (both backends): two batches at once both commit.
- Run each 20 times.

## Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
python3 scripts/ci/ci.py run foundation_db --profile dev
```

## Done when

- [ ] Both tests fail on master and pass now, 20/20. Doc comment on the deadlock rule. CI green.
