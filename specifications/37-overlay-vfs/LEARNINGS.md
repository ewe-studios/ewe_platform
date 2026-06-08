# Learnings — Spec 37: OverlayFileSystem

## Phase 1 (2026-06-05)

### Version counter separation
The OverlayFileSystem has its own version counter (AtomicU64) separate from the delta's internal version counter. Originally tried comparing overlay whiteout versions against delta metadata versions — these are incomparable since they come from different counters. **Fix:** Delta entry at exact path always wins over whiteouts. The overlay contract ensures delta entries are only created after whiteout handling, so existence in delta means "created after deletion."

### Visibility rule (simplified from spec)
Instead of the version-comparison approach from the spec, the implemented rule is:
1. Delta has entry at path → visible (always wins)
2. Whiteout covers path → not visible
3. Fall through to base

This is simpler, correct, and avoids cross-counter version comparison. Version comparison may be revisited if we need more granular ordering, but for Phase 1 in-memory implementations this rule is complete.

### OverlayDirectory raw pointers
OverlayDirectory holds raw pointers to base and delta (owned by OverlayFileSystem). Required `+ 'static` bounds on B and D type parameters. This is sound because OverlayDirectory can only be obtained from an OverlayFileSystem reference and the directory handle cannot outlive the overlay. Consider switching to Arc-based approach in Phase 2 if lifetime issues arise.

### Path model
All VFS paths are string slices, `/`-separated, always absolute within the VFS namespace. No `std::path::Path` — this enables cross-platform + WASM compatibility. Path normalization is a private function duplicated in memory_fs and overlay_fs (could extract to a shared util in Phase 2).

### Error model
Using `foundation_errstacks::ErrorTrace<VfsError>` throughout. Named struct fields in VfsError variants (`NotFound { path: String }` not `NotFound(String)`) for clarity at construction sites.

### Feature flag isolation
The `vfs` feature is fully opt-in, doesn't affect default features, and adds only `foundation_errstacks` + `derive_more` as deps. All VFS code lives under `#[cfg(feature = "vfs")]` in `src/shared/vfs/`.

## Phase 2 (2026-06-05)

### NativeFs.exists must not fail on missing parents
`NativeFs.resolve_path` canonicalizes the parent directory when the target path doesn't exist. If the parent also doesn't exist, canonicalize fails with an I/O error. This caused `exists()` to error out instead of returning `false`. **Fix:** `exists()` catches resolve errors and returns `Ok(false)`. The correct semantic is "does this path exist and is it reachable?" — if the parent path doesn't exist, the answer is simply false, not an error.

### NativeFs path containment via canonicalize
Path containment uses `canonicalize()` to resolve symlinks and `..` components, then checks `starts_with(root)`. This catches all path escape attempts including symlink-based escapes. For paths that don't exist yet (create/mkdir), we canonicalize the parent and join the filename, which still catches escapes via `..` in ancestor components.

### DirectoryDelta whiteout sentinels
Whiteouts stored as `.wh.<basename>` sentinel files on disk with version number as content. An in-memory `HashMap<String, u64>` cache is built at construction time by scanning for `.wh.*` files. This avoids filesystem round-trips on every `is_whiteout` check. The `FilteredDirectory` wrapper hides sentinel files from directory listings.

### Feature flag hierarchy
`vfs-native = ["vfs"]` — the native features depend on the core VFS traits. The native VFS module lives in `src/native/vfs/` and is feature-gated with `#[cfg(feature = "vfs-native")]`.

### Path traversal — `..` rejection at normalize layer (post-Phase 2 fix)
Originally, all `normalize_vfs_path` functions only collapsed slashes and stripped trailing slashes — they did NOT reject `..` components. This was a security vulnerability: DirectoryDelta's whiteout operations (`add_whiteout`, `remove_whiteout`) went directly to `std::fs` via `whiteout_sentinel_path`, bypassing NativeFs's canonicalize-based containment. A path like `/../../../etc/passwd` would write a sentinel file outside the shadow directory.

**Fix:** Extracted a shared `path_utils::normalize_vfs_path` that returns `VfsResult` and rejects `..` with `InvalidPath` error. All four modules (memory_fs, overlay_fs, native_fs, dir_delta) now use this shared function. Additionally, `whiteout_sentinel_path` in dir_delta validates the resolved path starts with the shadow root. This is defense-in-depth: `..` is rejected at the normalize layer (first line of defense), and the sentinel path is validated against the root (second line).

## Phase 3 (2026-06-05) — Scaffold Macro

### proc-macro crate can't export macro_rules!
`macro_rules!` macros cannot be `#[macro_export]`ed from a proc-macro crate. `pub use` of a macro from a proc-macro crate is also forbidden. **Solution:** Define `scaffold!()` in `foundation_nostd::macros` (a regular library crate) and have consumers import from there.

### macro_rules! and `self` keyword
`self` is a reserved keyword that can't appear literally in `macro_rules!` templates when used as a receiver. **Solution:** The generated macro accepts `$self:ident` as the first parameter and the receiver is rewritten to use `$self`. When `#[derive(Scaffold)]` invokes the macro, it passes `self` as the first argument: `__scaffold_methods_Type!(self, self.field)`.

### Receiver must be included in generated method signature
The original scaffoldable macro filtered out `self` from parameters when generating forwarding templates. This caused the generated methods to lack the `self` receiver, making them associated functions instead of methods. **Fix:** Extract the `Receiver` from the original signature and include it in the generated template (with `self` → `$self` substitution).

### #[scaffold_impl] reconstructs impl header
`quote! { impl #impl_block }` produces `impl impl ...` because `ItemImpl` already contains the `impl` keyword. **Solution:** Clone items before iterating, then reconstruct the impl header manually using `impl #generics #trait_path for #self_ty`.

### #[scaffoldable] generates crate-level macros
The generated `__scaffold_methods_{TypeName}!` macro is `#[macro_export]`ed, making it available at the crate root. This means users of `#[derive(Scaffold)]` must have access to the crate where `#[scaffoldable]` was applied. For cross-crate usage, the macro must be re-exported or both types must be in the same crate.

## Phase 4 (2026-06-07) — IPC VFS Daemon

### VfsTransport trait direction mismatch
The `VfsTransport::request(&self)` method takes `&self` but the IPC bus receiver (`EndpointReceiver`) requires `&mut` to call `recv()`. **Solution:** Wrap sender and receiver in `Mutex` so `&self` can acquire a lock. This is a common pattern when a sync trait exposes only shared references but the underlying transport is bidirectional.

### Options doesn't implement Default
`ipc::Options` uses a builder pattern (`Options::new(identifier, label).controller_affinity(true)`) rather than `Default`. Don't use `..Default::default()`.

### ErrorTrace wrapping required
`VfsResult<T>` is `Result<T, ErrorTrace<VfsError>>` — not `Result<T, VfsError>`. All error conversions in IPC bus code must wrap with `ErrorTrace::new(...)`.

### Client/Daemon have opposite bus directions
The client calls `join::<VfsRequest, VfsResponse>()` (sends requests, receives responses). The daemon calls `join::<VfsResponse, VfsRequest>()` (sends responses, receives requests). This means two separate transport structs (`ClientTransport` and `DaemonTransport`) with different generic parameters are needed — they can't share the same struct.

### DirectTransport for in-process testing
`DirectTransport<F>` wraps a `VfsDaemon<F>` and implements `VfsTransport` by calling `daemon.dispatch()` directly. This avoids the IPC bus for testing — no socket setup needed. Both client and daemon logic are validated through this transport.

### NativeFs doesn't support stat_by_inode/path_by_inode
`NativeFs` implements `stat_by_inode` and `path_by_inode` as "operation not supported" because there's no efficient way to reverse-lookup an inode to a path on the native filesystem. These operations require an inode-tracking backend (e.g., OverlayFileSystem with delta metadata). Tests should expect `Err` when using NativeFs.

### Feature flag gating for IPC VFS
The `vfs-ipc` feature depends on both `vfs` and `ipc`. The IPC VFS modules (`ipc_messages`, `ipc_daemon`, `ipc_client`) live in `shared::vfs` (always available when `vfs-ipc` is enabled). The bus transport (`ipc_bus.rs`) lives in `native::vfs` because it uses `ipc::Message<T>` which is native-only (platform socket transport). Feature gates:
- `shared::vfs::ipc_messages` — `#[cfg(feature = "vfs-ipc")]`
- `shared::vfs::ipc_daemon` / `ipc_client` — `#[cfg(all(feature = "vfs-ipc", any(target_os = "linux", target_os = "macos", target_os = "windows")))]`
- `native::vfs::ipc_bus` — same as above

## Phase 4 (2026-06-07) — Ptrace Interceptor

### PTRACE_SEIZE vs PTRACE_TRACEME
`PTRACE_TRACEME` has a well-known issue: after the initial SIGTRAP stop, calling `PTRACE_SYSCALL` completes the execve entry but the subsequent stop is a plain `SIGTRAP` (not `PTRACE_EVENT_EXEC`), and further `PTRACE_SYSCALL` calls produce `PtraceSyscall` stops that don't generate corresponding exit stops — the ptrace loop hangs.

**Solution**: Use `PTRACE_SEIZE` (Linux 3.4+) which:
1. Stops the child atomically with trace options set before any code runs
2. Supports `PTRACE_O_TRACESYSGOOD` — syscall stops delivered as `SIGTRAP | 0x80` (signal 137), distinguishing them from plain signal stops
3. The execve flow works correctly: SEIZE → SYSCALL (execve entry) → waitpid (execve entry stop) → SYSCALL (complete execve) → waitpid (PTRACE_EVENT_EXEC) → SYSCALL (start tracing new program)

Sources: [man7.org ptrace(2)](https://man7.org/linux/man-pages/man2/ptrace.2.html), [strace README-linux-ptrace](https://github.com/bnoordhuis/strace/blob/master/README-linux-ptrace), [blog.nelhage.com](https://blog.nelhage.com/2010/08/write-yourself-an-strace-in-70-lines-of-code/), [stackoverflow: changing from TRACEME to SEIZE](https://stackoverflow.com/questions/73168140/changing-from-ptrace-traceme-to-ptrace-seize)

### SYSCALL_STOP_SIGNAL detection
With `PTRACE_O_TRACESYSGOOD`, `WSTOPSIG(status) == SIGTRAP | 0x80` (137) for syscall stops. This is the approach used by `strace` and is cleaner than checking `orig_rax >= 0`.

### PID-1 supervision via waitpid(-1)
The ptrace loop uses `waitpid(-1, __WALL)` instead of `waitpid(child_pid)` to receive stops from all traced processes (including forked children). This follows the iii-init pattern. Key insight: `pending_actions` is keyed by `(pid)` so each traced process has its own entry/exit state machine.

Sources: [iii-init supervisor.rs](/home/darkvoid/Boxxed/@formulas/src.rust/src.AI/src.iii/iii/crates/iii-init/src/supervisor.rs), [iii-supervisor child.rs](/home/darkvoid/Boxxed/@formulas/src.rust/src.AI/src.iii/iii/crates/iii-supervisor/src/child.rs)

### Platform module: errno translation and openat2
Copied from `iii-filesystem/platform.rs` — 85+ BSD→Linux errno mappings for macOS FUSE protocol compatibility, plus `openat2(RESOLVE_BENEATH)` kernel-enforced path containment probe (Linux 5.6+). On Linux, `linux_error()` is identity; on macOS it maps BSD errno to Linux errno.

### killpg for process group management (iii pattern)
The iii-supervisor uses `nix::sys::signal::{Signal, killpg}` for SIGTERM→SIGKILL escalation with graceful shutdown. `Command::process_group(0)` puts each worker in its own process group, and `killpg(pgid, Signal::SIGTERM)` kills the entire worker subtree (npm, shells, tsx, node, esbuild) in one shot. **Relevance to ptrace:** When the traced child forks a complex process tree (e.g., `sh -c "npm run dev"`), we may need killpg to clean up orphaned descendants if the traced child dies unexpectedly.

### DynFs type erasure for heterogeneous mount table
`VfsFileSystem` has associated types (`File`, `SeekableFile`, `Directory`) that can't be erased with `dyn`. Solution: `DynFs` wraps `Arc<dyn DynFsOps>` where `DynFsOps` is a trait with all VFS operations returning boxed trait objects (`ErasedFile`, `ErasedDir`). This allows the `MountTable` to store heterogeneous filesystems behind a common interface.

### VfsTask/VfsTaskBuilder/VfsEventReadiness feature gate
These types were imported in `lib.rs` without `#[cfg(feature = "vfs")]` — they're defined in `valtron::vfs_task` which is only compiled with `vfs`. Fixed by gating the re-export: `#[cfg(feature = "vfs")] pub use valtron::{VfsTask, VfsTaskBuilder, VfsEventReadiness};`

### Blanket impls for Box<dyn VfsFile> and Box<dyn VfsDirectory>
To make type-erased wrappers work, blanket impls `impl<T: VfsFile + ?Sized> VfsFile for Box<T>` and `impl<T: SeekableVfsFile + ?Sized> SeekableVfsFile for Box<T>` are needed. These delegate to `(**self)`. The `VfsDirectory` blanket impl must carefully preserve associated types: `type File = T::File; type SeekableFile = T::SeekableFile;`.

### MountTable Clone for Arc-based sharing
`MountTable` contains `Vec<(String, DynFs)>` where `DynFs` wraps `Arc<dyn DynFsOps>`. Manual `Clone` impl that clones the Arc (not the underlying filesystem) — cheap clone, shared ownership.

## Phase 4 (2026-06-07) — FUSE Adapter

### FUSE operates on inodes, not paths
Every FUSE callback receives an inode number, never a path string. The adapter maintains a synthetic inode-to-path cache: root is always inode 1, `AtomicU64` counter allocates subsequent inodes on `lookup()`. The kernel manages refcounts via `forget(ino, nlookup)` — eviction is optional for performance.

### VfsError to FUSE errno mapping
Each `VfsError` variant maps to a specific `libc` errno: `NotFound→ENOENT`, `AlreadyExists→EEXIST`, `PermissionDenied→EACCES`, `NotAFile→EISDIR`, `NotADirectory→ENOTDIR`, `Unsupported→ENOSYS`, `Io→EIO`, `InvalidPath→EINVAL`, `ReadOnly→EROFS`, `EntryPending→EAGAIN`.

## Phase 4 (2026-06-07) — Inode-Native VFS

### Inodes as first-class VFS concept
FUSE's synthetic inode cache was duplicating state already tracked by the underlying VFS. Solution: push inode awareness into the VFS layer itself — `VfsMetadata` and `VfsDirEntry` carry inode numbers, `VfsFileSystem` owns inode allocation and provides `inode()`, `path_by_inode()`, `stat_by_inode()` reverse lookup. Eliminates the FUSE-side cache entirely.

### Inode rules
Inode 0 = reserved ("no inode"), inode 1 = root `/` (FUSE convention), monotonic counters never reuse, `u64` matches POSIX `ino_t`. NativeFs uses real OS inodes (`MetadataExt::ino()`); `path_by_inode()` returns `Unsupported` since the OS provides no efficient reverse lookup.

## Phase 3 (2026-06-06) — Async-First Migration

### Async-first, sync wraps: two backend categories
**Async-native backends** (LibsqlDelta, TursoDelta) implement `AsyncVfsFileSystem` only and get sync API for free via `SyncFs<Backend>`. **Sync-native backends** (MemoryFs, NativeFs) implement sync traits directly AND also implement async traits (methods return immediately) for use in async contexts. The `_async` suffix convention (`read_at_async`, `open_async`) eliminates UFCS disambiguation.

### SyncFs<A> bridge wrapper
Generic `SyncFs<A: AsyncVfsFileSystem>` bridges async→sync by running async calls through valtron's `exec_async`. This replaced ~30 per-method inline `exec_future(async move { ... })` closures in LibsqlDelta, each of which cloned Arcs and `.to_string()`ed parameters. One bridge type replaced all boilerplate.

## Phase 5 (2026-06-07) — ObservableFs

### ObservableFs as pure decorator
`ObservableFs<F>` wraps any `VfsFileSystem` and emits audit events for ALL operations via `Broadcaster`. It does not add any I/O logic — every method delegates to the inner filesystem and emits a `VfsEvent` before/after. This makes it composable: `ObservableFs<OverlayFileSystem<NativeFs, LibsqlDelta>>`.
