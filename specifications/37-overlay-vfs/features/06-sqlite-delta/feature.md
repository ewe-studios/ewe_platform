---
feature_name: "SqliteDelta"
description: "SqliteDelta — libsql-backed DeltaStore providing a complete VfsFileSystem (file CRUD, directory hierarchy, metadata, chunked content) plus DeltaStore whiteout/lifecycle extensions, all in a single .db file. Feature-gated behind vfs-sqlite."
status: "in-progress"
priority: "medium"
phase: 3
created: 2026-06-04
updated: 2026-06-06
dependencies:
  - "01-core-traits"
tasks:
  completed: 44
  uncompleted: 12
  total: 56
  completion_percentage: 79%
---

# Feature 06: SqliteDelta

## Overview

A complete VfsFileSystem and DeltaStore backed by a single SQLite database file (via **libsql**). This is not just a delta store — it is a full filesystem implementation where every file, directory, symlink, and metadata record lives in SQL tables. The DeltaStore trait adds whiteout tracking and lifecycle operations on top.

Inspired by AgentFS schema (inode/dentry/data tables), adapted for local SQLite specifics: WAL mode for concurrent readers, larger default chunk size (64 KB), no HTTP round-trip concerns.

Feature-gated behind `vfs-sqlite` to avoid adding libsql as a mandatory dependency.

### Dual Identity

The `SqliteDelta` struct provides **both** roles:

1. **SqliteFs** (VfsFileSystem) — full file CRUD, directory hierarchy, metadata, chunked content storage, symlinks. Every `VfsFileSystem` method maps to SQL queries against the dentry/chunks tables.
2. **SqliteDelta** (DeltaStore extending VfsFileSystem) — adds whiteout table, `flush()` maps to WAL checkpoint, `reset()` maps to DELETE all rows.

A single struct implements both traits. Callers that only need `VfsFileSystem` use it as such; `OverlayFileSystem` uses it as `DeltaStore`.

### Module Structure

```
src/shared/vfs/
    libsql_delta/
        mod.rs              # LibsqlDelta struct, constructors, VfsFileSystem + DeltaStore impls
        schema.rs           # SQL table definitions, migrations, schema version
        chunking.rs         # Chunk sizing, read/write assembly, partial reads
        file_handle.rs      # SqliteFile, SeekableSqliteFile implementations
        types.rs            # SqliteDentry, SqliteChunkRef, internal types
        path_resolve.rs     # Path → ino resolution via dentry lookups
```

## libsql Library Choice

Uses **libsql** (`libsql` crate — Turso's fork of SQLite) instead of `rusqlite`.

**Why libsql:**

- **Drop-in SQLite compatibility** — same SQL dialect, same file format, readable by `sqlite3` CLI
- **Edge replication** — Turso cloud sync for future remote replication scenarios (spec 36 agentic API)
- **Embedded replicas** — local SQLite file that can sync to/from a Turso remote, enabling hybrid local+cloud delta stores
- **WASM support** — libsql compiles to WASM, aligning with the platform's cross-target strategy
- **Active maintenance** — Turso actively develops libsql with upstream SQLite merges
- **API similarity to rusqlite** — migration path is straightforward; `Connection`, `Statement`, `Row` patterns are nearly identical

```toml
[dependencies]
libsql = { version = "0.6", optional = true, default-features = false, features = ["core"] }
```

## SQL Schema

Three tables mirror the D1 feature's schema (feature 13), adapted for local SQLite.

```sql
-- Directory entries (hierarchy + metadata)
CREATE TABLE IF NOT EXISTS vfs_dentry (
    ino           INTEGER PRIMARY KEY AUTOINCREMENT,
    name          TEXT NOT NULL,
    parent_ino    INTEGER NOT NULL,
    file_type     TEXT NOT NULL CHECK (file_type IN ('file', 'dir', 'symlink')),
    size          INTEGER NOT NULL DEFAULT 0,
    permissions   INTEGER NOT NULL DEFAULT 493,  -- 0o755
    owner_uid     INTEGER NOT NULL DEFAULT 0,
    owner_gid     INTEGER NOT NULL DEFAULT 0,
    checksum      BLOB,                          -- blake3 32 bytes, NULL for dirs
    version_id    BLOB(16) NOT NULL,             -- SCRU128 version
    created_at    INTEGER NOT NULL,              -- unix epoch milliseconds
    updated_at    INTEGER NOT NULL,              -- unix epoch milliseconds
    symlink_target TEXT,                         -- target path if file_type = 'symlink'
    chunk_size    INTEGER NOT NULL DEFAULT 65536, -- chunk size used for this file
    UNIQUE(parent_ino, name)
);

-- Root directory bootstrap (ino = 1, self-referencing parent)
INSERT OR IGNORE INTO vfs_dentry (ino, name, parent_ino, file_type, size, permissions,
    owner_uid, owner_gid, version_id, created_at, updated_at, chunk_size)
VALUES (1, '', 1, 'dir', 0, 493, 0, 0,
     X'00000000000000000000000000000000',
    CAST(strftime('%s', 'now') * 1000 AS INTEGER),
    CAST(strftime('%s', 'now') * 1000 AS INTEGER),
    65536);

-- File content chunks (only for file_type = 'file')
CREATE TABLE IF NOT EXISTS vfs_chunks (
    ino         INTEGER NOT NULL,
    chunk_idx   INTEGER NOT NULL,
    data        BLOB NOT NULL,
    PRIMARY KEY (ino, chunk_idx),
    FOREIGN KEY (ino) REFERENCES vfs_dentry(ino) ON DELETE CASCADE
);

-- Whiteouts (DeltaStore extension)
CREATE TABLE IF NOT EXISTS vfs_whiteouts (
    path        TEXT PRIMARY KEY,
    version_id  BLOB(16) NOT NULL  -- SCRU128 version
);

-- Hierarchical whiteout prefix index for O(1) list_whiteouts (see below)
CREATE TABLE IF NOT EXISTS vfs_whiteout_prefixes (
    prefix      TEXT NOT NULL,
    path        TEXT NOT NULL,
    version_id  BLOB(16) NOT NULL,
    PRIMARY KEY (prefix, path)
);

-- Indexes for common query patterns
CREATE INDEX IF NOT EXISTS idx_dentry_parent ON vfs_dentry(parent_ino);
CREATE INDEX IF NOT EXISTS idx_chunks_ino ON vfs_chunks(ino);
CREATE INDEX IF NOT EXISTS idx_whiteouts_path ON vfs_whiteouts(path);
CREATE INDEX IF NOT EXISTS idx_whiteout_prefix ON vfs_whiteout_prefixes(prefix);
```

### Schema Notes

- **Table names use `vfs_` prefix** — SQLite reserves `sqlite_` for internal tables. Using `sqlite_dentry`, `sqlite_chunks` etc. causes `LibsqlDelta::new()` to fail with "table name is reserved" error. All tables must use `vfs_` prefix.
- **`ino` is AUTOINCREMENT** — unlike D1 which may use UUIDs, local SQLite benefits from monotonic integer keys for B-tree locality
- **`UNIQUE(parent_ino, name)`** — enforces no duplicate filenames within a directory, atomically prevents races
- **`ON DELETE CASCADE`** on chunks — removing a dentry automatically removes all its content chunks
- **`chunk_size` per file** — stored in dentry so readers know how to reassemble even if the default changes
- **Root directory** is `ino = 1` with `parent_ino = 1` (self-referencing). The empty name `''` is the root sentinel.
- **`checksum` is BLOB** not TEXT — stores raw blake3 bytes (32 bytes), avoids hex encoding overhead

## Path Resolution

Path resolution walks the `sqlite_dentry` table from root to target, one component at a time.

### Single-step lookup (primary path)

The overlay resolves paths layer-by-layer, so `SqliteDelta` typically receives already-normalized absolute paths. Resolution splits the path into components and walks:

```sql
-- For each component in the path: "/foo/bar/baz.txt" → ["foo", "bar", "baz.txt"]
-- Start from root ino = 1, walk each component:
SELECT ino, file_type, size FROM sqlite_dentry WHERE parent_ino = ? AND name = ?;
```

This is a single-row indexed lookup per path component — fast even for deep hierarchies.

### Recursive CTE (batch operations)

For operations that need to resolve multiple paths or list subtrees:

```sql
WITH RECURSIVE subtree(ino, name, parent_ino, file_type, depth, full_path) AS (
    SELECT ino, name, parent_ino, file_type, 0, ''
    FROM sqlite_dentry WHERE ino = ?  -- starting directory ino

    UNION ALL

    SELECT d.ino, d.name, d.parent_ino, d.file_type, s.depth + 1,
           s.full_path || '/' || d.name
    FROM sqlite_dentry d
    JOIN subtree s ON d.parent_ino = s.ino
    WHERE s.file_type = 'dir'
)
SELECT * FROM subtree ORDER BY depth, name;
```

Used by `remove_all()` to find all descendants for cascading delete, and by future tree-diff operations.

## VfsFileSystem Method Mapping

Every `VfsFileSystem` trait method has a concrete SQL implementation:

### `capabilities() -> VfsCapabilities`

Returns static capabilities:
```rust
VfsCapabilities {
    seekable: true,
    symlinks: true,
    permissions_enforced: false,  // stored but not enforced
    event_emission: false,
    persistent: true,
}
```

### `stat(path) -> VfsMetadata`

```sql
-- Resolve path to ino (walk components), then:
SELECT ino, file_type, size, permissions, owner_uid, owner_gid,
       checksum, version, created_at, updated_at
FROM sqlite_dentry WHERE ino = ?;
```

Maps columns to `VfsMetadata` fields. `created_at`/`updated_at` are milliseconds since epoch, converted to `SystemTime`. `checksum` BLOB is mapped to `Checksum::Blake3([u8; 32])` or `Checksum::None` if NULL.

### `exists(path) -> bool`

Path resolution; returns `true` if resolution succeeds (all components found), `false` on `NotFound`.

### `open(path, mode) -> SqliteFile`

1. Resolve path to ino
2. Verify `file_type = 'file'` (else `NotAFile`)
3. If mode is `Write` or `ReadWrite`, verify path exists (else `NotFound`)
4. Return `SqliteFile { db, ino, mode }`

### `open_seekable(path, mode) -> SeekableSqliteFile`

Same as `open` but returns `SeekableSqliteFile { inner: SqliteFile, cursor: 0 }`.

### `open_directory(path) -> SqliteDirectory`

1. Resolve path to ino
2. Verify `file_type = 'dir'` (else `NotADirectory`)
3. Return `SqliteDirectory { db, ino, path }`

### `create(path, mode) -> SqliteFile`

```sql
BEGIN;
-- Ensure parent directory exists (resolve parent path to parent_ino)
-- Insert new file entry:
INSERT INTO sqlite_dentry (name, parent_ino, file_type, size, permissions,
    owner_uid, owner_gid, version, created_at, updated_at, chunk_size)
VALUES (?, ?, 'file', 0, 0o644, 0, 0, 0, ?, ?, ?);
-- No chunks inserted yet (empty file)
COMMIT;
```

Returns `SqliteFile` for the newly created entry. If the path already exists, returns `AlreadyExists`.

### `mkdir(path)`

```sql
INSERT INTO sqlite_dentry (name, parent_ino, file_type, size, permissions,
    owner_uid, owner_gid, version, created_at, updated_at, chunk_size)
VALUES (?, ?, 'dir', 0, 0o755, 0, 0, 0, ?, ?, 0);
```

Parent must exist and be a directory. Returns `AlreadyExists` if directory already exists.

### `remove(path)`

```sql
BEGIN;
-- Resolve path to ino
-- If directory, verify it's empty:
SELECT COUNT(*) FROM sqlite_dentry WHERE parent_ino = ?;
-- Delete the entry (CASCADE removes chunks):
DELETE FROM sqlite_dentry WHERE ino = ?;
COMMIT;
```

Non-empty directories return an error; use `remove_all` for recursive deletion.

### `rename(from, to)`

```sql
BEGIN;
-- Resolve 'from' path to ino
-- Resolve 'to' parent path to new_parent_ino
-- If 'to' exists, remove it first (overwrite semantics)
UPDATE sqlite_dentry SET name = ?, parent_ino = ?, updated_at = ? WHERE ino = ?;
COMMIT;
```

The `UNIQUE(parent_ino, name)` constraint ensures atomicity.

### `chmod(path, mode)`

```sql
UPDATE sqlite_dentry SET permissions = ?, updated_at = ? WHERE ino = ?;
```

### `symlink(target, link_path)`

```sql
INSERT INTO sqlite_dentry (name, parent_ino, file_type, size, permissions,
    owner_uid, owner_gid, version, created_at, updated_at, symlink_target, chunk_size)
VALUES (?, ?, 'symlink', 0, 0o777, 0, 0, 0, ?, ?, ?, 0);
```

### `readlink(path) -> String`

```sql
SELECT symlink_target FROM sqlite_dentry WHERE ino = ? AND file_type = 'symlink';
```

Returns `NotFound` if not a symlink.

### `list` (via VfsDirectory)

```sql
SELECT name, file_type FROM sqlite_dentry WHERE parent_ino = ? ORDER BY name;
```

Maps to `Vec<VfsDirEntry>`.

### Default implementations

`read_file`, `write_file`, `copy`, `remove_all`, `mkdir_all` use the default trait implementations which compose the primitive operations above.

## Chunking Strategy

File content is stored as ordered chunks in `sqlite_chunks`. This enables partial reads/writes without loading entire files into memory.

### Configuration

```rust
pub struct ChunkConfig {
    /// Default chunk size in bytes. Default: 64 KB.
    pub default_chunk_size: usize,
}

impl Default for ChunkConfig {
    fn default() -> Self {
        Self {
            default_chunk_size: 64 * 1024, // 64 KB
        }
    }
}
```

**Why 64 KB default (not 4 KB like AgentFS or 512 KB like D1):**

- Local SQLite has no row-size limit like D1's 2 MB cap — no need to stay small
- 4 KB (AgentFS) creates too many rows for multi-MB files, hurting INSERT performance
- 512 KB (D1) is tuned for minimizing HTTP round-trips — irrelevant for local access
- 64 KB balances row count vs. memory overhead: a 10 MB file = ~160 chunks, a 100 MB file = ~1600 chunks
- SQLite's page size is 4 KB by default; a 64 KB chunk spans 16 pages, which SQLite handles efficiently
- The chunk size is stored per-file in `sqlite_dentry.chunk_size`, so changing the default doesn't break existing files

### Write Path

```
1. Split input data into N chunks of chunk_size bytes (last chunk may be smaller)
2. Compute blake3 checksum over the entire content
3. In a single transaction:
   a. DELETE FROM sqlite_chunks WHERE ino = ?  (clear old chunks)
   b. For each chunk:
      INSERT INTO sqlite_chunks (ino, chunk_idx, data) VALUES (?, ?, ?)
   c. UPDATE sqlite_dentry SET size = ?, checksum = ?, updated_at = ?, chunk_size = ? WHERE ino = ?
4. COMMIT
```

All chunk inserts happen in one transaction — no partial writes visible to readers.

### Read Path

```
1. Full read:
   SELECT data FROM sqlite_chunks WHERE ino = ? ORDER BY chunk_idx;
   → Concatenate all chunk blobs into Vec<u8>

2. Partial read (offset + length):
   -- Calculate chunk range:
   --   start_chunk = offset / chunk_size
   --   end_chunk   = (offset + length - 1) / chunk_size
   SELECT chunk_idx, data FROM sqlite_chunks
   WHERE ino = ? AND chunk_idx BETWEEN ? AND ?
   ORDER BY chunk_idx;
   → Concatenate, then slice [offset_within_first_chunk .. offset_within_first_chunk + length]
```

### Offset-based read_at / write_at

`VfsFile::read_at(buf, offset)` and `write_at(buf, offset)` map to chunk-range queries:

- **read_at**: Compute affected chunk range, fetch those chunks, copy the relevant byte range into `buf`
- **write_at**: Fetch affected chunks, splice new data into the byte stream, re-chunk and write back the affected chunks only. Unchanged chunks are not touched.

For `write_at` that extends the file, new chunks are appended and `size` is updated.

### Truncate

```sql
BEGIN;
-- Calculate the last chunk index to keep:
--   last_chunk = (new_size - 1) / chunk_size  (or -1 if new_size = 0)
-- Delete chunks beyond that:
DELETE FROM sqlite_chunks WHERE ino = ? AND chunk_idx > ?;
-- If new_size doesn't align to chunk boundary, fetch+truncate the last chunk:
UPDATE sqlite_chunks SET data = SUBSTR(data, 1, ?) WHERE ino = ? AND chunk_idx = ?;
-- Update dentry size:
UPDATE sqlite_dentry SET size = ?, updated_at = ? WHERE ino = ?;
COMMIT;
```

## File Handle Types

### SqliteFile (implements VfsFile)

```rust
pub struct SqliteFile {
    db: Arc<Mutex<libsql::Connection>>,
    ino: i64,
    mode: OpenMode,
    chunk_size: usize,
}

impl VfsFile for SqliteFile {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize> { ... }
    fn write_at(&self, buf: &[u8], offset: u64) -> VfsResult<usize> { ... }
    fn sync_data(&self) -> VfsResult<()> { ... }  // no-op, writes are immediately durable
    fn size(&self) -> VfsResult<u64> { ... }
    fn truncate(&self, size: u64) -> VfsResult<()> { ... }
    fn metadata(&self) -> VfsResult<VfsMetadata> { ... }
}
```

- Each `read_at`/`write_at` acquires the mutex, executes SQL, releases
- `sync_data()` is a no-op because SQLite transactions are durable on commit
- `metadata()` reads from `sqlite_dentry` for the file's ino

### SeekableSqliteFile (implements SeekableVfsFile)

```rust
pub struct SeekableSqliteFile {
    inner: SqliteFile,
    cursor: u64,
}

impl VfsFile for SeekableSqliteFile {
    // Delegates to inner, using self.cursor as offset
    fn read_at(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize> {
        self.inner.read_at(buf, offset)
    }
    // ... other VfsFile methods delegate to inner
}

impl SeekableVfsFile for SeekableSqliteFile {
    fn read(&mut self, buf: &mut [u8]) -> VfsResult<usize> {
        let n = self.inner.read_at(buf, self.cursor)?;
        self.cursor += n as u64;
        Ok(n)
    }

    fn write(&mut self, buf: &[u8]) -> VfsResult<usize> {
        let n = self.inner.write_at(buf, self.cursor)?;
        self.cursor += n as u64;
        Ok(n)
    }

    fn seek(&mut self, pos: SeekFrom) -> VfsResult<u64> {
        self.cursor = match pos {
            SeekFrom::Start(n) => n,
            SeekFrom::End(n) => (self.inner.size()? as i64 + n) as u64,
            SeekFrom::Current(n) => (self.cursor as i64 + n) as u64,
        };
        Ok(self.cursor)
    }

    fn position(&self) -> u64 {
        self.cursor
    }
}
```

### SqliteDirectory (implements VfsDirectory)

```rust
pub struct SqliteDirectory {
    db: Arc<Mutex<libsql::Connection>>,
    ino: i64,
    path: String,
}
```

Implements `VfsDirectory` by delegating to SQL queries scoped to `parent_ino = self.ino`.

## Concurrency Model

### WAL Mode

The database is opened in WAL (Write-Ahead Log) mode:

```sql
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;    -- safe with WAL, faster than FULL
PRAGMA foreign_keys = ON;       -- enforce CASCADE deletes
PRAGMA busy_timeout = 5000;     -- 5s retry on lock contention
```

WAL mode allows concurrent readers while a single writer holds the lock. This is ideal for the overlay VFS pattern: the overlay reads from the delta while occasionally writing.

### Connection Strategy

```rust
pub struct SqliteDelta {
    conn: Arc<Mutex<libsql::Connection>>,
    chunk_config: ChunkConfig,
}
```

Single connection wrapped in `Arc<Mutex<...>>`:

- **Why not connection pool**: SQLite is an embedded database with a single-writer model. Multiple connections add complexity (lock contention, WAL checkpoint coordination) without throughput benefit for the overlay use case.
- **Why `Arc<Mutex<>>`**: The `VfsFileSystem` trait requires `Send + Sync`. The mutex serializes writes while allowing the struct to be shared across threads. File handles hold a clone of the `Arc` and acquire the mutex per operation.
- **Future**: If profiling shows mutex contention, can switch to `tokio::sync::RwLock` or a read-connection pool with a dedicated write connection.

## Constructors

```rust
impl SqliteDelta {
    /// Open or create a database at the given path.
    /// Runs migrations on first open. Enables WAL mode.
    pub fn new(db_path: impl AsRef<Path>) -> VfsResult<Self> {
        let db = libsql::Database::open(db_path.as_ref().to_str().unwrap())
            .map_err(|e| /* wrap in VfsError::Io */)?;
        let conn = db.connect()
            .map_err(|e| /* wrap in VfsError::Io */)?;
        let delta = Self {
            conn: Arc::new(Mutex::new(conn)),
            chunk_config: ChunkConfig::default(),
        };
        delta.run_migrations()?;
        delta.enable_wal()?;
        Ok(delta)
    }

    /// Create an in-memory database. Useful for testing.
    /// Data is lost when the struct is dropped.
    pub fn in_memory() -> VfsResult<Self> {
        let db = libsql::Database::open(":memory:")
            .map_err(|e| /* wrap in VfsError::Io */)?;
        let conn = db.connect()
            .map_err(|e| /* wrap in VfsError::Io */)?;
        let delta = Self {
            conn: Arc::new(Mutex::new(conn)),
            chunk_config: ChunkConfig::default(),
        };
        delta.run_migrations()?;
        // WAL mode not applicable to :memory:
        Ok(delta)
    }

    /// Open with custom chunk configuration.
    pub fn with_config(db_path: impl AsRef<Path>, config: ChunkConfig) -> VfsResult<Self> {
        let mut delta = Self::new(db_path)?;
        delta.chunk_config = config;
        Ok(delta)
    }

    /// Run schema migrations. Idempotent (CREATE IF NOT EXISTS).
    fn run_migrations(&self) -> VfsResult<()> { ... }

    /// Enable WAL mode and set pragmas.
    fn enable_wal(&self) -> VfsResult<()> { ... }
}
```

## DeltaStore Trait Implementation

```rust
impl DeltaStore for SqliteDelta {
    // --- Inherited from VfsFileSystem (all methods above) ---

    fn add_whiteout(&self, path: &str, version: u64) -> VfsResult<()> {
        let version_id = pack_version(version); // u64 → Scru128Id → BLOB(16)
        // INSERT OR REPLACE INTO sqlite_whiteouts (path, version_id) VALUES (?, ?)
    }

    fn is_whiteout(&self, path: &str) -> VfsResult<Option<u64>> {
        // SELECT version_id FROM sqlite_whiteouts WHERE path = ?
        // BLOB(16) → Scru128Id::from_bytes() → unpack_version(id) → Some(u64)
    }

    fn remove_whiteout(&self, path: &str) -> VfsResult<()> {
        // DELETE FROM sqlite_whiteouts WHERE path = ?
    }

    fn list_whiteouts(&self, dir: &str) -> VfsResult<Vec<(String, u64)>> {
        // SELECT path, version FROM sqlite_whiteouts WHERE path LIKE ? || '/%'
        // Returns all whiteouts under the given directory prefix
    }

    fn flush(&self) -> VfsResult<()> {
        // PRAGMA wal_checkpoint(TRUNCATE);
        // Forces WAL to be written back to the main database file.
        // For in-memory databases, this is a no-op.
    }

    fn reset(&self) -> VfsResult<()> {
        // BEGIN;
        // DELETE FROM sqlite_chunks;
        // DELETE FROM sqlite_whiteouts;
        // DELETE FROM sqlite_dentry WHERE ino != 1;  -- keep root
        // UPDATE sqlite_dentry SET version = 0, updated_at = ? WHERE ino = 1;
        // COMMIT;
        // Clears all data, leaving only the root directory.
    }
}
```

### Whiteout path matching for `list_whiteouts`

```sql
-- For dir = "/foo":
SELECT path, version FROM sqlite_whiteouts
WHERE path LIKE '/foo/%'
ORDER BY path;
```

The `LIKE` pattern uses the directory prefix + `/%` to find all whiteouts under that directory. The index on `sqlite_whiteouts.path` makes this efficient.

## SCRU128 Version Tracking

Replace plain `version INTEGER` with SCRU128 IDs stored as `BLOB(16)`. This gives versions time-ordering (ms-precision timestamp embedded), monotonicity (counter fields), and node entropy (32-bit random) — useful for future replication scenarios with Turso embedded replicas.

### Schema Changes

```sql
-- sqlite_dentry: version column changes type
ALTER TABLE sqlite_dentry ADD COLUMN version_id BLOB(16);
-- Migration: populate from old INTEGER version using pack(timestamp=now, counter=old_version, node=0)

-- sqlite_whiteouts: version column changes type
ALTER TABLE sqlite_whiteouts ADD COLUMN version_id BLOB(16);
```

The `version_id BLOB(16)` stores raw SCRU128 bytes in big-endian order. Byte-level comparison = chronological ordering, so `ORDER BY version_id` and range queries work directly.

### Version Generation

```rust
use scru128::Scru128Id;

fn next_version(&self) -> Scru128Id {
    scru128::new() // thread-local generator, guaranteed monotonic
}
```

Every file create, write, rename, chmod, or symlink operation generates a new SCRU128 version. The version is stored in the dentry row and can be compared chronologically via byte ordering.

### SCRU128 ↔ u64 Bridge (DeltaStore Trait Compatibility)

The `DeltaStore` trait uses `version: u64` and `VfsMetadata` exposes `version: u64`. Internally we use `Scru128Id`. Bridge functions:

```rust
/// Pack overlay's u64 version into a Scru128Id for storage.
fn pack_version(overlay_version: u64) -> Scru128Id {
    let ts = scru128::timestamp();
    let counter = (overlay_version & 0xFF_FFFF) as u32;
    scru128::new_with_components(ts, counter, 0)
}

/// Extract u64 from Scru128Id, preserving ordering.
fn unpack_version(id: Scru128Id) -> u64 {
    let ts = id.timestamp() as u64;
    let counter = id.counter() as u64;
    (ts << 24) | counter
}
```

**DeltaStore whiteout round-trip:** `add_whiteout(path, version: 42)` → `pack_version(42)` → stored as `BLOB(16)`. `is_whiteout(path)` → read `BLOB(16)` → `unpack_version(id)` → returns u64.

**VfsMetadata mapping:** `SqliteDentry.version_id` (BLOB) → `Scru128Id::from_bytes()` → `unpack_version(id)` → `VfsMetadata.version` (u64).

**Ordering guarantee:** `a < b` as SCRU128 bytes → `unpack(a) < unpack(b)` as u64.

### Query Patterns

```sql
-- "What changed since version X?" (range scan on SCRU128 bytes)
SELECT * FROM sqlite_dentry WHERE version_id > ? ORDER BY version_id;

-- "When was this file last modified?" (timestamp extracted from SCRU128)
-- Done in Rust by unpacking the version_id bytes
```

### Dependency

```toml
scru128 = "0.10"  # or latest — added to vfs-sqlite feature deps
```

## Hierarchical Whiteout Prefix Index

Replace `LIKE '/foo/%'` queries in `list_whiteouts` with exact prefix matches using hierarchical prefix entries — the xs indexing pattern adapted for SQL.

### New Table

```sql
CREATE TABLE IF NOT EXISTS sqlite_whiteout_prefixes (
    prefix      TEXT NOT NULL,     -- hierarchical prefix (e.g., "/foo/bar/", "/foo/", "/")
    path        TEXT NOT NULL,     -- the actual whiteout path
    version_id  BLOB(16) NOT NULL, -- SCRU128 version when whiteout was created
    PRIMARY KEY (prefix, path)
);

CREATE INDEX IF NOT EXISTS idx_whiteout_prefix ON sqlite_whiteout_prefixes(prefix);
```

### Write Path (O(d) entries per whiteout)

For `add_whiteout("/src/lib/utils.rs", version)`, insert d+1 entries:

```
prefix = "/src/lib/utils.rs"  path = "/src/lib/utils.rs"  (exact match)
prefix = "/src/lib/"           path = "/src/lib/utils.rs"  (depth-2)
prefix = "/src/"               path = "/src/lib/utils.rs"  (depth-1)
prefix = "/"                   path = "/src/lib/utils.rs"  (root)
```

### Query Replacement

```sql
-- OLD (feature 06 base): uses LIKE, index partially effective
SELECT path, version FROM sqlite_whiteouts WHERE path LIKE '/src/lib/' || '%';

-- NEW: exact equality match, fully indexed
SELECT path, version_id FROM sqlite_whiteout_prefixes WHERE prefix = '/src/lib/';
```

`is_whiteout(path)` remains a direct lookup on the `sqlite_whiteouts` table (exact match by path). The prefix table is only used by `list_whiteouts`.

### Cleanup on `remove_whiteout`

Removing a whiteout deletes all its prefix entries:

```sql
DELETE FROM sqlite_whiteout_prefixes WHERE path = ?;
DELETE FROM sqlite_whiteouts WHERE path = ?;
```

## VFS Changelog Table

Optional SCRU128-keyed journal of all VFS mutations. Provides a time-ordered audit trail and enables "what changed since X?" queries — foundation for future sync/replication with Turso embedded replicas.

### Schema

```sql
CREATE TABLE IF NOT EXISTS sqlite_vfs_changelog (
    id          BLOB(16) PRIMARY KEY,  -- SCRU128 ID (time-ordered)
    operation   TEXT NOT NULL,          -- 'create', 'write', 'remove', 'rename', 'mkdir',
                                       -- 'rmdir', 'chmod', 'symlink', 'whiteout_add',
                                       -- 'whiteout_remove', 'reset'
    path        TEXT NOT NULL,
    old_path    TEXT,                   -- populated for 'rename' operations
    version_id  BLOB(16) NOT NULL,     -- the dentry version after this operation
    size        INTEGER,               -- file size after operation (NULL for dirs/removes)
    created_at  INTEGER NOT NULL       -- unix epoch ms (denormalized from SCRU128 for SQL convenience)
);

CREATE INDEX IF NOT EXISTS idx_changelog_path ON sqlite_vfs_changelog(path);
CREATE INDEX IF NOT EXISTS idx_changelog_created ON sqlite_vfs_changelog(created_at);
```

### Usage

```sql
-- Changes since a known point (cursor-based pagination)
SELECT * FROM sqlite_vfs_changelog WHERE id > ? ORDER BY id;

-- Changes to a specific file
SELECT * FROM sqlite_vfs_changelog WHERE path = ? ORDER BY id;

-- Changes in the last hour
SELECT * FROM sqlite_vfs_changelog WHERE created_at > ? ORDER BY id;
```

### Configuration

Changelog is opt-in via `SqliteDelta` constructor:

```rust
pub struct SqliteDeltaConfig {
    pub chunk_config: ChunkConfig,
    pub enable_changelog: bool,  // default: false
}
```

When disabled, no changelog table is created and no journal entries are written — zero overhead for callers that don't need change tracking.

## Tasks

### Schema Design (`src/shared/vfs/libsql_delta/schema.rs`)

- [x] Define SQL schema strings as constants (dentry, chunks, whiteouts, meta tables)
- [x] Implement `run_migrations()` — CREATE IF NOT EXISTS all tables, insert root dentry
- [x] Implement schema version check and upgrade path via `sqlite_vfs_meta` table
- [x] Define indexes (parent_ino, chunks ino, whiteouts path)

### Types (`src/shared/vfs/libsql_delta/types.rs`)

- [x] Define `SqliteDentry` struct (Rust-side representation of a dentry row)
- [x] Define `SqliteChunkRef` struct (ino + chunk_idx + data reference)
- [x] Define `ChunkConfig` struct with default 64 KB chunk size
- [x] Implement row-to-struct mapping helpers for libsql `Row` → `SqliteDentry`
- [x] SCRU128 version tracking via `pack_version`/`unpack_version`

### Path Resolution (`src/shared/vfs/libsql_delta/path_resolve.rs`)

- [x] Implement `resolve_path_async(conn, path)` — walk components, return ino
- [x] Implement `resolve_parent_async(conn, path)` — return parent ino + leaf name
- [x] Implement recursive CTE subtree query `subtree_inos_async` for `remove_all`

### Chunking (`src/shared/vfs/libsql_delta/chunking.rs`)

- [x] Implement `split_into_chunks(data, chunk_size)` — chunk splitting
- [x] Implement chunk read/write assembly via `read_chunk_range_async`
- [x] Implement `write_all_chunks_async` — transactional chunk write
- [x] Implement `truncate_file_async` — partial chunk truncation

### File Handles (`src/shared/vfs/libsql_delta/file_handle.rs`)

- [x] Implement `SqliteFile` with async `AsyncVfsFile` trait (read_at, write_at, sync_data, size, truncate, metadata)
- [x] Implement `SeekableSqliteFile` with `Arc<AtomicU64>` cursor, implements `AsyncSeekableVfsFile`
- [x] Implement `SqliteDirectory` with async `AsyncVfsDirectory` trait
- [x] Implement `SyncSeekableSqliteFile` with `Arc<AtomicU64>` cursor for sync API (see Feature 21)
- [x] Implement `SyncSqliteDirectory` for sync API delegation

### Core (`src/shared/vfs/libsql_delta/mod.rs`)

- [x] Define `LibsqlDelta` struct: `conn: Arc<Connection>`, `chunk_config: ChunkConfig`
- [x] Implement `LibsqlDelta::new(db_path)` — open database, run migrations, enable WAL
- [x] Implement `LibsqlDeltaConfig` with optional Turso remote config
- [x] Implement `AsyncVfsFileSystem` for `LibsqlDelta`: all trait methods mapped to SQL
- [x] Implement `AsyncDeltaStore` for `LibsqlDelta`: whiteout CRUD, flush (WAL checkpoint), reset (DELETE all)
- [x] Implement `capabilities()` returning persistent + seekable + symlinks
- [x] Implement checksum computation (blake3) on file write, stored in dentry
- [x] Implement hierarchical whiteout prefix index (`sqlite_whiteout_prefixes` table)
- [x] Implement `SyncLibsqlDelta` struct wrapping `SyncFs<LibsqlDelta>` with custom seekable
- [x] Implement `SyncFs::inner()` accessor for delegation
- [x] Implement `LibsqlDelta::into_sync()` returning `SyncLibsqlDelta`

### Feature Gating

- [x] Add `vfs-sqlite = ["vfs", "dep:libsql", "dep:scru128", "dep:blake3"]` feature flag to `Cargo.toml`
- [x] Gate module with `#[cfg(feature = "vfs-sqlite")]`
- [x] Wire into `src/shared/vfs/mod.rs` with conditional re-export

### Remaining Work

- [ ] Integration tests — `LibsqlDelta::new()` creates database, CRUD operations, whiteouts, overlay
- [ ] SCRU128 version tracking tests — monotonicity, pack/unpack round-trip
- [ ] Hierarchical whiteout prefix tests — exact prefix match, cleanup on remove
- [ ] Changelog table — optional `sqlite_vfs_changelog` table (deferred, opt-in feature)

## Iron Rule: Valtron-Backed Tests Required

**This feature uses valtron through `SyncLibsqlDelta` → `SyncFs<LibsqlDelta>` → `exec_async`. All sync-bridge code paths MUST be tested through a valtron-initialized pool.**

Direct `LibsqlDelta` async method calls bypass the bridge entirely. Tests must call `SyncLibsqlDelta` methods with `initialize_pool()` first — see `backends/foundation_nativeapis/tests/valtron_executor_integration.rs`:

```rust
fn init_pool() -> PoolGuard { initialize_pool(42, Some(3)) }

#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn test_sync_libsql() {
    let _guard = init_pool();
    let sync = SyncLibsqlDelta::new(LibsqlDelta::new("/tmp/test.db").unwrap());
    sync.mkdir("/src").unwrap();  // goes through valtron's executor
}
```

**Blocked by Feature 22:** No further work until Feature 22 (VFS Valtron Tests) tests `SyncLibsqlDelta` through valtron.

## Feature Flags

```toml
[features]
vfs-sqlite = ["vfs", "dep:libsql", "dep:scru128", "dep:blake3"]
```

## Verification

- [x] `cargo check -p foundation_nativeapis --features vfs-sqlite` compiles cleanly
- [x] Schema matches the specification (dentry, chunks, whiteouts tables exist with correct columns)
- [x] WAL mode is active (verified via `PRAGMA journal_mode`)
- [ ] All tests pass (no integration tests yet)
- [ ] Database file created by `LibsqlDelta::new()` is inspectable with `sqlite3` CLI
- [ ] `OverlayFileSystem<MemoryFs, LibsqlDelta>` integration test passes

## References
- WAL mode is active (verified via `PRAGMA journal_mode`)
- File written via VfsFileSystem is readable with identical content
- Multi-chunk file reassembled correctly
- Whiteout hides file from overlay, reset restores visibility
- `OverlayFileSystem<MemoryFs, SqliteDelta>` integration test passes

## References

- Feature 01 (core-traits) — VfsFileSystem and DeltaStore trait definitions
- Feature 13 (cloudflare-d1-delta) — sister feature with analogous schema for edge SQLite
- [libsql crate](https://crates.io/crates/libsql) — Turso's SQLite fork
- [SQLite WAL mode](https://www.sqlite.org/wal.html) — write-ahead logging documentation


## Global Rule: `foundation_errstacks` Error Handling

All VFS types MUST implement `Debug` and use `VfsResult<T>` (`Result<T, ErrorTrace<VfsError>>`) for errors. Tests use `err.current_context()` for typed error matching. See **plan.md §4d** for the full rule.

## Implementation Learnings

### libsql `Statement::execute()` Silently Drops Rows on Reuse

**Bug:** When a prepared `Statement` is reused across multiple `execute()` calls in a loop, libsql silently drops all but the first row. `execute()` returns `rows_affected=1` for each call, but only the first INSERT persists. This was discovered in three locations:

1. `write_all_chunks_async` — only first chunk persisted, causing data corruption for files >64KB
2. `write_at_async` (SqliteFile) — same chunk insert loop, only first chunk saved
3. `add_whiteout_async` — only first prefix inserted, breaking `list_whiteouts`

**Fix:** Use dynamic multi-row INSERT with `Vec<libsql::Value>` positional params:
```rust
// GOOD — single round-trip, all rows persist
let placeholders = chunks.iter().map(|_| "(?, ?, ?)").collect::<Vec<_>>().join(", ");
let sql = format!("INSERT INTO vfs_chunks (ino, chunk_idx, data) VALUES {}", placeholders);
let mut values: Vec<libsql::Value> = Vec::with_capacity(chunks.len() * 3);
for (idx, chunk) in chunks.iter().enumerate() {
    values.push(ino.into());
    values.push((idx as i64).into());
    values.push(chunk.to_vec().into());
}
conn.execute(&sql, values).await?;
```

**Why `Connection::execute()` works but `Statement::execute()` doesn't:** `Connection::execute()` internally prepares, executes, and finalizes the statement per call. `Statement::execute()` reuses the prepared handle — libsql's async implementation has a bug where the statement's internal state isn't properly reset between executions.

### PRAGMA Statements Return Rows

`execute()` fails with "Execute returned rows" for PRAGMAs like `PRAGMA journal_mode = WAL` and `PRAGMA wal_checkpoint(TRUNCATE)` because they return result rows. Use `execute_batch()` instead:
```rust
conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").await?;
```

### `read_at` and `size` Must Query Current Size from DB

`SqliteFile.size` is a cached field set at open time. After `write_at_async` modifies the file, subsequent `read_at_async` calls using the stale cached size return 0 bytes. Fix: query `SELECT size FROM vfs_dentry WHERE ino = ?` in both `read_at_async` and `size_async`.

### Whiteout Prefix Trailing Slash Must Have Leading Slash

The `whiteout_prefixes()` function generates parent directory prefixes like `/src` and `/src/`. A bug produced `src/` (no leading slash) for trailing-slash variants, so the index lookup for `prefix = '/src'` never found rows with `prefix = 'src/'`. Fix: `format!("/{}/", ...)` not `format!("{}/", ...)`.

### `is_whiteout` Returns Unpacked SCRU128, Not Raw Input

`add_whiteout(path, 1)` stores the version via `pack_version(1)` → SCRU128 BLOB. `is_whiteout(path)` returns `unpack_version(id)` which is `(timestamp << 24) | counter`, not the original `1`. Tests must use `is_some()` or extract the counter component, not compare against the raw input.

### Batch Insert Pattern (All 3 Locations)

```rust
// chunking.rs::write_all_chunks_async
// file_handle.rs::write_at_async  
// mod.rs::add_whiteout_async
// All follow the same pattern:
// 1. Build "(?, ?, ?), (?, ?, ?), ..." placeholders
// 2. Build flat Vec<libsql::Value> with all params
// 3. conn.execute(&sql, values) — 1 round-trip
```

---

_Created: 2026-06-04 | Updated: 2026-06-06_
