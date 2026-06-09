# Example: SQLite Delta

## Purpose

Demonstrates `LibsqlDelta` — a SQLite-backed `DeltaStore` implementation that persists filesystem operations (creates, writes, whiteouts) in a libSQL/SQLite database. This example shows how to use SQLite as a durable, transactional delta layer for overlay filesystems, with chunked blob storage, flush/reset lifecycle, and whiteout-based deletion.

## Prerequisites

- Rust toolchain with workspace dependencies
- Feature flag: `vfs-sqlite`
- Linux or macOS (valtron thread pool support)

## How to Run

```bash
cargo run -p foundation_nativeapis --features vfs-sqlite --example sqlite_delta
```

## Architecture

`LibsqlDelta` implements the `DeltaStore` trait using an embedded SQLite database:

1. **Chunked blob storage** — Files are split into fixed-size chunks (e.g., 64KB) stored in a `chunks` table, indexed by `(path, chunk_index)`. This enables efficient random-access writes without rewriting entire files.

2. **Metadata table** — Each file/directory has a row in the `entries` table storing inode, type, permissions, size, and modification timestamp.

3. **Whiteout tracking** — Deleted files are marked with a tombstone in a `whiteouts` table, which hides base-layer entries in overlay configurations.

4. **Transactional writes** — All mutations are wrapped in SQLite transactions, ensuring atomicity even across process crashes.

5. **Sync bridge** — The async-first `LibsqlDelta` is wrapped in `SyncLibsqlDelta` using valtron's `exec_async` runtime, providing a synchronous API backed by async I/O.

The example creates files, performs reads, adds/removes whiteouts, flushes to disk, and resets the delta to demonstrate the full lifecycle.

## Expected Output

```
=== SQLite DeltaStore Example ===

Created LibsqlDelta at "/tmp/foundation_sqlite_delta_example.db"
Created /config/app.toml (42 bytes)
Wrote /config/data.csv (1000 bytes)
Read /config/app.toml:
[server]
port = 8080
host = "0.0.0.0"

stat data.csv: size=1000, inode=3
Added whiteout for /deleted_file.txt
is_whiteout: Some(WhiteoutInfo { .. })
Whiteouts in /: ["/deleted_file.txt"]
Removed whiteout

Flushed to disk
Reset — all data cleared

=== Done ===
```

## Key APIs Demonstrated

- `LibsqlDelta::new(db_path)` — Create a new SQLite-backed delta store
- `SyncLibsqlDelta::new(async_delta)` — Wrap async delta in a sync bridge via valtron
- `DeltaStore::create(path, mode)` — Create a file (stored as chunks in SQLite)
- `DeltaStore::mkdir(path)` — Create a directory entry
- `DeltaStore::write_file(path, data)` — Atomic write (chunked and persisted)
- `DeltaStore::read_file(path)` — Read entire file (reassembled from chunks)
- `DeltaStore::stat(path)` — Get metadata from the entries table
- `DeltaStore::add_whiteout(path, inode)` — Mark a file as deleted
- `DeltaStore::is_whiteout(path)` — Check if a whiteout exists
- `DeltaStore::list_whiteouts(prefix)` — Enumerate whiteouts under a prefix
- `DeltaStore::remove_whiteout(path)` — Remove a whiteout (undelete)
- `DeltaStore::flush()` — Persist all pending changes (checkpoint WAL)
- `DeltaStore::reset()` — Clear all entries, chunks, and whiteouts

## Where to Use This

- **Persistent overlays** — Survive process restarts while keeping base layer immutable
- **Edge computing** — Store deltas on resource-constrained devices (SQLite is lightweight)
- **Version control** — Each commit is a flushed delta; branches are separate databases
- **Container snapshots** — Layer multiple deltas for union mount semantics
- **Audit trails** — SQLite's journal provides a complete history of all mutations
- **Offline-first apps** — Queue changes locally in SQLite, sync when online

## Database Schema

```sql
CREATE TABLE entries (
    path TEXT PRIMARY KEY,
    inode INTEGER NOT NULL UNIQUE,
    file_type TEXT NOT NULL,  -- 'file' | 'directory' | 'symlink'
    size INTEGER,
    mode INTEGER,
    created_at REAL,
    modified_at REAL
);

CREATE TABLE chunks (
    path TEXT NOT NULL,
    chunk_index INTEGER NOT NULL,
    data BLOB NOT NULL,
    PRIMARY KEY (path, chunk_index),
    FOREIGN KEY (path) REFERENCES entries(path) ON DELETE CASCADE
);

CREATE TABLE whiteouts (
    path TEXT PRIMARY KEY,
    base_inode INTEGER,
    created_at REAL
);
```

## Related

- Feature spec: `specifications/37-overlay-vfs/features/06-sqlite-delta/feature.md`
- Source: `src/shared/vfs/libsql_delta.rs`
- Delta store trait: `src/shared/vfs/delta_store.rs` (`DeltaStore`)
- Valtron runtime: `foundation_core/src/valtron/mod.rs`
