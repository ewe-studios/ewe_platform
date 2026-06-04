---
feature_name: "Cloudflare R2 DeltaStore"
description: "Cloudflare R2 (S3-compatible object storage) as DeltaStore — pluggable key layout adapter (Path, CAS, DirManifest), dual implementations (wasm-bindgen/worker-rs + native S3 HTTP)"
status: "pending"
priority: "low"
phase: 5
created: 2026-06-04
updated: 2026-06-04
dependencies:
  - "01-core-traits"
tasks:
  completed: 0
  uncompleted: 14
  total: 14
  completion_percentage: 0%
---

# Feature 14: Cloudflare R2 DeltaStore

## Overview

Use Cloudflare R2 (S3-compatible object storage) as a DeltaStore implementation. The key design is a **pluggable layout adapter** — the same R2 backend supports multiple storage strategies, letting users choose what fits their use case.

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

### R2Delta Structure

```rust
pub struct R2Delta<Layout, Meta> {
    bucket: R2Bucket,        // worker-rs R2Bucket or HTTP S3 client
    layout: Layout,          // PathKeyLayout, CASKeyLayout, or DirManifestLayout
    meta: Meta,              // metadata store (None for PathKeyLayout, D1Conn for CAS, etc.)
    version_counter: Arc<AtomicU64>,
}

impl<Layout, Meta> DeltaStore for R2Delta<Layout, Meta>
where
    Layout: R2Layout,
    Meta: R2MetaStore,
{ ... }
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

- [ ] Define `R2Layout` trait + `FileRef`, `R2DirEntry` types
- [ ] Define `R2MetaStore` trait + `EntryMeta` type
- [ ] Implement `PathKeyLayout` (simplest, no metadata store needed)
- [ ] Implement `CASKeyLayout` (content hashing, dedup logic)
- [ ] Implement `DirManifestLayout` (manifest read/write, directory listing)
- [ ] Implement `R2Delta<Layout, Meta>` struct + `DeltaStore` trait
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

- [ ] PathKeyLayout: write → read → content matches
- [ ] PathKeyLayout: rename, delete, directory listing
- [ ] CASKeyLayout: duplicate content → single R2 object stored
- [ ] CASKeyLayout: read back different paths with same content → same bytes
- [ ] DirManifestLayout: directory listing = single R2 GET
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
