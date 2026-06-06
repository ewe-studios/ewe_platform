---
feature_name: "ObservableFs"
description: "ObservableFs<F: VfsFileSystem> — decorator that wraps any VfsFileSystem, intercepts ALL operations (reads, writes, seeks, stat, readdir, etc.), emits typed audit events via Broadcaster. Full audit trail as a separate concern."
status: "pending"
priority: "medium"
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

## Global Rule: `foundation_errstacks` Error Handling

All VFS types MUST implement `Debug` and use `VfsResult<T>` (`Result<T, ErrorTrace<VfsError>>`) for errors. Tests use `err.current_context()` for typed error matching. See **plan.md §4d** for the full rule.

---

# Feature 15: ObservableFs

## Overview

`ObservableFs<F: VfsFileSystem>` is a decorator that wraps any VfsFileSystem — not just OverlayFileSystem. It implements VfsFileSystem itself, delegates all calls to the inner filesystem, and emits a typed event for **every** operation: reads, writes, seeks, stat, readdir, open, close, chmod, rename, delete — full audit of all actions.

This is a separate concern from the filesystem implementation. The inner filesystem doesn't know it's being observed. Events are delivered via `Broadcaster` for multi-subscriber consumption.

```rust
// Any VfsFileSystem can become observable
let fs = ObservableFs::new(
    OverlayFileSystem::new(NativeFs::new("./project"), MemoryDelta::new())
);
let subscriber = fs.subscribe();

// Use fs normally — every operation emits an event
fs.open("/src/main.rs", Read)?;
// subscriber receives: VfsEvent::FileOpened { path, mode, version, timestamp }
```

## Event Types

Events cover the full range of filesystem operations, not just mutations:

```rust
pub enum VfsEvent {
    // File operations
    FileOpened { path: String, mode: OpenMode, seekable: bool, version: u64 },
    FileCreated { path: String, mode: u32, version: u64 },
    FileClosed { path: String, version: u64 },
    FileRead { path: String, offset: u64, size: usize, version: u64 },
    FileWritten { path: String, offset: u64, size: usize, version: u64 },
    FileSeeked { path: String, position: u64, version: u64 },
    FileTruncated { path: String, size: u64, version: u64 },
    FileSynced { path: String, version: u64 },

    // Path operations
    StatQueried { path: String, version: u64 },
    ExistsQueried { path: String, exists: bool, version: u64 },
    PermissionsChanged { path: String, mode: u32, version: u64 },
    Renamed { from: String, to: String, version: u64 },
    Removed { path: String, version: u64 },
    SymlinkCreated { target: String, link: String, version: u64 },
    SymlinkRead { path: String, target: String, version: u64 },

    // Directory operations
    DirectoryOpened { path: String, version: u64 },
    DirectoryClosed { path: String, version: u64 },
    DirectoryListed { path: String, entry_count: usize, version: u64 },
    DirectoryCreated { path: String, version: u64 },

    // Errors
    OperationFailed { operation: String, path: String, error: String, version: u64 },
}
```

Each event carries a `version` (from the global monotonic counter) and is timestamped.

## Tasks

### Core (`src/shared/vfs/observable_fs.rs`)

- [ ] Define `ObservableFs<F: VfsFileSystem>` struct: inner filesystem + Broadcaster<VfsEvent> + version counter
- [ ] Implement `VfsFileSystem` for `ObservableFs<F>`: delegate every method to inner, emit event before/after
- [ ] Implement `ObservableVfsFile` wrapping `F::File`: intercepts read_at, write_at, sync, truncate with event emission
- [ ] Implement `ObservableSeekableVfsFile` wrapping `F::SeekableFile`: intercepts read, write, seek with event emission
- [ ] Implement `ObservableVfsDirectory` wrapping `F::Directory`: intercepts list, create_file, create_dir, etc with event emission
- [ ] Implement `ObservableFs::subscribe()` → event receiver channel
- [ ] Implement `ObservableFs::new(inner)` constructor
- [ ] Emit `OperationFailed` event when any operation returns Err (includes error context)

### Event Definitions (`src/shared/vfs/vfs_events.rs`)

- [ ] Define `VfsEvent` enum with all variants
- [ ] Implement Serialize + Deserialize for VfsEvent (for IPC/logging)
- [ ] Implement ArrowSerialize for VfsEvent (when foundation_arrow available)

### Integration with Broadcaster

- [ ] Use existing `Broadcaster<VfsEvent>` from foundation_nativeapis
- [ ] Multiple subscribers receive all events

### Tests

- [ ] Test: open + read emits FileOpened + FileRead events
- [ ] Test: write emits FileWritten event with correct offset/size
- [ ] Test: stat emits StatQueried event
- [ ] Test: failed operation emits OperationFailed event
- [ ] Test: multiple subscribers all receive events

## Verification

- Tests pass
- All VfsFileSystem operations produce corresponding events
- No event loss under normal operation
- `cargo check --features vfs --target wasm32-unknown-unknown` passes (no OS deps)
