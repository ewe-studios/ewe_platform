# Spec 37: OverlayFileSystem — Cross-Platform Virtual File System

## Context

We need a cross-platform virtual filesystem abstraction that can overlay any directory, providing read-only passthrough for existing files while capturing all writes to a pluggable delta store. This enables safe sandboxing (agents can't corrupt source files), session-based rollback, auditability, and extensible storage backends (local, SQLite, S3, etc).

**Research sources:**
- **AgentFS**: SQLite-backed overlay VFS for AI agents — copy-on-write, whiteout tombstones, directory entry merging, FUSE/NFS mounting, ptrace syscall interception
- **iii-filesystem**: Production FUSE passthrough — zero-copy I/O, synthetic inode table, path containment via `openat2(RESOLVE_BENEATH)`, cross-platform errno/flag translation

**Target crate:** `foundation_nativeapis` — must work across native (Linux, macOS, Windows), WASI, and WASM targets.

---

## Design Decisions (All Settled)

### 1. Trait-first, implementations plug in

The traits ARE the product. Everything else — FUSE, ptrace, SQLite, S3, in-memory, NFS — is an implementation. The user always knows what implementation they instantiated.

### 2. Three distinct handle types — File, SeekableFile, Directory

Files and directories are NOT conflated.

**VfsFile** — offset-based byte I/O on a regular file:
```rust
pub trait VfsFile: Send + Sync {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> Result<usize>;
    fn write_at(&self, buf: &[u8], offset: u64) -> Result<usize>;
    fn sync(&self) -> Result<()>;
    fn size(&self) -> Result<u64>;
    fn truncate(&self, size: u64) -> Result<()>;
    fn metadata(&self) -> Result<VfsMetadata>;
}
```

**SeekableVfsFile: VfsFile** — adds seek-position state:
```rust
pub trait SeekableVfsFile: VfsFile {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize>;
    fn write(&mut self, buf: &[u8]) -> Result<usize>;
    fn seek(&mut self, pos: SeekFrom) -> Result<u64>;
    fn position(&self) -> u64;
}
```

**VfsDirectory** — directory-specific operations, knows its path/parent:
```rust
pub trait VfsDirectory: Send + Sync {
    type File: VfsFile;
    type SeekableFile: SeekableVfsFile;

    // Identity
    fn path(&self) -> &str;
    fn metadata(&self) -> Result<VfsMetadata>;

    // Immediate children
    fn list(&self) -> Result<Vec<VfsDirEntry>>;
    fn get_entry(&self, name: &str) -> Result<Option<VfsDirEntry>>;
    fn create_file(&self, name: &str, mode: u32) -> Result<Self::File>;
    fn create_dir(&self, name: &str) -> Result<Box<dyn VfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>>;
    fn remove_entry(&self, name: &str) -> Result<()>;
    fn rename_entry(&self, old_name: &str, new_name: &str) -> Result<()>;

    // Path resolution — relative, absolute (within subtree), child names all work
    fn open(&self, path: &str, mode: OpenMode) -> Result<Self::File>;
    fn open_seekable(&self, path: &str, mode: OpenMode) -> Result<Self::SeekableFile>;
    fn open_directory(&self, path: &str) -> Result<Box<dyn VfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>>;
    fn stat(&self, path: &str) -> Result<VfsMetadata>;
    fn exists(&self, path: &str) -> Result<bool>;

    // Recursive — default impls, implementations override with optimized versions
    fn remove_all(&self, name: &str) -> Result<()> { ... }
    fn mkdir_all(&self, path: &str) -> Result<()> { ... }
    fn copy(&self, from: &str, to: &str) -> Result<()> { ... }
}
```

### 3. Open model — enum + three methods

```rust
pub enum OpenMode {
    Read,
    Write,
    ReadWrite,
}
```

FileSystem provides: `open()`, `open_seekable()`, `open_directory()`. The enum carries access intent, the method name carries capability type. Implementations that can't provide seekable return `Err`.

### 4. Async-first, sync wraps via valtron

All implementations are written as `async fn`. The async traits are the primary surface. The sync traits are thin wrappers that use valtron to execute the async implementations synchronously. **No tokio** — the async runtime is never a library dependency. Valtron handles execution at the edge.

### 5. Metadata model

Every file/directory carries metadata regardless of platform. Where the platform doesn't enforce something (uid/gid on WASM), it's still stored, recorded, and retrievable. Permissions are metadata, not enforcement — enforcement is the implementation's business.

```rust
pub struct VfsMetadata {
    pub size: u64,
    pub file_type: VfsFileType,         // Regular, Directory, Symlink
    pub permissions: u32,               // mode bits — always stored
    pub owner: (u32, u32),              // (uid, gid) — stored even where meaningless
    pub created: Option<SystemTime>,
    pub modified: Option<SystemTime>,
    pub accessed: Option<SystemTime>,
    pub checksum: Checksum,             // always present, blake3 default
    pub version: u64,                   // global monotonic version — every mutation increments
    pub state: VfsEntryState,           // whether entry is settled or in-flight
}

pub enum VfsEntryState {
    Ready,                              // fully materialized, safe to read/write/seek
    Pending,                            // CoW or write in progress — reads/seeks should fail
}

pub enum Checksum {
    Blake3([u8; 32]),
    Md5([u8; 16]),
    Crc32(u32),
}
```

Version is a global monotonic counter owned by the OverlayFileSystem instance. Every mutation (create, write, delete, rename, whiteout) increments it. Shared across all file/directory handles via an inner shared state (`Arc<AtomicU64>` or similar). Resolves ambiguity in overlay semantics — e.g., whiteout at version 5 on `/src/` is superseded by delta directory at version 8. Also provides natural ordering for event emission.

### 6. Error model

Uses `foundation_errstack` with `derive_more::Derive` for custom errors carrying trace information. `VfsError::Unsupported` for operations the implementation can't handle.

### 7. Filesystem capabilities

`capabilities()` method on VfsFileSystem reports what this implementation supports (seek, symlinks, permissions enforcement, event emission, etc). Query before calling.

### 8. Default impls with override

`remove_all`, `mkdir_all`, `copy`, `read_file`, `write_file` have default implementations. Implementations override with optimized versions (CAS-aware copy, bulk delete, etc).

### 9. DeltaStore = VfsFileSystem + whiteouts + lifecycle

A DeltaStore IS a filesystem. How it stores data internally (full copies, CAS-deduplicated chunks, binary diffs reconstructed on read) is the implementation's business. The overlay just calls `open()` and gets bytes back.

```rust
pub trait DeltaStore: VfsFileSystem {
    // Whiteout management — versioned
    fn add_whiteout(&self, path: &str, version: u64) -> Result<()>;
    fn is_whiteout(&self, path: &str) -> Result<Option<u64>>;  // returns whiteout version if exists
    fn remove_whiteout(&self, path: &str) -> Result<()>;
    fn list_whiteouts(&self, dir: &str) -> Result<Vec<(String, u64)>>;  // (path, version) pairs

    // Lifecycle
    fn flush(&self) -> Result<()>;
    fn reset(&self) -> Result<()>;   // discard all changes
}
```

Whiteouts carry version numbers. When resolving overlay lookups, OverlayFileSystem compares whiteout version against delta entry version — if a delta entry has a higher version than the whiteout on its parent, the delta entry wins (it was created after the deletion).

Any filesystem can become a delta store: directory, SQLite, PostgreSQL, S3/GCS/R2, in-memory, tar archive, KV store.

### 10. Symlinks — follow by default

Overlay CoW follows symlinks and operates on the target. The implementation could defer, but that's its concern. Explicit `symlink()` calls create symlinks in the delta as-is.

### 11. CoW — whole-file at trait level, implementation optimizes internally

The OverlayFileSystem overlay contract is whole-file CoW. Implementations are free to chunk, deduplicate (CAS), or diff internally — the trait consumer sees complete files.

### 12. Concurrency — pushed to implementation

OverlayFileSystem and all traits require `Send + Sync`. How that's achieved is the implementation's concern: `RwLock<HashMap>` for memory, ACID for SQLite, eventual consistency for S3, etc.

### 13. Event emission — optional capability via Broadcaster

Delta mutations can emit change events (file created, modified, deleted). Reported via `capabilities()`. Integrates with spec-34 file watcher infrastructure — a custom poller checks the event stream, `Broadcaster` handles multiple subscribers/consumers.

---

## VfsFileSystem Trait — Full Surface

```rust
pub trait VfsFileSystem: Send + Sync {
    type File: VfsFile;
    type SeekableFile: SeekableVfsFile;
    type Directory: VfsDirectory;

    // Capabilities
    fn capabilities(&self) -> VfsCapabilities;

    // Path operations
    fn stat(&self, path: &str) -> Result<VfsMetadata>;
    fn exists(&self, path: &str) -> Result<bool>;
    fn chmod(&self, path: &str, mode: u32) -> Result<()>;
    fn symlink(&self, target: &str, link: &str) -> Result<()>;
    fn readlink(&self, path: &str) -> Result<String>;
    fn rename(&self, from: &str, to: &str) -> Result<()>;
    fn remove(&self, path: &str) -> Result<()>;

    // Typed open
    fn open(&self, path: &str, mode: OpenMode) -> Result<Self::File>;
    fn open_seekable(&self, path: &str, mode: OpenMode) -> Result<Self::SeekableFile>;
    fn open_directory(&self, path: &str) -> Result<Self::Directory>;
    fn create(&self, path: &str, mode: u32) -> Result<Self::File>;
    fn mkdir(&self, path: &str) -> Result<()>;

    // Convenience (default impls, overridable)
    fn read_file(&self, path: &str) -> Result<Vec<u8>> { ... }
    fn write_file(&self, path: &str, data: &[u8]) -> Result<()> { ... }
    fn copy(&self, from: &str, to: &str) -> Result<()> { ... }
    fn remove_all(&self, path: &str) -> Result<()> { ... }
    fn mkdir_all(&self, path: &str) -> Result<()> { ... }
}
```

---

## OverlayFileSystem — The Overlay Composition Primitive

```rust
pub struct OverlayFileSystem<B: VfsFileSystem, D: DeltaStore> {
    base: B,        // read-only passthrough
    delta: D,       // writable delta layer
}
```

OverlayFileSystem itself implements VfsFileSystem, so overlays can stack:
`OverlayFileSystem<OverlayFileSystem<NativeFs, MemoryDelta>, S3Delta>`

### Lookup Semantics

1. Check whiteouts — if path is whiteout'd, return NotFound
2. Check delta — if exists in delta, return delta version
3. Check base — fall through to base layer
4. Neither — return NotFound

### Copy-on-Write

- `open(path, Read)` — delta handle if in delta, else base handle (read-only)
- `open(path, Write|ReadWrite)` — copy from base to delta if not already there, return delta handle
- Follows symlinks during CoW

### Overlay Edge Case Semantics

**Rename across layers:** Atomic. Read from base → write to delta at new path → whiteout old path. If the write fails, no whiteout is created, partial state is cleaned up. File stays at original path.

**Concurrent CoW:** Implementation owns concurrency. First writer wins. Delta store ensures this (locks, UNIQUE constraints, etc). OverlayFileSystem does NOT hold per-path locks.

**Write into whiteout'd directory:** Versions resolve it. Whiteout at version 5 on `/src/` hides base entries. New delta directory at version 8 and file at version 9 supersede the whiteout (higher version wins). No need to remove the directory whiteout or enumerate individual child whiteouts.

**Partial write failure during CoW:** All-or-nothing. If copy from base to delta fails midway, partial data is cleaned up from delta. File remains base-only. The returned error is rich — carries partial data location, bytes written, original path. Caller can build resumption/compensation on top. OverlayFileSystem guarantees atomicity; resumption is the caller's concern.

**Stat during active CoW:** Metadata is returned with `state: VfsEntryState::Pending`, indicating the file is in-flight (CoW in progress). Callers that attempt to `open()`, `read()`, or `seek()` on a Pending entry get an error. Once CoW completes, state flips to `Ready`. If CoW fails, the Pending state is cleaned up and the file reverts to base-only (Ready from base).

### Whiteout Semantics (Versioned)

- Delete base file → add whiteout tombstone with current version
- Delete delta file → remove from delta + add whiteout (with version) if also in base
- Whiteouts inherit downward: `/a/b` whiteout at version N means `/a/b/**` from base is hidden
- Create at whiteout'd path → create in delta with version M > N. The whiteout remains but is superseded by the higher-versioned delta entry. No need to remove the whiteout or create individual child whiteouts.
- Version comparison resolves ambiguity: delta entry version > whiteout version means "user recreated this after deleting the parent"

### Directory Entry Merging

1. Delta entries for directory
2. Add base entries NOT in delta AND NOT whiteout'd
3. Return merged, deduplicated list

---

## Implementation Targets

### VfsFileSystem Implementations (Base Layer)

| Implementation | Platform | Description |
|----------------|----------|-------------|
| `NativeFs` | Linux, macOS, Windows | Host filesystem via std::fs, path containment |
| `WasiFs` | WASI | WASI filesystem APIs |
| `MemoryFs` | All (esp. WASM) | In-memory HashMap-backed |

### DeltaStore Implementations (Write Layer)

| Implementation | Platform | Description |
|----------------|----------|-------------|
| `MemoryDelta` | All | In-memory. WASM, testing, ephemeral sessions. |
| `DirectoryDelta` | Native + WASI | Shadow directory with sentinel whiteout files. |
| `SqliteDelta` | Native + WASI | SQLite-backed. ACID, single-file, queryable. Feature-gated. |
| (future) `S3Delta` | Native | AWS S3 / GCS / R2 as write layer. |
| (future) `PgDelta` | Native | PostgreSQL-backed delta store. |
| (future) `CasDelta` | All | Content-addressable storage for deduplication. |

### Mount Adapters (Transparent Access for External Processes)

| Implementation | Platform | Description |
|----------------|----------|-------------|
| `FuseMount` | Linux | FUSE via `fuser`. Synthetic inode-to-path cache. |
| `NfsMount` | macOS, Linux | NFS v3 loopback. No kext on macOS. |
| `PtraceInterceptor` | Linux | Reverie/ptrace syscall interception. |
| `SocketDaemon` | All | Unix socket / named pipe VFS daemon + client library. |
| `LdPreloadShim` | Linux, macOS | LD_PRELOAD libc interception, redirects to VFS daemon. |
| `ProjFsMount` | Windows 10+ | Windows Projected File System — overlay-native. Deferred. |
| (future) `WinFspMount` | Windows | WinFsp/Dokan FUSE-like driver. |

---

## Platform Mounting & Interception — Full Landscape

### How Overlay/VFS Transparency Works Per Platform

Each platform has different mechanisms for making a VfsFileSystem transparent to external processes (so `cat`, `ls`, `gcc`, etc. work without modification). These range from fully kernel-integrated to userspace-only workarounds.

### Linux — Richest Options

| Mechanism | Transparency | Requires | Performance | Notes |
|-----------|-------------|----------|-------------|-------|
| **FUSE** | Full (kernel intercepts all VFS ops on mountpoint) | `fuse` kernel module (usually loaded) | Good — one context switch per op. FUSE passthrough mode on 6.9+ bypasses userspace for unmodified base files | Primary adapter. `fuser` crate. |
| **Kernel OverlayFS** | Full (kernel merges upper/lower dirs) | Nothing (built-in since 3.18) | Best — zero userspace overhead | Only works with DirectoryDelta (dir-over-dir). Can't plug SQLite/S3/CAS. Docker uses this. Potential fast path when DirectoryDelta is the delta store. |
| **ptrace/Reverie** | Full (intercepts every syscall) | ptrace permission | Moderate — per-syscall ptrace overhead | Best for sandboxing (prevent host writes). Reverie crate. |
| **eBPF FUSE passthrough** | Full | Kernel 6.9+ | Excellent — reads bypass userspace for base files | Future optimization for FUSE adapter. Unmodified base files served directly by kernel. |
| **Unix socket daemon** | Needs client lib | Nothing | Good — one IPC roundtrip per op | Portable, no kernel deps. Multiple processes share one VFS instance. |
| **LD_PRELOAD** | Semi (intercepts libc calls but not raw syscalls) | Nothing | Good — function pointer redirect | Catches most apps. Misses statically-linked binaries and direct `syscall()` invocations. Works for interpreted languages (Python, Node, etc). |
| **Named pipes (FIFOs)** | None (point-to-point byte stream) | Nothing | N/A | NOT a mounting mechanism. IPC channel only. No directory listing, no stat, no random access. Only useful as transport between client and VFS daemon. |
| **io_uring** | N/A (I/O optimization, not mounting) | Kernel 5.1+ | Excellent | Batch I/O for FUSE backends. Reduces syscall overhead inside the VFS daemon, not for the mounting itself. |

### macOS — Constrained by Apple Security

| Mechanism | Transparency | Requires | Performance | Notes |
|-----------|-------------|----------|-------------|-------|
| **NFS loopback** | Full (kernel mounts NFS) | Nothing (NFS client built-in) | Good — loopback network overhead | Primary macOS adapter. AgentFS uses this. `mount_nfs` with `resvport,soft,timeo=30,retrans=5`. |
| **macFUSE** | Full | kext (kernel extension) install | Good | Apple increasingly restricts kexts (SIP). Requires user to disable SIP or use notarized kext. Fragile across macOS updates. |
| **FUSE-T** | Full | Nothing (uses NFS internally) | Good | FUSE-compatible API but backed by NFS transport. No kext. Best of both worlds — FUSE API, no kernel extension. Consider as alternative to raw NFS. |
| **LD_PRELOAD (DYLD_INSERT_LIBRARIES)** | Semi | Nothing (but SIP restricts for system binaries) | Good | Works for non-system binaries. SIP prevents injection into `/usr/bin/*` etc. Fine for user-compiled tools. |
| **Unix socket daemon** | Needs client lib | Nothing | Good | Same as Linux. Portable. |
| **ptrace** | Blocked by SIP | SIP disabled | N/A | Not viable in production on macOS. |

### Windows — Different Model Entirely

| Mechanism | Transparency | Requires | Performance | Notes |
|-----------|-------------|----------|-------------|-------|
| **ProjFS (Projected File System)** | Full | Built into Windows 10+ | Excellent | **Best fit for overlay model.** You register a "provider" that materializes files on demand. Base files are placeholders (projections), writes create real files. Microsoft uses this for VFS for Git (GVFS). Maps almost perfectly to OverlayFileSystem: base = projections, delta = hydrated files. |
| **WinFsp** | Full | Driver install | Good | FUSE-like for Windows. Mature, well-maintained. `winfsp-rs` crate exists. Requires admin install. |
| **Dokan** | Full | Driver install | Good | Alternative to WinFsp. Similar trade-offs. |
| **Named pipes** | Needs client lib | Nothing | Good | Windows named pipes are bidirectional (unlike Unix FIFOs). Could serve as transport for VFS daemon. |
| **Minifilter driver** | Full | Kernel-mode dev + code signing | Best | Maximum control. Way too complex for our needs. |

### WASM / WASI

| Mechanism | Transparency | Requires | Performance | Notes |
|-----------|-------------|----------|-------------|-------|
| **WASI filesystem** | Within WASI sandbox | WASI runtime | Good | WASI provides `fd_read`, `fd_write`, `path_open` etc. Rust std::fs works on WASI targets. NativeFs/WasiFs just works. |
| **In-memory (MemoryFs)** | Via trait API only | Nothing | Best | Pure WASM (browser) has no real filesystem. MemoryFs is the only option. |
| **Service worker intercept** | Semi (browser only) | Browser APIs | N/A | Future: service worker intercepts fetch requests to virtual paths. Very niche. |

### Transparency Hierarchy (Most → Least Transparent)

```
Kernel-level (fully transparent, any process works)
  ├── FUSE (Linux) / macFUSE (macOS) / WinFsp (Windows)
  ├── Kernel OverlayFS (Linux, DirectoryDelta only)
  ├── NFS loopback (macOS, Linux)
  ├── ProjFS (Windows 10+)
  └── ptrace/Reverie (Linux, per-process)

Userspace interception (mostly transparent, some gaps)
  ├── LD_PRELOAD / DYLD_INSERT_LIBRARIES (libc calls only)
  └── FUSE-T (macOS, NFS-backed FUSE API)

Client library (not transparent, app must opt in)
  ├── Unix socket / named pipe daemon
  └── Direct VfsFileSystem trait usage
```

### Key Insight: Named Pipes vs FUSE

Named pipes (FIFOs) are **IPC channels**, not filesystem mechanisms. They're point-to-point byte streams — one writer, one reader, no directory listing, no seeking, no stat. They cannot present a virtual directory tree.

**FUSE** uses `/dev/fuse` (a character device, conceptually similar to a named pipe) as its transport — the kernel writes VFS operation requests to it, the userspace daemon reads and responds. So FUSE is essentially "kernel intercepts filesystem calls + named-pipe-like transport to userspace daemon."

Named pipes/Unix sockets fit our architecture as the **transport layer** for a VFS daemon that multiple client processes connect to. The daemon hosts the OverlayFileSystem, clients send operation requests over the socket, daemon responds with results. Not transparent to unmodified apps, but portable everywhere and requires no kernel support.

---

## Module Structure

```
src/shared/vfs/            # Always compiled, cross-platform
    mod.rs                 # Re-exports
    traits.rs              # VfsFileSystem, VfsFile, SeekableVfsFile, VfsDirectory, DeltaStore
    async_traits.rs        # Async counterparts of all traits
    types.rs               # VfsMetadata, VfsDirEntry, OpenMode, VfsCapabilities, Checksum
    error.rs               # VfsError via foundation_errstack
    overlay_fs.rs          # OverlayFileSystem<B, D> overlay implementation
    memory_fs.rs           # MemoryFs (VfsFileSystem impl)
    memory_delta.rs        # MemoryDelta (DeltaStore impl)

src/native/vfs/            # Feature-gated, OS-specific
    native_fs.rs           # NativeFs passthrough (path containment, openat2)
    dir_delta.rs           # DirectoryDelta (.snapshot/ with sentinel whiteouts)
    fuse.rs                # FUSE adapter (Linux)
    nfs.rs                 # NFS adapter (macOS)
    ptrace.rs              # Ptrace interceptor (Linux)

src/valtron/
    vfs_task.rs            # VfsTask — event emission via Broadcaster, integrates with spec-34 watchers
```

### Feature Flags

```toml
vfs = []                     # Core traits + OverlayFileSystem + MemoryFs/MemoryDelta (zero new deps)
vfs-native = ["vfs"]         # NativeFs + DirectoryDelta
vfs-sqlite = ["vfs"]         # SqliteDelta (adds sqlite dep)
vfs-fjall = ["vfs"]          # FjallFs/FjallDelta (adds fjall, cacache, ssri, scru128, bincode deps)
vfs-fuse = ["vfs-native"]   # Linux FUSE (adds fuser dep)
vfs-nfs = ["vfs-native"]    # macOS NFS (adds nfsserve dep)
vfs-ptrace = ["vfs-native"] # Linux ptrace (adds reverie dep)
```

---

## Feature Phasing (To Discuss)

### Phase 1 — Core (portable, zero new deps)
- Feature 01: Core traits + types + error
- Feature 02: MemoryFs + MemoryDelta
- Feature 03: OverlayFileSystem overlay logic (CoW, whiteouts, merging)

### Phase 2 — Native
- Feature 04: NativeFs passthrough (path containment)
- Feature 05: DirectoryDelta

### Phase 3 — Structured Storage + Serialization
- Feature 06: SqliteDelta
- Feature 12: foundation_arrow crate (standalone, general-purpose Arrow serialization)
- Feature 18: Scaffold derive macro
- Feature 19: FjallFs / FjallDelta (fjall LSM-tree + cacache CAS backend)

### Phase 4 — Transparent Mounting
- Feature 07: FUSE adapter (Linux)
- Feature 08: NFS adapter (macOS)
- Feature 09: Ptrace/Reverie interceptor (Linux)

### Phase 5 — Integration + Cloud
- Feature 10: Valtron VfsTask + event emission + Broadcaster
- Feature 11: IPC VFS Daemon + VfsClient (builds on IPC bus from spec-34)
- Feature 13: D1Delta — Cloudflare D1 as delta store
- Feature 14: R2Delta — Cloudflare R2 as delta store
