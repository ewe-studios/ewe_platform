---
feature_name: "Cloudflare D1 DeltaStore"
description: "Cloudflare D1 (edge SQLite) as DeltaStore — 4KB chunks matching SQLite page size, two implementations: wasm-bindgen/worker-rs for Workers, native HTTP API for server-side"
status: "pending"
priority: "low"
phase: 5
created: 2026-06-04
updated: 2026-06-04
dependencies:
  - "01-core-traits"
tasks:
  completed: 0
  uncompleted: 28
  total: 28
  completion_percentage: 0%
---

# Feature 13: Cloudflare D1 DeltaStore

## Overview

This feature provides **two layers** of D1-backed filesystem:

1. **D1Fs** -- a standalone `VfsFileSystem` implementation backed by D1. Can be used as a base layer, a standalone cloud filesystem, or as a layer in `OverlayFileSystem`. All VfsFileSystem methods map to SQL queries against the `d1_dentry` / `d1_chunks` schema.
2. **D1Delta** -- extends D1Fs with `DeltaStore` trait (adds whiteout tracking + lifecycle). Used as the upper/delta layer in `OverlayFileSystem`.

D1Fs can be used standalone as a `VfsFileSystem` -- it is a complete filesystem, not just a delta store. D1Delta extends it with whiteout support for overlay use.

### D1Fs as Standalone VfsFileSystem

```rust
// D1Fs is a complete VfsFileSystem -- usable standalone
let d1fs = D1Fs::new(conn, D1FsOptions::default());
d1fs.mkdir("/projects")?;
d1fs.write_file("/projects/readme.md", b"Hello")?;
let data = d1fs.read_file("/projects/readme.md")?;

// Also usable as a base layer in overlay
let overlay = OverlayFileSystem::new(d1fs, MemoryDelta::new());
```

Files are chunked into rows -- each chunk is a row with file identity, ordering, and binary data. Batched queries eliminate N+1 reads. Two implementations share the same schema and chunking logic:

1. **wasm-bindgen/worker-rs** — runs inside Cloudflare Workers via the Workers SDK
2. **Native HTTP API** — runs anywhere, calls D1's REST API (lessons from `foundation_db`'s native HTTP implementation)

### Module Structure

```
src/shared/vfs/
    d1_delta/
        mod.rs              # D1Delta struct + common logic
        schema.rs           # D1 table schema (shared by both impls)
        chunking.rs         # Chunk sizing, ordering, batch assembly
        types.rs            # D1FileMeta, D1ChunkRef, etc.

src/native/vfs/
    d1_http.rs              # Native HTTP API implementation

wasm/wasm-bindgen/
    d1/
        mod.rs              # wasm-bindgen implementation using worker-rs bindings
```

### Schema

Takes ideas from AgentFS: separate directory hierarchy from file data, inode-like identity, `UNIQUE(parent, name)` constraint enforces no duplicate filenames.

```sql
-- Directory entries (hierarchy)
CREATE TABLE d1_dentry (
    ino         INTEGER PRIMARY KEY,    -- unique inode-like ID
    name        TEXT NOT NULL,          -- filename/dirname
    parent_ino  INTEGER NOT NULL,       -- parent directory inode
    file_type   TEXT NOT NULL,          -- 'file' | 'dir' | 'symlink'
    size        INTEGER,                -- file size (NULL for dirs)
    permissions INTEGER NOT NULL DEFAULT 0o755,
    owner_uid   INTEGER NOT NULL DEFAULT 0,
    owner_gid   INTEGER NOT NULL DEFAULT 0,
    checksum    TEXT,                   -- blake3 hex (NULL for dirs)
    version     INTEGER NOT NULL,       -- delta store version
    created_at  INTEGER NOT NULL,       -- unix epoch ms
    updated_at  INTEGER NOT NULL,       -- unix epoch ms
    symlink_target TEXT,                -- target path if file_type = 'symlink'
    UNIQUE(parent_ino, name)            -- no duplicate names in a directory
);

-- File content chunks (only for file_type = 'file')
CREATE TABLE d1_chunks (
    ino         INTEGER NOT NULL,       -- FK → d1_dentry.ino
    chunk_idx   INTEGER NOT NULL,       -- 0-based ordering
    data        BLOB NOT NULL,          -- chunk bytes (≤ chunk_size)
    PRIMARY KEY (ino, chunk_idx)
);

-- Whiteouts (from DeltaStore trait — inode-based)
CREATE TABLE d1_whiteouts (
    ino         INTEGER PRIMARY KEY,    -- whiteout'd inode (or synthetic for base-only whiteouts)
    path        TEXT NOT NULL,          -- human-readable path
    version     INTEGER NOT NULL        -- version when whiteout was created
);
```

### Path Lookup

Path resolution uses a **recursive CTE** walking `d1_dentry` from root (`ino = 1`) to the target:

```sql
WITH RECURSIVE path_lookup(name, ino, parent_ino, depth) AS (
    -- Base: root directory
    SELECT name, ino, parent_ino, 0
    FROM d1_dentry WHERE ino = 1

    UNION ALL

    -- Recursive: children of current match
    SELECT d.name, d.ino, d.parent_ino, pl.depth + 1
    FROM d1_dentry d
    JOIN path_lookup pl ON d.parent_ino = pl.ino
)
SELECT ino, file_type, size, checksum, version
FROM path_lookup
WHERE name = ? AND depth = ?    -- depth = number of path components
LIMIT 1;
```

In practice, the overlay already resolves paths layer-by-layer, so D1Delta receives individual path components and walks one level at a time: `SELECT ino FROM d1_dentry WHERE parent_ino = ? AND name = ?` — fast single-row lookup.

### Directory Listing

```sql
SELECT name, file_type, size, version
FROM d1_dentry
WHERE parent_ino = ?
ORDER BY name;
```

### Rename

```sql
UPDATE d1_dentry SET name = ?, parent_ino = ?, version = ? WHERE ino = ?;
```

No cascading updates — child entries keep their `parent_ino` unchanged. The `UNIQUE(parent_ino, name)` constraint prevents collisions atomically.

### Chunking Strategy

- **Configurable chunk size**, default: **512 KB**. D1's row/BLOB limit is **2 MB** (Cloudflare docs), so 512 KB is well under the ceiling even with row metadata overhead
- Each chunk row: `file_id` (36 bytes UUID) + `chunk_idx` (4 bytes) + `data` (≤ chunk_size) — row overhead ~40 bytes, so at 512 KB chunks we're at ~524 KB per row, safely under the 2 MB limit
- File metadata row stores `chunk_size` and `total_chunks` so any implementation knows how to reassemble
- Empty files: `total_chunks = 0`, no chunk rows
- A 10 MB file = ~20 chunks at 512 KB — easily fetched in one batched query
- Seek reads: fetch only the chunk range covering the seek window, not the whole file

### Read Path (Batched)

```
1. SELECT * FROM d1_files WHERE path = ? → get file metadata
2. If total_chunks > 0:
   SELECT data FROM d1_chunks WHERE file_id = ? ORDER BY chunk_idx
   → single batched query pulls all chunks in order
   → concatenate chunks into Vec<u8>
3. If partial read (SeekableVfsFile), fetch only needed chunks:
   SELECT data FROM d1_chunks WHERE file_id = ? AND chunk_idx BETWEEN ? AND ?
   → batched query for the chunk range covering the seek window
```

### Write Path (Batched)

```
1. Chunk file data into N pieces
2. Batch INSERT INTO d1_files (...) — single statement
3. Batch INSERT INTO d1_chunks (file_id, chunk_idx, data) VALUES (...), (...), ... — single statement with N rows
4. If updating existing file: DELETE FROM d1_chunks WHERE file_id = ? before INSERT (in same batch transaction)
```

### Two Implementations

#### wasm-bindgen/worker-rs (`wasm/wasm-bindgen/d1/`)

- Uses `worker-rs` D1 bindings (`worker::env::D1Database`, `worker::d1::*`)
- Runs inside Cloudflare Workers runtime
- WASM-compatible, no HTTP overhead

#### Native HTTP API (`src/native/vfs/d1_http.rs`)

- Calls D1's REST API (`POST /accounts/:id/d1/database/:id/query`)
- Runs anywhere (Linux, macOS, server processes)
- Takes lessons from `foundation_db`'s native HTTP implementation pattern
- Authentication via Cloudflare API token + account ID

#### Shared Code (`src/shared/vfs/d1_delta/`)

- Schema definition, chunking logic, types, and the `D1Delta` struct
- The `D1Delta` struct holds a trait-object or generic for the database connection
- Both implementations satisfy the same `DeltaStore` trait

### VfsFileSystem Method to SQL Mapping (D1Fs)

D1Fs implements the full `VfsFileSystem` trait. Each method translates to SQL queries against the `d1_dentry` and `d1_chunks` tables:

| VfsFileSystem Method | SQL Queries | Notes |
|---------------------|-------------|-------|
| `stat(path)` | `SELECT ino, file_type, size, permissions, owner_uid, owner_gid, checksum, version, created_at, updated_at FROM d1_dentry WHERE parent_ino = ? AND name = ?` (walk path components) | Returns `VfsMetadata` constructed from dentry row. Walk from root (ino=1) one component at a time. |
| `exists(path)` | Same as `stat()` | Returns `true` if `stat()` succeeds, `false` on `NotFound`. |
| `open(path, mode)` | `SELECT ino, file_type, size FROM d1_dentry WHERE parent_ino = ? AND name = ?` | Resolves path to inode. Returns `D1File` handle holding the inode. Read/write operations go through `d1_chunks`. Mode checked against permissions. |
| `open_seekable(path, mode)` | Same as `open()` | Returns `D1SeekableFile` with internal cursor position. |
| `create(path, mode)` | `INSERT INTO d1_dentry (ino, name, parent_ino, file_type, size, permissions, ...) VALUES (?, ?, ?, 'file', 0, ?, ...)` | Allocates new inode (max(ino)+1 or sequence). Parent must exist and be a directory. `UNIQUE(parent_ino, name)` enforces no duplicates -- violation returns `AlreadyExists`. |
| `mkdir(path)` | `INSERT INTO d1_dentry (ino, name, parent_ino, file_type, ...) VALUES (?, ?, ?, 'dir', ...)` | Same inode allocation. Parent must exist. |
| `remove(path)` | `DELETE FROM d1_chunks WHERE ino = ?; DELETE FROM d1_dentry WHERE ino = ?` | Both in a batch transaction. For directories, must be empty (`SELECT COUNT(*) FROM d1_dentry WHERE parent_ino = ?` check first). |
| `rename(from, to)` | `UPDATE d1_dentry SET name = ?, parent_ino = ? WHERE ino = ?` | Single row update. If target exists, remove it first (in same batch). `UNIQUE(parent_ino, name)` prevents collisions. |
| `chmod(path, mode)` | `UPDATE d1_dentry SET permissions = ?, updated_at = ? WHERE ino = ?` | Path resolved to inode first. |
| `symlink(target, link)` | `INSERT INTO d1_dentry (ino, name, parent_ino, file_type, symlink_target, ...) VALUES (?, ?, ?, 'symlink', ?, ...)` | `symlink_target` stores the target path string. |
| `readlink(path)` | `SELECT symlink_target FROM d1_dentry WHERE ino = ?` | Returns `NotAFile` if entry is not a symlink. |
| `open_directory(path)` | `SELECT ino, file_type FROM d1_dentry WHERE parent_ino = ? AND name = ?` | Returns `D1Directory` handle. `list()` on the handle runs `SELECT name, file_type FROM d1_dentry WHERE parent_ino = ? ORDER BY name`. |
| `capabilities()` | (hardcoded) | Returns `VfsCapabilities { seekable: true, symlinks: true, permissions_enforced: false, event_emission: false, persistent: true }`. |

#### D1File read_at / write_at

```
read_at(buf, offset):
    1. Calculate start_chunk = offset / chunk_size, end_chunk = (offset + buf.len()) / chunk_size
    2. SELECT data FROM d1_chunks WHERE ino = ? AND chunk_idx BETWEEN ? AND ? ORDER BY chunk_idx
    3. Concatenate chunks, slice to [offset % chunk_size .. offset % chunk_size + buf.len()]
    4. Copy into buf, return bytes read

write_at(data, offset):
    1. Calculate affected chunk range
    2. Fetch existing chunks in range (if partial chunk at start/end)
    3. Merge data into chunks
    4. Batch UPSERT: INSERT OR REPLACE INTO d1_chunks (ino, chunk_idx, data) VALUES (?, ?, ?)
    5. UPDATE d1_dentry SET size = max(size, offset + data.len()), updated_at = ?, checksum = ? WHERE ino = ?
    6. Return bytes written
```

#### D1Fs Struct

```rust
pub struct D1Fs<Conn: D1Connection> {
    conn: Conn,
    chunk_size: usize,  // default 512 KB
}

impl<Conn: D1Connection> VfsFileSystem for D1Fs<Conn> {
    type File = D1File<Conn>;
    type SeekableFile = D1SeekableFile<Conn>;
    type Directory = D1Directory<Conn>;
    // ... all methods map to SQL queries above
}
```

### DeltaStore Trait Implementation (D1Delta extends D1Fs)

D1Delta wraps D1Fs and adds whiteout tracking. It delegates all VfsFileSystem methods to the inner D1Fs and implements DeltaStore on top.

```rust
pub struct D1Delta<Conn: D1Connection> {
    inner: D1Fs<Conn>,      // full VfsFileSystem implementation
    version_counter: Arc<AtomicU64>,  // shared with OverlayFileSystem
}

impl<Conn: D1Connection> D1Delta<Conn> {
    pub fn new(conn: Conn, chunk_size: Option<usize>) -> Self {
        Self {
            inner: D1Fs::new(conn, chunk_size.unwrap_or(512 * 1024)),
            version_counter: Arc::new(AtomicU64::new(0)),
        }
    }
}

// D1Delta is a VfsFileSystem (delegates to inner D1Fs)
impl<Conn: D1Connection> VfsFileSystem for D1Delta<Conn> {
    type File = D1File<Conn>;
    type SeekableFile = D1SeekableFile<Conn>;
    type Directory = D1Directory<Conn>;
    // All methods delegate to self.inner
}

// D1Delta extends VfsFileSystem with DeltaStore
impl<Conn: D1Connection> DeltaStore for D1Delta<Conn> {
    fn add_whiteout(&self, path: &str, version: u64) -> Result<()> {
        // INSERT INTO d1_whiteouts (ino, path, version) VALUES (?, ?, ?)
    }
    fn is_whiteout(&self, path: &str) -> Result<Option<u64>> {
        // SELECT version FROM d1_whiteouts WHERE path = ?
    }
    fn remove_whiteout(&self, path: &str) -> Result<()> {
        // DELETE FROM d1_whiteouts WHERE path = ?
    }
    fn list_whiteouts(&self, dir: &str) -> Result<Vec<(String, u64)>> {
        // SELECT path, version FROM d1_whiteouts WHERE path LIKE ?||'%'
    }
    fn flush(&self) -> Result<()> { /* no-op for D1 -- writes are immediate */ }
    fn reset(&self) -> Result<()> {
        // DELETE FROM d1_chunks; DELETE FROM d1_dentry WHERE ino != 1;
        // DELETE FROM d1_whiteouts; -- keep root
    }
}
```

### D1Connection Trait (Internal)

```rust
#[async_trait]
pub trait D1Connection: Send + Sync {
    /// Execute a batch of statements in a transaction
    async fn batch(&self, statements: Vec<D1Statement>) -> Result<Vec<D1Result>>;

    /// Execute a single statement
    async fn execute(&self, statement: D1Statement) -> Result<D1Result>;

    /// Query returning rows
    async fn query(&self, statement: D1Statement) -> Result<Vec<D1Row>>;
}
```

Both the worker-rs impl and HTTP impl implement this trait. The shared `D1Delta` struct is generic over it.

## Tasks

### Shared (`src/shared/vfs/d1_delta/`)

- [ ] Define `D1Connection` trait (internal abstraction)
- [ ] Define `D1Statement`, `D1Result`, `D1Row` types
- [ ] Implement chunking logic with configurable chunk size (default 512 KB)
- [ ] Define `D1FileMeta`, `D1ChunkRef` types
- [ ] Define SQL schema and migration (first-connect check/create tables, insert root dentry ino=1)
- [ ] Implement `D1Fs<Conn>` struct with full `VfsFileSystem` trait:
  - `stat()` -- path walk via parent_ino + name lookups
  - `open()` / `open_seekable()` -- resolve inode, return D1File/D1SeekableFile
  - `create()` -- INSERT dentry + allocate inode
  - `mkdir()` -- INSERT directory dentry
  - `remove()` -- DELETE dentry + chunks (with empty-directory check)
  - `rename()` -- UPDATE dentry name/parent_ino
  - `chmod()` -- UPDATE permissions
  - `symlink()` / `readlink()` -- INSERT/SELECT with symlink_target
  - `open_directory()` -- resolve inode, return D1Directory (list = SELECT children)
- [ ] Implement `D1File<Conn>` with `read_at`/`write_at` via chunk range queries
- [ ] Implement `D1SeekableFile<Conn>` wrapping D1File with cursor position
- [ ] Implement `D1Directory<Conn>` with `list()` via `SELECT ... WHERE parent_ino = ?`
- [ ] Implement `DeltaStore` trait for `D1Delta<Conn>` (wraps D1Fs, adds whiteout operations)

### wasm-bindgen (`wasm/wasm-bindgen/d1/`)

- [ ] Create wasm module directory structure
- [ ] Implement `D1Connection` for worker-rs `D1Database`
- [ ] Handle worker-rs type conversions (JsValue → Rust types)
- [ ] Feature gate: `cfg(target_arch = "wasm32")` or feature flag

### Native HTTP (`src/native/vfs/d1_http.rs`)

- [ ] Implement `D1Connection` for HTTP client
- [ ] Cloudflare API authentication (token + account ID)
- [ ] Handle HTTP response parsing, error translation
- [ ] Feature gate: `cfg(not(target_arch = "wasm32"))` or feature flag

### Tests

- [ ] Test: D1Fs standalone -- mkdir, create, write, read, stat, remove (full VfsFileSystem surface)
- [ ] Test: D1Fs as base layer in OverlayFileSystem
- [ ] Test: write file -> read back, content matches
- [ ] Test: multi-chunk file -> single batched query reassembles correctly
- [ ] Test: partial read (seek) -> fetches only needed chunks
- [ ] Test: update file -> old chunks replaced, new content matches
- [ ] Test: whiteout -> file hidden, reset restores
- [ ] Test: batch write performance -- N chunks in single round-trip
- [ ] Test: worker-rs impl (requires mock/simulated D1)
- [ ] Test: HTTP impl (requires mock/simulated D1 API)

## Feature Flags

```toml
vfs-d1-wasm = ["vfs"]       # wasm-bindgen/worker-rs D1Delta
vfs-d1-http = ["vfs"]       # Native HTTP D1Delta (adds reqwest or similar)
```

## Verification

- D1Fs is usable as a standalone VfsFileSystem (not just as a DeltaStore)
- D1Fs is usable as a base layer in OverlayFileSystem
- File written to D1Fs/D1Delta is readable with identical content
- Multi-chunk file reassembled via single batched query
- Whiteout hides file from overlay, reset restores
- worker-rs impl compiles for wasm32 target
- HTTP impl works against real D1 (or mock)
- Batch insert of 100 chunks completes in single round-trip

## References

- `foundation_db` — existing pattern for native HTTP database implementation
- Cloudflare D1 API docs
- `worker-rs` crate — Workers SDK for Rust
