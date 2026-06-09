# Example: Sync/Async Bridge

## Purpose

Demonstrates the seamless interoperability between synchronous and asynchronous VFS APIs using valtron's `exec_async` runtime and the `SyncFs<A>` bridge adapter. This example shows how to wrap an async-native `VfsFileSystem` implementation and use it through a purely sync interface, enabling integration of async-first codebases into blocking contexts without tokio or other heavy runtimes.

## Prerequisites

- Rust toolchain with workspace dependencies
- Feature flag: `vfs`
- Linux, macOS, or Windows (valtron multi-threaded executor support)

## How to Run

```bash
cargo run -p foundation_nativeapis --features vfs --example sync_async_bridge
```

## Architecture

The example demonstrates two bridging patterns:

1. **Direct async via `exec_async`** — `MemoryFs` implements both sync and async `VfsFileSystem` traits. Using `exec_async`, async operations are executed on valtron's thread pool, allowing async code to run in a blocking context:

   ```rust
   exec_async(async move {
       fs.mkdir_async("/dir".into()).await?;
       let file = fs.create_async("/dir/file.txt".into(), 0o644).await?;
       file.write_at_async(data, 0).await?;
       Ok(())
   }).unwrap();
   ```

2. **`SyncFs<A>` wrapper** — A generic adapter that takes any `AsyncVfsFileSystem` and implements the sync `VfsFileSystem` trait by internally calling `exec_async` for every operation. This provides a clean, ergonomic sync API over async implementations:

   ```rust
   let sync_fs = SyncFs::new(MemoryFs::new());
   sync_fs.mkdir("/bridged").unwrap();  // Internally calls exec_async
   ```

Both patterns avoid the need for tokio, async-std, or other heavyweight runtimes. Valtron provides a minimal, embeddable thread pool suitable for mixed sync/async codebases.

## Expected Output

```
=== Sync/Async Bridge Example ===

--- Direct async via exec_async ---
Wrote via async API
Read via sync API: "async bytes"

--- SyncFs bridge ---
SyncFs read: "written through SyncFs bridge"
SyncFs stat: size=29, inode=3

=== Done ===
```

## Key APIs Demonstrated

- `initialize_pool(thread_count, queue_size)` — Initialize valtron's thread pool
- `exec_async(future)` — Execute an async closure on the valtron pool, blocking until completion
- `SyncFs::new(async_fs)` — Wrap an `AsyncVfsFileSystem` as a sync `VfsFileSystem`
- `AsyncVfsFileSystem::mkdir_async(path)` — Async directory creation
- `AsyncVfsFileSystem::create_async(path, mode)` — Async file creation
- `AsyncVfsFile::write_at_async(data, offset)` — Async write at offset
- `VfsFileSystem::read_file(path)` — Sync read (works on both direct and bridged impls)
- `VfsFileSystem::stat(path)` — Sync stat (works on both direct and bridged impls)

## Where to Use This

- **Legacy integration** — Use async VFS impls in blocking frameworks (e.g., actix-web, synchronous ORMs)
- **CLI tools** — Write async core logic but expose a sync CLI interface
- **Testing** — Run async filesystem code in synchronous test harnesses without `#[tokio::test]`
- **Embedded systems** — Minimal runtime overhead compared to full async runtimes
- **Hybrid servers** — Mix sync and async code paths without maintaining two separate implementations
- **Migration** — Gradually port sync code to async by swapping `SyncFs` for direct async usage

## Implementation Pattern

```rust
// Option 1: Direct async execution
let fs = Arc::new(MemoryFs::new());
exec_async(async move {
    fs.mkdir_async("/dir".into()).await?;
    Ok(())
}).unwrap();

// Option 2: Sync wrapper for ergonomic usage
let sync_fs = SyncFs::new(MemoryFs::new());
sync_fs.mkdir("/dir").unwrap();  // Internally exec_async
```

## Related

- Feature spec: `specifications/37-overlay-vfs/features/07-sync-async-bridge/feature.md`
- Source: `src/shared/vfs/sync_async.rs` (`SyncFs`, `exec_async`)
- Trait definitions: `src/shared/vfs/traits.rs` (`VfsFileSystem`, `AsyncVfsFileSystem`)
- Valtron runtime: `foundation_core/src/valtron/mod.rs`
