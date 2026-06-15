---
feature_name: "Cloudflare R2 DeltaStore"
description: "Cloudflare R2 (S3-compatible object storage) as DeltaStore — pluggable key layout adapter (Path, CAS, DirManifest), dual implementations (wasm-bindgen/worker-rs + native S3 HTTP)"
status: "done"
priority: "low"
phase: 5
created: 2026-06-04
updated: 2026-06-09
dependencies:
  - "01-core-traits"
tasks:
  completed: 3
  uncompleted: 27
  total: 30
  completion_percentage: 10%

## Global Rule: `foundation_errstacks` Error Handling

All VFS types MUST implement `Debug` and use `VfsResult<T>` (`Result<T, ErrorTrace<VfsError>>`) for errors. Tests use `err.current_context()` for typed error matching. See **plan.md §4d** for the full rule.

---

# Feature 14: Cloudflare R2 DeltaStore

## Overview

This feature provides **two layers** of R2-backed filesystem:

1. **R2Fs** -- a standalone `VfsFileSystem` implementation backed by R2 object storage. Can be used as a base layer, a standalone cloud filesystem, or as a layer in `OverlayFileSystem`. All VfsFileSystem methods work through the pluggable layout adapter.
2. **R2Delta** -- extends R2Fs with `DeltaStore` trait (adds whiteout tracking + lifecycle). Used as the upper/delta layer in `OverlayFileSystem`.

R2Fs can be used standalone as a `VfsFileSystem` -- it is a complete filesystem, not just a delta store. R2Delta extends it with whiteout support for overlay use.

```rust
// R2Fs is a complete VfsFileSystem -- usable standalone
let r2fs = R2Fs::new(bucket, PathKeyLayout::new(), NoMeta);
r2fs.mkdir("/uploads")?;
r2fs.write_file("/uploads/photo.jpg", &image_bytes)?;
let data = r2fs.read_file("/uploads/photo.jpg")?;

// Also usable as a base layer in overlay
let overlay = OverlayFileSystem::new(r2fs, MemoryDelta::new());
```

The key design is a **pluggable layout adapter** -- the same R2 backend supports multiple storage strategies, letting users choose what fits their use case.

### Two Implementations (Shared Layout Adapters)

| Implementation | Location | Description |
|---------------|----------|-------------|
| wasm-bindgen/worker-rs | `wasm/wasm-bindgen/r2/` | Runs inside Cloudflare Workers via `worker::r2::R2Bucket` |
| Native HTTP S3 API | `src/native/vfs/r2_s3.rs` | Runs anywhere, standard S3-compatible REST API |

### Pluggable Layout Adapters

The layout adapter determines how VFS paths map to R2 object keys, how metadata is stored, and how directory listing works. `R2Delta<Layout, MetaStore>` is generic over both.

#### 1. PathKeyLayout — keys match VFS paths

Simplest. R2 object key = VFS path with leading `/` stripped.

```
VFS path: /src/main.rs  →  R2 key: src/main.rs
VFS path: /docs/api.md  →  R2 key: docs/api.md
```

- **Pros:** human-readable, browseable in R2 console, no metadata store needed
- **Cons:** rename = copy + delete (2 API calls), no deduplication
- **Best for:** simple use cases, debugging, small overlays

Metadata comes from R2 object properties (Content-Length, ETag, custom headers).

#### 2. CASKeyLayout — content-addressed storage

Keys are content hashes. Identical content = identical key = automatic deduplication.

```
VFS path: /src/main.rs (sha256: abc123...)  →  R2 key: cas/ab/c1/abc123...
VFS path: /lib/main.rs (same content)        →  R2 key: cas/ab/c1/abc123...  (same!)
```

Requires a metadata store to map paths → CAS keys:

- **D1 metadata store** — SQLite table (`path TEXT, hash TEXT, size INT, ...`), fast lookups, supports directory listing via queries
- **R2 manifest store** — flat metadata files stored alongside CAS blobs in R2 itself:
  - Per-directory manifest: `.delta/manifests/{dir_path}.json` listing children with hashes
  - Fast: one R2 GET per directory listing, one GET per file read

- **Pros:** deduplication (same content stored once), integrity verification (hash on read)
- **Cons:** rename needs metadata update only (no blob copy), but requires metadata store round-trip
- **Best for:** large overlays with duplicate content, integrity-sensitive use cases

#### 3. DirManifestLayout — per-directory manifest files

Each directory has a manifest object in R2 listing its children with metadata. Files stored at arbitrary keys (path-based or hash-based).

```
VFS directory: /src/
  → R2 key: .delta/src/manifest.json
  → Contents: [{"name": "main.rs", "key": "blobs/x/y/main_v2.rs", "size": 1024, "checksum": "..."}, ...]

VFS file: /src/main.rs
  → R2 key: blobs/x/y/main_v2.rs
```

- **Pros:** directory listing = single R2 GET (fast, no prefix scan), metadata always available without scanning
- **Cons:** every mutation in a directory requires updating the manifest (read-modify-write)
- **Best for:** directories with many files where listing performance matters

### VfsFileSystem Method to Layout Mapping (R2Fs)

R2Fs implements the full `VfsFileSystem` trait. Each method's behavior depends on the active layout:

#### PathKeyLayout

| VfsFileSystem Method | R2 Operation | Details |
|---------------------|-------------|---------|
| `stat(path)` | `HEAD src/main.rs` | Object metadata (Content-Length, ETag, custom headers `x-vfs-type`, `x-vfs-permissions`). Directories: check if any key starts with `path/` prefix (prefix scan). |
| `exists(path)` | `HEAD src/main.rs` | Returns true if HEAD succeeds, false on 404. |
| `open(path, mode)` | (no R2 call yet) | Returns `R2File` handle. Reads/writes happen on `read_at`/`write_at`. |
| `read_at(buf, offset)` | `GET src/main.rs` with `Range: bytes=offset-offset+len` | R2 supports range requests. Full file fetch if range not supported. |
| `write_at(data, offset)` | `GET` + modify + `PUT src/main.rs` | R2 does not support partial writes. Must fetch, modify in-memory, PUT entire object. For append-only, just PUT with full content. |
| `create(path, mode)` | `PUT src/main.rs` (empty body) | Custom headers for metadata. |
| `mkdir(path)` | `PUT src/.dir-marker` (empty) | R2 has no directories -- use a marker object or rely on prefix convention. |
| `remove(path)` | `DELETE src/main.rs` | For directories: delete marker + all objects with prefix. |
| `rename(from, to)` | `COPY src/old.rs -> src/new.rs` + `DELETE src/old.rs` | R2/S3 rename = copy + delete (2 API calls). |
| `list(dir)` | `LIST prefix=src/&delimiter=/` | S3 list-objects-v2 with prefix and delimiter. Returns CommonPrefixes (subdirs) + Contents (files). |
| `chmod(path, mode)` | `COPY src/main.rs -> src/main.rs` with updated metadata | S3 metadata update = copy-to-self with new headers. |
| `symlink(target, link)` | `PUT link` with `x-vfs-type: symlink`, body = target path | Symlink stored as object with target in body. |
| `readlink(path)` | `GET path` if `x-vfs-type: symlink` | Read object body as symlink target. |

#### CASKeyLayout

| VfsFileSystem Method | R2 Operation | Details |
|---------------------|-------------|---------|
| `stat(path)` | `meta.get_entry(path)` -> `HEAD cas/ab/c1/abc123...` | Look up path in metadata store to get hash, then HEAD the blob for size verification. |
| `open(path, mode)` | `meta.get_entry(path)` | Resolve path to CAS hash via metadata store. |
| `read_at(buf, offset)` | `GET cas/ab/c1/abc123...` with Range header | Fetch blob by hash. Range supported. |
| `write_at(data, offset)` | Fetch blob, modify, compute new hash, `PUT cas/xx/yy/newhash...`, `meta.upsert_entry(path, new_hash)` | New content = new hash = new blob. Old blob may still be referenced by other paths (dedup). |
| `create(path, mode)` | `PUT cas/e3/b0/e3b0c44...` (empty file hash) + `meta.upsert_entry(path, empty_hash)` | Empty file always has the same hash (dedup). |
| `remove(path)` | `meta.delete_entry(path)` | Only remove metadata. Blob remains if referenced by other paths. GC is separate. |
| `rename(from, to)` | `meta.delete_entry(from)` + `meta.upsert_entry(to, same_hash)` | Metadata-only operation -- no blob copy needed (major advantage over PathKey). |
| `list(dir)` | `meta.list_children(dir)` | Directory listing from metadata store (D1 query or manifest read). |

#### DirManifestLayout

| VfsFileSystem Method | R2 Operation | Details |
|---------------------|-------------|---------|
| `stat(path)` | `GET .delta/src/manifest.json` -> find entry for `main.rs` | Read parent directory manifest, find child entry. Entry includes size, checksum, blob key. |
| `open(path, mode)` | Read manifest for parent dir, get blob key for file | Resolve to blob key via manifest. |
| `read_at(buf, offset)` | `GET blobs/x/y/main_v2.rs` with Range header | Fetch blob at the key specified in manifest. |
| `write_at(data, offset)` | Fetch blob, modify, `PUT blobs/x/y/main_v3.rs`, update manifest entry | Write new blob, read-modify-write the parent manifest. |
| `create(path, mode)` | `PUT blobs/...` + read-modify-write `.delta/src/manifest.json` | Add entry to parent manifest. |
| `remove(path)` | `DELETE blobs/...` + read-modify-write manifest | Remove entry from parent manifest, delete blob. |
| `rename(from, to)` | Read-modify-write both source and target manifests | If same directory: single manifest update. Cross-directory: update two manifests. |
| `list(dir)` | `GET .delta/src/manifest.json` | Single R2 GET returns all children with metadata. Fast -- no prefix scan. |
| `mkdir(path)` | `PUT .delta/src/newdir/manifest.json` (empty manifest `[]`) + update parent manifest | Create empty manifest for new dir, add dir entry to parent. |

#### R2Fs Struct

```rust
pub struct R2Fs<Layout: R2Layout, Meta: R2MetaStore> {
    bucket: R2Bucket,
    layout: Layout,
    meta: Meta,
}

impl<Layout: R2Layout, Meta: R2MetaStore> VfsFileSystem for R2Fs<Layout, Meta> {
    type File = R2File<Layout, Meta>;
    type SeekableFile = R2SeekableFile<Layout, Meta>;
    type Directory = R2Directory<Layout, Meta>;
    // All methods delegate to layout + meta
}
```

### R2Delta Structure (extends R2Fs)

R2Delta wraps R2Fs and adds whiteout tracking. It delegates all VfsFileSystem methods to the inner R2Fs.

```rust
pub struct R2Delta<Layout: R2Layout, Meta: R2MetaStore> {
    inner: R2Fs<Layout, Meta>,  // full VfsFileSystem implementation
    version_counter: Arc<AtomicU64>,
}

// R2Delta is a VfsFileSystem (delegates to inner R2Fs)
impl<Layout: R2Layout, Meta: R2MetaStore> VfsFileSystem for R2Delta<Layout, Meta> {
    type File = R2File<Layout, Meta>;
    type SeekableFile = R2SeekableFile<Layout, Meta>;
    type Directory = R2Directory<Layout, Meta>;
    // All methods delegate to self.inner
}

// R2Delta extends VfsFileSystem with DeltaStore
impl<Layout: R2Layout, Meta: R2MetaStore> DeltaStore for R2Delta<Layout, Meta> {
    fn add_whiteout(&self, path: &str, version: u64) -> Result<()> {
        // PathKey: PUT path.whiteout marker object
        // CAS/DirManifest: meta.upsert_entry(path, EntryMeta::Whiteout { version })
    }
    fn is_whiteout(&self, path: &str) -> Result<Option<u64>> { ... }
    fn remove_whiteout(&self, path: &str) -> Result<()> { ... }
    fn list_whiteouts(&self, dir: &str) -> Result<Vec<(String, u64)>> { ... }
    fn flush(&self) -> Result<()> { /* no-op -- writes are immediate */ }
    fn reset(&self) -> Result<()> {
        // Delete all objects in the delta namespace, clear whiteouts
    }
}
```

### R2Layout Trait

```rust
#[async_trait]
pub trait R2Layout: Send + Sync {
    /// Store file content at the appropriate R2 key(s). Returns a reference for later retrieval.
    async fn store_file(&self, bucket: &R2Bucket, path: &str, data: &[u8]) -> Result<FileRef>;

    /// Retrieve file content using the stored reference.
    async fn fetch_file(&self, bucket: &R2Bucket, path: &str, file_ref: &FileRef) -> Result<Vec<u8>>;

    /// Delete file content from R2.
    async fn delete_file(&self, bucket: &R2Bucket, path: &str) -> Result<()>;

    /// List directory entries.
    async fn list_dir(&self, bucket: &R2Bucket, dir_path: &str) -> Result<Vec<R2DirEntry>>;
}
```

### R2MetaStore Trait (for layouts that need external metadata)

```rust
#[async_trait]
pub trait R2MetaStore: Send + Sync {
    async fn upsert_entry(&self, path: &str, meta: EntryMeta) -> Result<()>;
    async fn get_entry(&self, path: &str) -> Result<Option<EntryMeta>>;
    async fn delete_entry(&self, path: &str) -> Result<()>;
    async fn list_children(&self, dir_path: &str) -> Result<Vec<(String, EntryMeta)>>;
}
```

Implementations:
- `D1MetaStore` — uses D1Delta's `D1Connection` to query the metadata tables
- `R2ManifestStore` — stores manifest JSON objects in R2 itself

### Module Structure

```
src/shared/vfs/
    r2_delta/
        mod.rs              # R2Delta struct + DeltaStore impl
        layout/
            mod.rs          # R2Layout trait, FileRef, R2DirEntry types
            path_key.rs     # PathKeyLayout implementation
            cas_key.rs      # CASKeyLayout implementation
            dir_manifest.rs # DirManifestLayout implementation
        meta/
            mod.rs          # R2MetaStore trait
            d1_meta.rs      # D1-based metadata store (uses D1Connection trait)
            manifest.rs     # R2 manifest-based metadata store

src/native/vfs/
    r2_s3.rs                # Native S3-compatible HTTP implementation

wasm/wasm-bindgen/
    r2/
        mod.rs              # wasm-bindgen implementation using worker-rs R2Bucket
```

## Tasks

### Shared (`src/shared/vfs/r2_delta/`)

- [x] Define `R2Layout` trait + `FileRef`, `R2DirEntry` types
- [x] Define `R2MetaStore` trait + `EntryMeta` type
- [x] Implement `PathKeyLayout` (simplest, no metadata store needed)
- [ ] Implement `CASKeyLayout` (content hashing, dedup logic)
- [ ] Implement `DirManifestLayout` (manifest read/write, directory listing)
- [ ] Implement `R2Fs<Layout, Meta>` struct with full `VfsFileSystem` trait:
  - `stat()` -- layout-dependent: HEAD object (PathKey), meta lookup (CAS), manifest read (DirManifest)
  - `open()` / `open_seekable()` -- resolve to blob key, return R2File/R2SeekableFile
  - `create()` -- PUT empty object + metadata
  - `mkdir()` -- PUT dir marker (PathKey), create manifest (DirManifest), metadata entry (CAS)
  - `remove()` -- DELETE object + metadata
  - `rename()` -- COPY+DELETE (PathKey), metadata-only (CAS), manifest update (DirManifest)
  - `list()` -- LIST with prefix (PathKey), meta query (CAS), GET manifest (DirManifest)
  - `chmod()` / `symlink()` / `readlink()` -- via custom metadata headers
- [ ] Implement `R2File` / `R2SeekableFile` / `R2Directory` handle types
- [ ] Implement `R2Delta<Layout, Meta>` struct wrapping R2Fs + `DeltaStore` trait
- [ ] Whiteout support (R2 objects with `.whiteout` suffix, or metadata table entries)

### Metadata Stores (`src/shared/vfs/r2_delta/meta/`)

- [ ] `D1MetaStore` — uses `D1Connection` trait from D1Delta feature
- [ ] `R2ManifestStore` — manifest JSON objects in R2 (`.delta/manifests/`)

### Native HTTP S3 (`src/native/vfs/r2_s3.rs`)

- [ ] Implement `R2Bucket` trait-object for S3 HTTP client
- [ ] S3 authentication (access key, secret key, endpoint URL)
- [ ] Handle S3 response parsing, error translation
- [ ] Feature gate: `cfg(not(target_arch = "wasm32"))` or feature flag

### wasm-bindgen (`wasm/wasm-bindgen/r2/`)

- [ ] Create wasm module directory structure
- [ ] Implement `R2Bucket` trait-object for worker-rs `R2Bucket`
- [ ] Handle worker-rs type conversions (JsValue → Rust types)
- [ ] Feature gate: `cfg(target_arch = "wasm32")` or feature flag

### Tests

- [ ] R2Fs standalone: mkdir, create, write, read, stat, remove (full VfsFileSystem surface) with PathKeyLayout
- [ ] R2Fs as base layer in OverlayFileSystem
- [ ] PathKeyLayout: write -> read -> content matches
- [ ] PathKeyLayout: rename (copy+delete), delete, directory listing (prefix scan)
- [ ] CASKeyLayout: duplicate content -> single R2 object stored
- [ ] CASKeyLayout: read back different paths with same content -> same bytes
- [ ] CASKeyLayout: rename is metadata-only (no blob copy)
- [ ] DirManifestLayout: directory listing = single R2 GET
- [ ] DirManifestLayout: create file updates parent manifest atomically
- [ ] worker-rs impl compiles for wasm32 target
- [ ] HTTP impl works against real R2 or S3-compatible mock (MinIO, LocalStack)

## Feature Flags

```toml
vfs-r2-wasm = ["vfs"]       # wasm-bindgen/worker-rs R2Delta
vfs-r2-http = ["vfs"]       # Native HTTP S3 R2Delta (adds reqwest or similar)
vfs-r2-cas = ["vfs"]        # CASKeyLayout (adds blake3/sha256)
vfs-r2-manifest = ["vfs"]   # DirManifestLayout
```

## Verification

- R2Fs is usable as a standalone VfsFileSystem (not just as a DeltaStore)
- R2Fs is usable as a base layer in OverlayFileSystem
- PathKeyLayout: file written to R2 is readable with identical content
- PathKeyLayout: R2 console shows human-readable keys matching VFS paths
- CASKeyLayout: two files with same content → one R2 blob stored
- CASKeyLayout: read returns correct content for both paths
- DirManifestLayout: `list()` returns directory contents from single manifest GET
- worker-rs impl compiles and runs in Cloudflare Workers
- HTTP impl works against R2 and any S3-compatible service (MinIO, S3, GCS)
- Whiteout hides file, reset restores

## References

- Cloudflare R2 docs
- AWS S3 API documentation (compatible with R2)
- `worker-rs` R2 bindings
- AgentFS — CAS storage ideas, manifest patterns
- `foundation_db` — native HTTP database implementation patterns
