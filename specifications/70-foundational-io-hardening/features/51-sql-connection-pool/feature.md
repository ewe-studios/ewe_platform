---
feature: "foundation_db: libsql and Turso stores use a connection pool; each operation and each batch gets its own connection"
status: "pending"
lane: "B"
model: "sonnet"
branch: "fix/io-51-60-lane-b"
crates: ["foundation_db"]
catalogue_rows: [17]
---

# Feature 51: connection pool for libsql and Turso

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

**Owner decision (2026-11-02):** valtron tasks must not share one connection. Each operation
checks out its own connection from a pool. Do not use a lock around one shared connection.

## Problem (review finding)

`LibsqlStorage` (`src/native/libsql_store.rs`, `conn: Arc<libsql::Connection>`) and
`TursoStorage` (`src/native/turso_backend.rs`, `conn: Arc<turso::Connection>`) use one
connection for every task. A transaction belongs to the connection, so while
`execute_batch_statements` is between `BEGIN` and `COMMIT` (awaiting), any statement from any
other task joins that transaction:

1. A rollback of the batch also erases other tasks' writes (they were told "OK").
2. A commit makes unrelated writes atomic with the batch.
3. Other tasks read the batch's uncommitted rows.
4. A second concurrent batch fails: "cannot start a transaction within a transaction".
5. `let _ = conn.execute_batch("ROLLBACK")` ignores a failed rollback; the shared connection
   then stays inside a transaction and later writes from every task are never committed.

## Design

### Pool type (shared by both backends)

Add `foundation_db::native::pool` with a small generic async pool, used by both stores:

- `ConnectionPool<C>` holds the `Database` handle and idle connections; `acquire().await`
  returns a `PooledConnection<C>` guard (`Deref` to the connection). Dropping the guard returns
  the connection to the idle list.
- Bounded: at most `max_connections` live connections. When all are checked out, `acquire`
  **parks** (an async semaphore or a valtron wait; never a spin or sleep loop) until one is
  returned, up to `acquire_timeout`, then returns a typed `StorageError::PoolTimeout`.
- `PooledConnection::discard()` drops the connection instead of returning it (the slot frees).
- Health: a connection is discarded, not returned, if its operation ended with a
  connection-level error or if it may still be inside a transaction (see batches).
- Check whether a suitable pool crate is already in the dependency tree (`Cargo.lock`) and is
  runtime-agnostic (no tokio). If one fits, you may use it; otherwise write the pool (about
  150 lines). Say which and why.

### `PoolConfig` (rule 8: configuration, documented defaults)

| Field | Default | Source to cite in the doc comment |
|---|---|---|
| `max_connections` | 10 | sqlx `PoolOptions` and HikariCP default |
| `acquire_timeout` | 30 s | sqlx default |
| `idle_timeout` | 10 min (`None` = keep) | sqlx default |
| `busy_timeout` | 5 s | SQLite `busy_timeout`, commonly 5 s (litestream guidance) |

Builder methods on the store builders (`LibsqlStorage::builder()`, Turso equivalent, or the
existing constructors plus `with_pool_config`).

### Per connection setup

Run on each new connection: `PRAGMA busy_timeout = <busy_timeout ms>` (several connections
writing one file otherwise get `SQLITE_BUSY` at once), and for file databases
`PRAGMA journal_mode = WAL` once (readers do not block the writer). Check what each backend
supports and say what you set. Remote libsql (Hrana/HTTP): each `connect()` is its own stream;
skip the pragmas that do not apply.

### In-memory databases (must research and prove)

An SQLite `:memory:` database exists **per connection**; a pool of them would be N empty
databases. For each backend, find how several connections can share one in-memory database:

- libsql local: a named shared-cache URI (`file:<unique-name>?mode=memory&cache=shared`, with
  URI open flags), with a unique name per store so tests do not share data.
- Turso (`turso` crate): check whether `Database::connect()` on one `Database` shares the
  in-memory data (it likely does: the `Database` owns the storage).

Prove it with a test (`memory_pool_connections_share_data`: write on one pooled connection,
read on another). If a backend cannot share an in-memory database, set `max_connections = 1`
for in-memory only, document it on the store, and say so in the report.

### Using the pool

- Every operation: `acquire` → run → release. Row streams (`libsql_rows` and the Turso
  equivalent) own their `PooledConnection` inside the stream and release it when the stream ends
  or is dropped. Document on the store type: a task that holds `max_connections` open row streams
  and then starts another operation waits until `acquire_timeout` (the usual pool rule).
- Batches (`execute_batch_statements`, and any other `BEGIN` sequence: search both files for
  `BEGIN` and `execute_batch(`): acquire one connection, `BEGIN`, statements, `COMMIT`. On a
  statement error: `ROLLBACK`; **if the rollback fails, `discard()` the connection** and return
  the statement error with the rollback error attached. Never `let _ =` a rollback.
- Remove the `conn: Arc<…Connection>` field. Constructors that take an existing single
  `Connection` (if any public ones exist): keep them as a pool of size 1 with a doc note, or
  remove them if no caller uses them (check callers in all crates; lane B owns foundation_db, but
  a caller in a lane A crate must keep compiling — report if one would break).

## Tests (`backends/foundation_db/tests/`, run for libsql and Turso; the first two fail on master)

- `batch_rollback_does_not_erase_other_writes`: task A starts a batch (make its 2nd statement
  wait on a test barrier, or use a long batch), task B inserts a row, A's 3rd statement fails;
  assert B's row exists.
- `concurrent_batches_both_commit`: two batches at once both succeed.
- `failed_rollback_discards_connection`: inject a rollback failure (a test hook or a connection
  closed mid-batch); assert the next operation works and is not inside a transaction.
- `pool_parks_when_exhausted`: `max_connections = 2`, hold 2 guards, a 3rd `acquire` waits
  (no CPU spin: thread CPU time under 20 ms over 500 ms) and succeeds when one is dropped.
- `pool_acquire_times_out`: `acquire_timeout = 200 ms`, all held → `PoolTimeout`.
- `memory_pool_connections_share_data` (both backends).
- `pool_config_defaults_are_documented_values` (the defaults in the table).
- Existing foundation_db store and contract tests pass on the pooled stores.

## Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
python3 scripts/ci/ci.py run foundation_db --profile dev
# every crate that uses LibsqlStorage / TursoStorage still builds:
cargo check --workspace --all-targets --profile dev
rg -n 'conn: Arc<(libsql|turso)::Connection>' backends/foundation_db/src   # no output
rg -n 'let _ = .*ROLLBACK' backends/foundation_db/src                      # no output
```
Flake check: 20 runs of each new test.

## Done when

- [ ] Both stores pooled; no shared single connection; batches isolated; failed rollback
      discards the connection.
- [ ] In-memory behaviour proven per backend (shared, or size 1 documented).
- [ ] First two tests fail on master and pass now (show both). 20/20 flake runs. CI green.
