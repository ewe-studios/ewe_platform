---
feature_name: "FjallFs / FjallDelta — LSM-tree + CAS VFS Backend"
description: "VfsFileSystem and DeltaStore backed by fjall (LSM-tree metadata with SCRU128 keys, hierarchical prefix indexing) + cacache (content-addressed blob storage). Inspired by xs project patterns. Four keyspaces: inodes, idx_path, chunks, whiteouts. Small-file inline optimization. Version-tagged chunks with lazy GC."
status: "pending"
priority: "medium"
phase: 3
created: 2026-06-05
updated: 2026-06-05
dependencies:
  - "01-core-traits"
tasks:
  completed: 0
  uncompleted: 57
  total: 57
  completion_percentage: 0%
---

# Feature 19: FjallFs / FjallDelta — LSM-tree + CAS VFS Backend

## Overview

A complete VfsFileSystem and DeltaStore backed by two storage engines:

- **fjall** — LSM-tree database for filesystem metadata (directory entries, path index, chunk references, whiteouts)
- **cacache** — Content-addressable storage for file content blobs (npm cache format, SRI hash paths, automatic deduplication)

Inspired by the **xs** project's architecture: SCRU128 time-ordered IDs as primary keys, hierarchical prefix indexing for fast path queries, and the fjall+cacache dual-engine pattern for separating metadata from content.

### Dual Identity

Like SqliteDelta (feature 06) and D1Delta (feature 13), this provides both roles:

1. **FjallFs** (VfsFileSystem) — full file CRUD, directory hierarchy, metadata, content storage. Every VfsFileSystem method maps to fjall keyspace operations + cacache blob reads/writes.
2. **FjallDelta** (DeltaStore extending VfsFileSystem) — adds whiteout keyspace, `flush()` maps to `db.persist(SyncAll)`, `reset()` clears all keyspaces.

```rust
// FjallFs is a complete VfsFileSystem — usable standalone
let fs = FjallFs::open("/tmp/my-vfs")?;
fs.mkdir("/src")?;
fs.write_file("/src/main.rs", b"fn main() {}")?;
let data = fs.read_file("/src/main.rs")?;

// Also usable as a layer in overlay
let overlay = OverlayFileSystem::new(base_fs, FjallDelta::open("/tmp/delta")?);
```

### Why fjall + cacache

| Concern | Engine | Why |
|---------|--------|-----|
| Directory entries, path index | fjall | LSM-tree: fast writes (append-only memtable), efficient prefix scans (sorted keys), bloom filters for point reads |
| File content | cacache | Content-addressed: automatic dedup (same content = same hash = stored once), SRI integrity verification, battle-tested (npm cache format) |
| Chunk references | fjall | Version-tagged chunk keys enable lazy GC without reference counting |
| Whiteouts | fjall | Dedicated keyspace with hierarchical prefix entries for fast `list_whiteouts` |

### Module Structure

```
src/shared/vfs/
    fjall_fs/
        mod.rs              # FjallFs + FjallDelta structs, constructors, trait impls
        keyspaces.rs        # Four keyspace definitions, key encoding/decoding
        config.rs           # FjallVfsConfig with bloom ratios, thresholds, engine tuning
        file_handle.rs      # FjallFile, SeekableFjallFile implementations
        directory.rs        # FjallDirectory implementation
        gc.rs               # GC worker: orphaned chunk cleanup, cacache blob sweep
        path_index.rs       # Hierarchical prefix entry generation, path resolution
        content.rs          # Inline vs chunked content logic, cacache read/write
        version.rs          # SCRU128 ↔ u64 bridge (pack_version, unpack_version)
```

## On-Disk Layout

```
<store_path>/
├── fjall/               # LSM-tree database
│   ├── inodes/          # Keyspace: SCRU128 ino → dentry metadata
│   ├── idx_path/        # Keyspace: hierarchical prefix → ino
│   ├── chunks/          # Keyspace: ino+version+idx → SRI hash
│   └── whiteouts/       # Keyspace: hierarchical prefix → empty
├── cacache/             # Content-addressable storage
│   ├── content-v2/      # Actual content blobs (SRI hash paths)
│   └── index-v5/        # cacache metadata index
└── lock                 # fjall file lock (single-process access)
```

## Four Keyspaces

### Keyspace 1: `inodes` (Primary Metadata)

Stores all filesystem entries — files, directories, symlinks.

```
Key:   16-byte SCRU128 ino (big-endian)
Value: Serialized dentry metadata (bincode)
```

**Dentry metadata structure:**

```rust
#[derive(Serialize, Deserialize)]
pub struct FjallDentry {
    pub path: String,
    pub name: String,               // last path component
    pub parent_ino: Scru128Id,      // parent directory's ino
    pub file_type: VfsFileType,     // File, Directory, Symlink
    pub size: u64,
    pub permissions: u32,
    pub owner_uid: u32,
    pub owner_gid: u32,
    pub checksum: Option<[u8; 32]>, // blake3, None for dirs
    pub version: Scru128Id,         // SCRU128 version (time-ordered)
    pub created_at: u64,            // unix epoch ms
    pub updated_at: u64,            // unix epoch ms
    pub symlink_target: Option<String>,
    pub content_mode: ContentMode,
}

#[derive(Serialize, Deserialize)]
pub enum ContentMode {
    Empty,
    Inline(Vec<u8>),                        // content <= inline_threshold (default 4KB)
    Chunked { chunk_size: u32, chunk_count: u32 },  // content > inline_threshold
}
```

**Configuration:**

```rust
KeyspaceCreateOptions::default()
    .max_memtable_size(config.memtable_size)
    .data_block_size_policy(BlockSizePolicy::all(config.block_size))
    .data_block_hash_ratio_policy(HashRatioPolicy::all(config.inodes_bloom_ratio))
    .expect_point_read_hits(true)  // most reads are by ino
```

- **Bloom filter: 8.0** — aggressive. Every `stat()`, `open()`, `read()` does a point read here. 0.1% false positive rate means near-zero unnecessary disk I/O.
- **Point read optimization enabled** — tunes compaction for frequent point lookups.

**SCRU128 ino properties:**
- Byte-level lexicographic comparison = chronological ordering (newer files sort after older)
- "What files were created after X?" = range scan from X to end
- Globally unique across processes (32-bit node entropy)
- Monotonic within same process (counter fields)

### Keyspace 2: `idx_path` (Hierarchical Path Index)

Secondary index mapping filesystem paths to inodes. Uses hierarchical prefix entries inspired by xs's `idx_topic` keyspace.

```
Key:   <prefix_bytes>\x00<ino_16_bytes>
Value: <ino_16_bytes>  (same as in key — clean pointer to inodes keyspace)
```

**For a file at `/src/lib/utils.rs` with ino X, four entries are written:**

```
src/lib/utils.rs\x00<X>    → X    # exact match
src/lib/\x00<X>             → X    # depth-2 prefix (parent directory)
src/\x00<X>                 → X    # depth-1 prefix
\x00<X>                     → X    # root prefix
```

**Prefix generation algorithm:**

```rust
fn path_prefixes(path: &str) -> Vec<String> {
    let normalized = path.strip_prefix('/').unwrap_or(path);
    let mut prefixes = vec![normalized.to_string()]; // exact
    let parts: Vec<&str> = normalized.split('/').collect();
    for i in 1..parts.len() {
        let prefix = parts[..i].join("/") + "/";
        prefixes.push(prefix);
    }
    prefixes.push(String::new()); // root prefix (empty string)
    prefixes
}
// "/src/lib/utils.rs" → ["src/lib/utils.rs", "src/lib/", "src/", ""]
```

**Query patterns:**

| Operation | Query | How |
|-----------|-------|-----|
| `stat("/src/lib/utils.rs")` | Prefix scan on `src/lib/utils.rs\x00` | Returns ino → point read inodes |
| `list("/src/lib")` | Prefix scan on `src/lib/\x00` | Returns all descendant inos → point read each → filter by parent_ino |
| `remove_all("/src")` | Prefix scan on `src/\x00` | Returns ALL descendants at any depth |
| `exists("/src/lib/utils.rs")` | Prefix scan on `src/lib/utils.rs\x00` | Non-empty = exists |

**Configuration:**

```rust
KeyspaceCreateOptions::default()
    .data_block_hash_ratio_policy(HashRatioPolicy::all(config.idx_path_bloom_ratio))
    .expect_point_read_hits(false)  // prefix scans, not point reads
```

- **Bloom filter: 0.0** — disabled. This keyspace is only used for prefix scans. Bloom filters don't help prefix scans (they're designed for point reads).

### Keyspace 3: `chunks` (Version-Tagged Content References)

Maps file inodes + version + chunk index to cacache SRI hashes. Only used for files with `ContentMode::Chunked`.

```
Key:   <ino_16_bytes>\x00<version_16_bytes>\x00<chunk_idx_4_bytes_be>
Value: SRI hash bytes (ssri::Integrity serialized)
```

**Example — a 200KB file (ino X, version V1, 64KB chunks = 4 chunks):**

```
<X>\x00<V1>\x00<0000>  → sha256-abc123...   # chunk 0 (64KB)
<X>\x00<V1>\x00<0001>  → sha256-def456...   # chunk 1 (64KB)
<X>\x00<V1>\x00<0002>  → sha256-789ghi...   # chunk 2 (64KB)
<X>\x00<V1>\x00<0003>  → sha256-jkl012...   # chunk 3 (8KB, last)
```

**On file overwrite (only chunk 1 changes):**

```
<X>\x00<V2>\x00<0000>  → sha256-abc123...   # SAME hash (cacache dedup)
<X>\x00<V2>\x00<0001>  → sha256-NEWONE...   # new content, new cacache blob
<X>\x00<V2>\x00<0002>  → sha256-789ghi...   # SAME hash
<X>\x00<V2>\x00<0003>  → sha256-jkl012...   # SAME hash
```

All chunk entries are rewritten under V2 (fjall index entries — cheap). Only the actually-changed chunk writes a new cacache blob. The V1 entries become orphaned and are cleaned up by the GC worker.

**Configuration:**

```rust
KeyspaceCreateOptions::default()
    .data_block_hash_ratio_policy(HashRatioPolicy::all(config.chunks_bloom_ratio))
    .expect_point_read_hits(false)  // mostly prefix scans, some point reads for read_at
```

- **Bloom filter: 2.0** — moderate. `read_at(offset)` computes the exact chunk index and does a point read. Random access (seek + read patterns) is common in VFS, so moderate bloom catches those. Sequential reads use prefix scans where bloom is unused.

### Keyspace 4: `whiteouts` (DeltaStore Extension)

Tracks deleted/hidden paths for overlay semantics. Uses hierarchical prefix entries for fast `list_whiteouts`.

```
Key:   <prefix_bytes>\x00<version_16_bytes>
Value: (empty)
```

**For `add_whiteout("/src/lib/utils.rs", V)`, four entries are written:**

```
src/lib/utils.rs\x00<V>  → (empty)   # exact match
src/lib/\x00<V>           → (empty)   # depth-2 prefix
src/\x00<V>               → (empty)   # depth-1 prefix
\x00<V>                    → (empty)   # root prefix
```

**Query patterns:**

| Operation | Query |
|-----------|-------|
| `is_whiteout("/src/lib/utils.rs")` | Prefix scan on `src/lib/utils.rs\x00` — non-empty = whiteout exists |
| `list_whiteouts("/src/lib")` | Prefix scan on `src/lib/\x00` — all whiteouts under directory |
| `remove_whiteout("/src/lib/utils.rs")` | Delete all entries matching `src/lib/utils.rs\x00*` prefix + prefix entries |

**Configuration:**

```rust
KeyspaceCreateOptions::default()
    .data_block_hash_ratio_policy(HashRatioPolicy::all(config.whiteouts_bloom_ratio))
    .expect_point_read_hits(false)  // prefix scans dominate
```

- **Bloom filter: 0.0** — disabled. `is_whiteout` and `list_whiteouts` are prefix scans. No point reads.

## Content Storage: Inline vs Chunked

### Decision Flow

```
write_file(path, data):
    if data.len() <= config.inline_threshold (default 4KB):
        → Store in inode value directly (ContentMode::Inline)
        → No cacache entry, no chunk entries
        → Single fjall point read = entire file

    else:
        → Split into chunks of config.chunk_size (default 64KB)
        → Write each chunk to cacache → get SRI hash
        → Write chunk entries to fjall chunks keyspace
        → Store ContentMode::Chunked { chunk_size, chunk_count } in inode
```

### Inline Content (Small Files)

The vast majority of files in typical filesystems are small — config files, source code, manifests, metadata. These resolve in a single point read from the inodes keyspace:

```rust
// Reading a small file
let dentry = self.get_inode(ino)?;  // single fjall point read
match &dentry.content_mode {
    ContentMode::Inline(bytes) => Ok(bytes.clone()),  // done, no second hop
    ContentMode::Chunked { .. } => self.read_chunks(ino, &dentry), // cacache path
    ContentMode::Empty => Ok(vec![]),
}
```

### Chunked Content (Large Files)

**Write path:**

```rust
fn write_chunked(&self, ino: Scru128Id, data: &[u8], old_version: Scru128Id) -> VfsResult<Scru128Id> {
    let new_version = scru128::new();

    // 1. Write each chunk to cacache, collect SRI hashes
    let chunk_hashes: Vec<ssri::Integrity> = data
        .chunks(self.config.chunk_size)
        .map(|chunk| {
            let mut writer = self.cas_writer();
            writer.write_all(chunk)?;
            writer.commit()  // returns SRI hash
        })
        .collect()?;

    // 2. Write chunk entries to fjall (all under new_version)
    let batch = self.db.batch();
    for (idx, hash) in chunk_hashes.iter().enumerate() {
        let key = chunk_key(ino, new_version, idx as u32);
        batch.insert(&self.chunks, key, hash.to_bytes());
    }

    // 3. Update inode with new version + content mode
    let mut dentry = self.get_inode(ino)?;
    dentry.version = new_version;
    dentry.content_mode = ContentMode::Chunked {
        chunk_size: self.config.chunk_size as u32,
        chunk_count: chunk_hashes.len() as u32,
    };
    dentry.size = data.len() as u64;
    dentry.updated_at = now_ms();
    batch.insert(&self.inodes, ino.as_bytes(), bincode::serialize(&dentry)?);

    // 4. Commit batch + emit GC message
    batch.commit()?;
    self.db.persist(PersistMode::SyncAll)?;
    self.gc_tx.send(GCTask::CleanChunks { ino, old_version, new_version })?;

    Ok(new_version)
}
```

**Read path (read_at):**

```rust
fn read_at_chunked(&self, ino: Scru128Id, version: Scru128Id,
                    chunk_size: u32, buf: &mut [u8], offset: u64) -> VfsResult<usize> {
    let start_chunk = (offset / chunk_size as u64) as u32;
    let end_chunk = ((offset + buf.len() as u64 - 1) / chunk_size as u64) as u32;

    let mut bytes_read = 0;
    for idx in start_chunk..=end_chunk {
        // Point read: exact chunk key
        let key = chunk_key(ino, version, idx);
        let hash_bytes = self.chunks.get(key)?
            .ok_or(VfsError::Io("missing chunk".into()))?;
        let hash = ssri::Integrity::from_bytes(&hash_bytes);

        // Fetch blob from cacache
        let chunk_data = cacache::read_sync(&self.cas_path, &hash)?;

        // Copy relevant portion into buf
        let chunk_start = if idx == start_chunk { (offset % chunk_size as u64) as usize } else { 0 };
        let chunk_end = if idx == end_chunk {
            ((offset + buf.len() as u64 - 1) % chunk_size as u64) as usize + 1
        } else {
            chunk_size as usize
        };
        let len = chunk_end - chunk_start;
        buf[bytes_read..bytes_read + len].copy_from_slice(&chunk_data[chunk_start..chunk_end]);
        bytes_read += len;
    }
    Ok(bytes_read)
}
```

## GC Worker

A dedicated background thread processes cleanup tasks, following the xs GC worker pattern.

```rust
pub enum GCTask {
    CleanChunks {
        ino: Scru128Id,
        old_version: Scru128Id,
        new_version: Scru128Id,
    },
    RemoveInode(Scru128Id),
    Drain(oneshot::Sender<()>),
}
```

### CleanChunks

Triggered by file overwrite. The write path already knows both the old and new version — no scanning needed on the hot path.

```
GC receives: { ino, old_version: V1, new_version: V2 }

1. Prefix scan chunks: <ino>\x00<V1>\x00 → collect all V1 SRI hashes
2. Delete those fjall chunk entries
3. For each V1 SRI hash:
   a. Check if the same hash appears in ANY chunk entry (any ino, any version)
   b. If unreferenced → cacache::remove(hash) to reclaim disk
```

Step 3b is the expensive part — it requires checking if a hash is still referenced. This can be done via a reverse index (hash → ref count) or a lazy sweep. Since this runs in a background thread, it never blocks the write path.

### RemoveInode

Triggered by file/directory deletion. Cleans up:
1. All chunk entries for the ino (all versions)
2. All idx_path prefix entries pointing to the ino
3. Unreferenced cacache blobs
4. The inode entry itself

### Drain

Used for graceful shutdown — waits for all pending GC tasks to complete before closing the store.

## Configuration

```rust
pub struct FjallVfsConfig {
    // Bloom filter ratios (0.0 = disabled)
    pub inodes_bloom_ratio: f64,      // default: 8.0 — point-read heavy, aggressive bloom
    pub idx_path_bloom_ratio: f64,    // default: 0.0 — prefix scans only, bloom useless
    pub chunks_bloom_ratio: f64,      // default: 2.0 — moderate, helps read_at point reads
    pub whiteouts_bloom_ratio: f64,   // default: 0.0 — prefix scans dominate

    // Content storage
    pub inline_threshold: usize,      // default: 4096 (4KB) — small files inline in inode
    pub chunk_size: usize,            // default: 65536 (64KB) — balance granularity vs cacache overhead

    // fjall engine
    pub cache_size: usize,            // default: 33_554_432 (32 MiB block cache)
    pub memtable_size: usize,         // default: 8_388_608 (8 MiB per keyspace)
    pub block_size: usize,            // default: 16_384 (16 KiB blocks)
    pub worker_threads: usize,        // default: 1 (single compaction worker)
}

impl Default for FjallVfsConfig {
    fn default() -> Self {
        Self {
            inodes_bloom_ratio: 8.0,
            idx_path_bloom_ratio: 0.0,
            chunks_bloom_ratio: 2.0,
            whiteouts_bloom_ratio: 0.0,
            inline_threshold: 4096,
            chunk_size: 65536,
            cache_size: 32 * 1024 * 1024,
            memtable_size: 8 * 1024 * 1024,
            block_size: 16 * 1024,
            worker_threads: 1,
        }
    }
}
```

### Default Rationale

| Parameter | Default | Rationale |
|-----------|---------|-----------|
| `inodes_bloom_ratio` | 8.0 | Every stat/open/read hits this keyspace with a point read. 8.0 = ~0.1% false positive. Higher has diminishing returns. |
| `idx_path_bloom_ratio` | 0.0 | Prefix scans only — bloom filters don't help prefix scans, just wasted space. |
| `chunks_bloom_ratio` | 2.0 | `read_at(offset)` computes exact chunk key = point read. Random access (seek+read) is common in VFS. Moderate bloom catches those without penalizing sequential reads that use prefix scans. |
| `whiteouts_bloom_ratio` | 0.0 | `is_whiteout` is a prefix scan (`path\x00` → any version?), not a point read. `list_whiteouts` also prefix scan. |
| `inline_threshold` | 4KB | Covers configs, small scripts, metadata. Matches SQLite page size. Conservative — keeps inodes keyspace lean so bloom stays effective. |
| `chunk_size` | 64KB | A 10MB file = ~160 chunks. `read_at` for 1KB range fetches at most 2 chunks (128KB). Balances cacache filesystem overhead (inode per blob) vs read granularity. |
| `cache_size` | 32 MiB | All fjall values are metadata (file content in cacache). 32 MiB covers hot working set for most filesystems. Millions of files → bump this. |
| `memtable_size` | 8 MiB | Handles burst writes (archive extraction, bulk file creation) without excessive memory. |
| `block_size` | 16 KiB | Favors scan-heavy keyspaces (idx_path, chunks, whiteouts). Inodes' bloom filter compensates for its point-read pattern at this block size. |
| `worker_threads` | 1 | Embedded local storage, not a server. Single compaction thread sufficient. |

## FjallFs Struct and VfsFileSystem Implementation

```rust
pub struct FjallFs {
    db: Database,
    inodes: Keyspace,
    idx_path: Keyspace,
    chunks: Keyspace,
    cas_path: PathBuf,
    config: FjallVfsConfig,
    root_ino: Scru128Id,
}
```

### VfsFileSystem Method Mapping

| Method | Keyspace Operations | Content Operations |
|--------|--------------------|--------------------|
| `stat(path)` | `idx_path` prefix scan → ino → `inodes` point read | — |
| `exists(path)` | `idx_path` prefix scan → non-empty = exists | — |
| `open(path, mode)` | `idx_path` → ino → `inodes` read | Returns `FjallFile` handle |
| `open_seekable(path, mode)` | Same as `open` | Returns `SeekableFjallFile` |
| `create(path, mode)` | Generate ino → write `inodes` + O(d) `idx_path` entries | — |
| `mkdir(path)` | Generate ino → write `inodes` + O(d) `idx_path` entries | — |
| `remove(path)` | Delete `inodes` entry + all `idx_path` prefix entries | GC cleans chunks + cacache |
| `rename(from, to)` | Update `inodes` path/name/parent → rebuild `idx_path` entries | — |
| `chmod(path, mode)` | `idx_path` → ino → update `inodes` entry | — |
| `symlink(target, link)` | Generate ino → write `inodes` (with symlink_target) + O(d) `idx_path` | — |
| `readlink(path)` | `idx_path` → ino → `inodes` read → symlink_target | — |
| `open_directory(path)` | `idx_path` → ino → verify directory | Returns `FjallDirectory` |
| `capabilities()` | — | Static: `{ seekable: true, symlinks: true, persistent: true }` |

### VfsFile Methods

| Method | Operation |
|--------|-----------|
| `read_at(buf, offset)` | If inline: slice from inode content. If chunked: compute chunk range → point read each chunk key → fetch cacache blobs → assemble |
| `write_at(buf, offset)` | Generate new version → write affected chunks to cacache → rewrite ALL chunk entries under new version → update inode → emit GC task for old version |
| `sync_data()` | `db.persist(PersistMode::SyncAll)` |
| `size()` | Read from inode metadata |
| `truncate(size)` | Rewrite inode. If now below inline threshold, convert to inline. If still chunked, rewrite chunk entries with truncated last chunk. |
| `metadata()` | Read from inode metadata |

## FjallDelta Struct and DeltaStore Implementation

```rust
pub struct FjallDelta {
    inner: FjallFs,
    whiteouts: Keyspace,     // dedicated whiteout keyspace
    gc_tx: mpsc::Sender<GCTask>,
    gc_handle: Option<JoinHandle<()>>,
}

impl FjallDelta {
    pub fn open(path: impl AsRef<Path>) -> VfsResult<Self> {
        Self::open_with_config(path, FjallVfsConfig::default())
    }

    pub fn open_with_config(path: impl AsRef<Path>, config: FjallVfsConfig) -> VfsResult<Self> {
        let db = Database::builder(path.as_ref().join("fjall"))
            .cache_size(config.cache_size)
            .worker_threads(config.worker_threads)
            .open()?;

        let inodes = db.keyspace("inodes", inodes_opts(&config))?;
        let idx_path = db.keyspace("idx_path", idx_path_opts(&config))?;
        let chunks = db.keyspace("chunks", chunks_opts(&config))?;
        let whiteouts = db.keyspace("whiteouts", whiteouts_opts(&config))?;

        let cas_path = path.as_ref().join("cacache");
        let (gc_tx, gc_rx) = mpsc::channel();
        let gc_handle = spawn_gc_worker(gc_rx, db.clone(), chunks.clone(), cas_path.clone());

        let fs = FjallFs { db, inodes, idx_path, chunks, cas_path, config, root_ino };
        Ok(Self { inner: fs, whiteouts, gc_tx, gc_handle: Some(gc_handle) })
    }
}

// Delegates VfsFileSystem to inner FjallFs
impl VfsFileSystem for FjallDelta { /* delegate to self.inner */ }

impl DeltaStore for FjallDelta {
    fn add_whiteout(&self, path: &str, version: u64) -> VfsResult<()> {
        let version_id = pack_version(version); // u64 → Scru128Id
        // Write O(d) hierarchical prefix entries to whiteouts keyspace
        // persist(SyncAll)
    }

    fn is_whiteout(&self, path: &str) -> VfsResult<Option<u64>> {
        // Prefix scan on exact_path\x00 in whiteouts keyspace
        // Non-empty → extract Scru128Id from key → unpack_version(id) → return u64
    }

    fn remove_whiteout(&self, path: &str) -> VfsResult<()> {
        // Delete all entries matching exact_path\x00* in whiteouts
        // Delete all hierarchical prefix entries for this path
    }

    fn list_whiteouts(&self, dir: &str) -> VfsResult<Vec<(String, u64)>> {
        // Prefix scan on dir_prefix\x00 in whiteouts keyspace
        // Returns all whiteouts under directory (any depth)
        // Each entry: unpack_version(scru128_id) → u64
    }

    fn flush(&self) -> VfsResult<()> {
        // db.persist(PersistMode::SyncAll)
    }

    fn reset(&self) -> VfsResult<()> {
        // Clear all keyspaces except root inode
        // Drain GC worker
        // Clear cacache directory
    }
}
```

## Constructors

```rust
impl FjallFs {
    /// Open or create a FjallFs at the given path.
    pub fn open(path: impl AsRef<Path>) -> VfsResult<Self> { ... }

    /// Open with custom configuration.
    pub fn open_with_config(path: impl AsRef<Path>, config: FjallVfsConfig) -> VfsResult<Self> { ... }

    /// Create an in-memory FjallFs. Uses tmpdir for fjall + cacache.
    /// Data is lost when dropped.
    pub fn in_memory() -> VfsResult<Self> { ... }
}

impl FjallDelta {
    /// Open or create a FjallDelta at the given path.
    pub fn open(path: impl AsRef<Path>) -> VfsResult<Self> { ... }

    /// Open with custom configuration.
    pub fn open_with_config(path: impl AsRef<Path>, config: FjallVfsConfig) -> VfsResult<Self> { ... }

    /// Create an in-memory FjallDelta.
    pub fn in_memory() -> VfsResult<Self> { ... }
}
```

## SCRU128 ↔ u64 Version Bridge

The `DeltaStore` trait uses `version: u64` for whiteout versions, and `VfsMetadata` exposes `version: u64`. Internally, FjallFs uses `Scru128Id` (16 bytes) for time-ordered IDs. This section defines the bridging strategy.

### Packing: u64 → Scru128Id

When the overlay calls `add_whiteout(path, version: u64)`, FjallDelta packs the u64 into a SCRU128 ID:

```rust
use scru128::Scru128Id;

/// Pack a u64 version into a Scru128Id.
/// Uses the current timestamp for the 48-bit time component,
/// the u64 counter for the 24-bit counter field (lower 24 bits),
/// and a fixed node ID (0 for single-process).
/// This guarantees:
/// - Chronological ordering (timestamp dominates)
/// - Round-trip compatibility for the counter portion
pub fn pack_version(overlay_version: u64) -> Scru128Id {
    let ts = scru128::timestamp(); // current ms since epoch (48-bit)
    let counter = (overlay_version & 0xFF_FFFF) as u32; // lower 24 bits
    scru128::new_with_components(ts, counter, 0)
}
```

**Why this works:** SCRU128's 48-bit timestamp dominates ordering. Two whiteouts created at different times will always order correctly regardless of the counter. The counter field preserves the overlay's version for round-trip extraction.

### Unpacking: Scru128Id → u64

When returning version information to the overlay (e.g., `is_whiteout`, `list_whiteouts`), extract the counter + timestamp:

```rust
/// Extract a u64 version from a Scru128Id.
/// Combines the 48-bit timestamp (ms) with the 24-bit counter
/// to produce a unique u64 that preserves ordering.
pub fn unpack_version(id: Scru128Id) -> u64 {
    let ts = id.timestamp() as u64; // 48-bit ms timestamp
    let counter = id.counter() as u64; // 24-bit counter
    // Shift timestamp to upper bits so ordering is preserved in u64:
    // ts << 24 | counter
    (ts << 24) | counter
}
```

**Ordering guarantee:** If `a < b` as SCRU128 bytes, then `unpack_version(a) < unpack_version(b)` as u64. This is critical — the overlay's version comparison logic must remain correct.

### VfsMetadata.version Mapping

`FjallDentry` stores `version: Scru128Id`. When converting to `VfsMetadata`:

```rust
impl From<&FjallDentry> for VfsMetadata {
    fn from(dentry: &FjallDentry) -> Self {
        VfsMetadata {
            size: dentry.size,
            file_type: dentry.file_type,
            permissions: dentry.permissions,
            owner: (dentry.owner_uid, dentry.owner_gid),
            checksum: dentry.checksum.map(Checksum::Blake3).unwrap_or(Checksum::None),
            version: unpack_version(dentry.version), // SCRU128 → u64
            state: VfsEntryState::Ready,
            // timestamps from Scru128Id:
            created: Some(SystemTime::UNIX_EPOCH + Duration::from_millis(
                dentry.version.timestamp() as u64
            )),
            modified: Some(SystemTime::UNIX_EPOCH + Duration::from_millis(
                dentry.version.timestamp() as u64
            )),
            accessed: None, // not tracked by fjall backend
        }
    }
}
```

**Important:** The `version` field in `VfsMetadata` uses `unpack_version()`, which embeds both timestamp and counter. This means:
- Files created at different times have different versions (timestamp dominates)
- Files created in the same ms have different versions (counter differentiates)
- Version ordering matches chronological ordering

### Whiteout Round-Trip

```
Overlay calls: add_whiteout("/src/foo.rs", version: 42)
  → pack_version(42) → Scru128Id(ts=current_ms, counter=42, node=0)
  → stored in whiteouts keyspace

Overlay calls: is_whiteout("/src/foo.rs")
  → prefix scan finds Scru128Id(ts=X, counter=42, node=0)
  → unpack_version(id) → returns u64 matching version 42's ordering
```

## Async-First Implementation

Per the spec-37 plan, all VFS implementations are **async-first**. The traits shown in `traits.rs` are sync wrappers. Here's the strategy for FjallFs:

### Async Trait Definition

```rust
// Internal async trait (not in the public API)
#[async_trait]
pub trait AsyncVfsFileSystem: Send + Sync {
    async fn stat(&self, path: &str) -> VfsResult<VfsMetadata>;
    async fn exists(&self, path: &str) -> VfsResult<bool>;
    // ... all other methods as async
}
```

### Sync Wrapper via Valtron

```rust
// The public VfsFileSystem impl wraps async calls through valtron
impl VfsFileSystem for FjallFs {
    fn stat(&self, path: &str) -> VfsResult<VfsMetadata> {
        // valtron::block(self.async_stat(path))
        // In practice: the sync trait impl uses a runtime at the edge
        todo!("valtron-wrapped async_stat")
    }
    // ... other sync wrappers
}

impl AsyncVfsFileSystem for FjallFs {
    async fn stat(&self, path: &str) -> VfsResult<VfsMetadata> {
        // Actual implementation is async
        let ino = resolve_path_async(&self.idx_path, path).await?;
        let dentry = self.inodes.get_async(ino.as_bytes()).await?;
        // ...
    }
}
```

### Why This Matters for FjallFs

fjall and cacache are **synchronous** crates — they don't provide async APIs. The async wrapper doesn't make I/O async; it makes the VFS interface async-compatible so valtron can schedule it at the edge:

```rust
async fn async_stat(&self, path: &str) -> VfsResult<VfsMetadata> {
    // Blocking I/O runs on the valtron-managed thread pool
    // No tokio::spawn::blocking needed — valtron handles this
    self.stat(path) // calls synchronous fjall/cacache under the hood
}
```

**No `#[async_trait]` macro needed for fjall internals** — the async methods are thin wrappers around synchronous fjall/cacache calls. The async surface exists so valtron can compose VFS operations with other async work (IPC, network, etc).

### Implementation Guidance

Write the core logic synchronously (fjall and cacache are sync). Wrap each `VfsFileSystem` / `DeltaStore` method body in valtron's blocking executor. Do **not** introduce a tokio dependency.

## Concurrency Model

- **fjall**: Built-in file lock prevents multiple processes from opening the same store. Within a process, fjall handles concurrent reads natively (LSM snapshots). Writes are serialized by fjall's internal write lock.
- **cacache**: Thread-safe by design (atomic writes via temp file + rename). Multiple threads can read/write concurrently.
- **GC worker**: Dedicated OS thread (like xs). Receives tasks via `mpsc::channel`. Never blocks the write path.

No `Arc<Mutex<>>` wrapper needed (unlike SqliteDelta) — fjall and cacache handle their own concurrency.

## Tasks

### Version Bridge (`src/shared/vfs/fjall_fs/version.rs`)

- [ ] Implement `pack_version(u64) -> Scru128Id` — pack overlay version into SCRU128
- [ ] Implement `unpack_version(Scru128Id) -> u64` — extract u64 preserving ordering
- [ ] Implement `From<&FjallDentry> for VfsMetadata` with version unpacking
- [ ] Test: ordering guarantee — `a < b` as bytes → `unpack(a) < unpack(b)` as u64
- [ ] Test: round-trip — `unpack_version(pack_version(v))` preserves relative ordering with other versions

### Config (`src/shared/vfs/fjall_fs/config.rs`)

- [ ] Define `FjallVfsConfig` struct with all bloom ratios, thresholds, engine tuning
- [ ] Implement `Default` with the rationale-backed defaults
- [ ] Implement builder pattern for config construction
- [ ] Validate config values (e.g., inline_threshold < chunk_size, bloom ratio >= 0.0)

### Keyspaces (`src/shared/vfs/fjall_fs/keyspaces.rs`)

- [ ] Define `FjallDentry` struct with `ContentMode` enum (Empty, Inline, Chunked)
- [ ] Implement key encoding: `inode_key(ino)`, `path_index_key(prefix, ino)`, `chunk_key(ino, version, idx)`, `whiteout_key(prefix, version)`
- [ ] Implement key decoding: extract ino from idx_path key, extract version+idx from chunk key
- [ ] Implement keyspace creation helpers with per-keyspace bloom/block configuration
- [ ] Implement root inode bootstrap (create root directory on first open)

### Path Index (`src/shared/vfs/fjall_fs/path_index.rs`)

- [ ] Implement `path_prefixes(path) -> Vec<String>` — hierarchical prefix generation
- [ ] Implement `resolve_path(idx_path, path) -> Option<Scru128Id>` — prefix scan → ino
- [ ] Implement `list_children(idx_path, inodes, dir_path) -> Vec<FjallDentry>` — prefix scan → filter direct children
- [ ] Implement `list_descendants(idx_path, dir_path) -> Vec<Scru128Id>` — prefix scan → all inos
- [ ] Implement `write_path_entries(batch, prefix_list, ino)` — O(d) batch inserts
- [ ] Implement `remove_path_entries(batch, prefix_list, ino)` — O(d) batch deletes
- [ ] Implement `rebuild_path_entries(batch, old_prefixes, new_prefixes, ino)` — for renames

### Content (`src/shared/vfs/fjall_fs/content.rs`)

- [ ] Implement inline read/write (content in inode value)
- [ ] Implement chunked write: split data → cacache write each chunk → collect SRI hashes → write chunk entries → update inode
- [ ] Implement chunked read: prefix scan chunk entries → cacache read each → assemble
- [ ] Implement `read_at` for chunked content: compute chunk range → point read specific chunks → cacache read → slice
- [ ] Implement `write_at` for chunked content: read existing chunks → splice → rewrite all chunk entries under new version → emit GC task
- [ ] Implement content mode transition: inline → chunked (file grows past threshold), chunked → inline (file shrinks below threshold)
- [ ] Implement truncate for both inline and chunked modes

### File Handles (`src/shared/vfs/fjall_fs/file_handle.rs`)

- [ ] Implement `FjallFile` struct with `read_at`, `write_at`, `sync_data`, `size`, `truncate`, `metadata`
- [ ] Implement `SeekableFjallFile` wrapping `FjallFile` with cursor tracking
- [ ] Implement `VfsFile` trait for `FjallFile`
- [ ] Implement `SeekableVfsFile` trait for `SeekableFjallFile`

### Directory (`src/shared/vfs/fjall_fs/directory.rs`)

- [ ] Implement `FjallDirectory` struct
- [ ] Implement `VfsDirectory` trait: `list()` via prefix scan + direct-children filter
- [ ] Implement `get_entry(name)` via prefix scan on `parent_path/name\x00`

### GC Worker (`src/shared/vfs/fjall_fs/gc.rs`)

- [ ] Define `GCTask` enum (CleanChunks, RemoveInode, Drain)
- [ ] Implement GC worker thread: receive tasks via mpsc channel
- [ ] Implement `CleanChunks`: prefix scan old version → delete fjall entries → check cacache hash references → remove unreferenced blobs
- [ ] Implement `RemoveInode`: delete all chunk versions + idx_path entries + cacache blobs + inode
- [ ] Implement `Drain`: process all pending tasks, signal completion via oneshot
- [ ] Implement graceful shutdown in `FjallDelta::drop`

### Core (`src/shared/vfs/fjall_fs/mod.rs`)

- [ ] Define `FjallFs` struct with db, keyspaces, cas_path, config
- [ ] Implement `FjallFs::open(path)` — open fjall + init cacache dir + bootstrap root inode
- [ ] Implement `FjallFs::in_memory()` — tmpdir-backed for tests
- [ ] Implement `VfsFileSystem` for `FjallFs`: all trait methods
- [ ] Define `FjallDelta` struct wrapping FjallFs + whiteouts keyspace + GC worker
- [ ] Implement `FjallDelta::open(path)` with GC worker spawn
- [ ] Implement `VfsFileSystem` for `FjallDelta` (delegate to inner FjallFs)
- [ ] Implement `DeltaStore` for `FjallDelta`: whiteout CRUD with hierarchical prefix entries, flush, reset
- [ ] Implement `capabilities()` returning persistent + seekable + symlinks

### Feature Gating

- [ ] Add `vfs-fjall = ["vfs", "dep:fjall", "dep:cacache", "dep:ssri", "dep:scru128"]` feature flag
- [ ] Gate module with `#[cfg(feature = "vfs-fjall")]`
- [ ] Wire into `src/shared/vfs/mod.rs` with conditional re-export

### Tests (`tests/vfs_fjall_tests.rs`)

- [ ] Test: `FjallFs::open()` creates store directory with correct layout (fjall/, cacache/)
- [ ] Test: `FjallFs::in_memory()` works without persistent path
- [ ] Test: create file → stat returns correct metadata (size=0, type=file)
- [ ] Test: write small file (< 4KB) → inline storage in inode → read back matches
- [ ] Test: write large file (> 4KB) → chunked storage in cacache → read back matches
- [ ] Test: `read_at` partial read on chunked file returns correct byte range
- [ ] Test: `write_at` mid-file update: only changed chunks get new cacache blob
- [ ] Test: file overwrite bumps version, old version chunks are GC-eligible
- [ ] Test: seekable file handle — sequential read/write with cursor tracking
- [ ] Test: mkdir → list → directory appears with correct type
- [ ] Test: nested directory creation via `mkdir_all`
- [ ] Test: remove empty directory succeeds, remove non-empty fails
- [ ] Test: `remove_all` recursively deletes via prefix scan
- [ ] Test: rename file updates idx_path entries (old removed, new created)
- [ ] Test: symlink creation and readlink
- [ ] Test: hierarchical path index — prefix scan returns correct descendants
- [ ] Test: whiteout add → is_whiteout returns version
- [ ] Test: whiteout remove → is_whiteout returns None
- [ ] Test: list_whiteouts returns all whiteouts under directory via prefix scan
- [ ] Test: reset clears all keyspaces, only root inode remains
- [ ] Test: content dedup — two files with same content share cacache blob
- [ ] Test: GC cleans orphaned chunks after file overwrite
- [ ] Test: inline → chunked transition when file grows past threshold
- [ ] Test: chunked → inline transition when file shrinks below threshold
- [ ] Test: configurable bloom ratios, chunk size, inline threshold
- [ ] Test: end-to-end with `OverlayFileSystem<MemoryFs, FjallDelta>`
- [ ] Test: concurrent reads don't block (no mutex contention)

## Iron Rule: Valtron-Backed Tests Required

**This feature will use valtron through `SyncFjallFs` → `SyncFs<FjallFs>` → `exec_async`. All sync-bridge code paths MUST be tested through a valtron-initialized pool.**

Tests must initialize the pool before calling any sync-bridge methods — see `backends/foundation_nativeapis/tests/valtron_executor_integration.rs` for the pattern.

**Blocked by Feature 22:** No work on this feature until Feature 22 (VFS Valtron Tests) tests sync bridges through valtron.

## Feature Flags

```toml
[features]
vfs-fjall = ["vfs", "dep:fjall", "dep:cacache", "dep:scru128", "dep:bincode", "dep:hex"]
```

## Dependencies

```toml
[dependencies]
fjall = { version = "2", optional = true }
cacache = { version = "13", optional = true }
scru128 = { version = "4", optional = true }
bincode = { version = "2", features = ["serde"], optional = true }
hex = { version = "0.4", optional = true }
```

## fjall v2 API Notes

fjall v2 uses a partition-based API:

```rust
// Open database
let keyspace = fjall::Config::new(path).open()?;

// Open partitions (like column families)
let inodes = keyspace.open_partition("inodes", fjall::PartitionCreateOptions::default())?;
let idx_path = keyspace.open_partition("idx_path", fjall::PartitionCreateOptions::default())?;

// Operations on partitions
inodes.insert(key, value)?;
let value = inodes.get(key)?;  // Option<Vec<u8>>
inodes.remove(key)?;

// Iteration
for item in idx_path.iter().flatten() {
    let (key, value) = item;
}

// Prefix scans
for item in idx_path.prefix(prefix).flatten() { ... }

// Durability
keyspace.persist(fjall::PersistMode::SyncAll)?;
```

## Verification

- All tests pass
- `cargo check -p foundation_nativeapis --features vfs-fjall` compiles cleanly
- Store directory created with correct layout (fjall/, cacache/ subdirectories)
- Small file (< 4KB) stored inline — single point read retrieves content
- Large file chunked into cacache — content reassembled correctly
- Identical content across files produces single cacache blob (dedup verified)
- File overwrite creates new version chunks, GC cleans old version
- Hierarchical path index enables fast directory listing via prefix scan
- Whiteout prefix entries enable `list_whiteouts` without LIKE patterns
- `OverlayFileSystem<MemoryFs, FjallDelta>` integration test passes
- No mutex/lock contention on concurrent reads

## References

- **xs project** — SCRU128 IDs, hierarchical prefix indexing, fjall+cacache dual-engine architecture
- Feature 01 (core-traits) — VfsFileSystem and DeltaStore trait definitions
- Feature 06 (sqlite-delta) — sister feature with SQL-based storage
- [fjall crate](https://crates.io/crates/fjall) — LSM-tree database
- [cacache crate](https://crates.io/crates/cacache) — Content-addressable storage
- [scru128 crate](https://crates.io/crates/scru128) — Time-ordered unique IDs
- [ssri crate](https://crates.io/crates/ssri) — Subresource Integrity hash format


## Global Rule: `foundation_errstacks` Error Handling

All VFS types MUST implement `Debug` and use `VfsResult<T>` (`Result<T, ErrorTrace<VfsError>>`) for errors. Tests use `err.current_context()` for typed error matching. See **plan.md §4d** for the full rule.

---

_Created: 2026-06-05 | Updated: 2026-06-05_
