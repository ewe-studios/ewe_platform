---
feature_name: "IPC VFS Daemon"
description: "VfsDaemon — hosts any VfsFileSystem over the spec-34 IPC bus. VfsClient implements VfsFileSystem by sending requests to the daemon. Includes a binary wrapper. Builds on existing MessageBox, EndpointSender/Receiver, platform transport."
status: "done"
priority: "medium"
phase: 4
created: 2026-06-04
updated: 2026-06-07
dependencies:
  - "01-core-traits"
  - "12-arrow-serialization"
  - "23-inode-native-vfs"
tasks:
  completed: 16
  uncompleted: 0
  total: 16
  completion_percentage: 100%

## Global Rule: `foundation_errstacks` Error Handling

All VFS types MUST implement `Debug` and use `VfsResult<T>` (`Result<T, ErrorTrace<VfsError>>`) for errors. Tests use `err.current_context()` for typed error matching. See **plan.md §4d** for the full rule.

---

# Feature 11: IPC VFS Daemon

## Overview

Hosts any VfsFileSystem over the IPC bus from spec-34 feature 04. A daemon process listens for filesystem operation requests, executes them against the VFS, and sends responses. A client library (`VfsClient`) implements VfsFileSystem itself — callers use it like any other filesystem, unaware it's remote.

Works everywhere the IPC bus works (Linux, macOS, Windows). No kernel module, no FUSE. Multiple client processes can share one OverlayFileSystem instance.

Builds on existing infrastructure:
- `MessageBox` trait (TypeUuid + Serialize + Deserialize → automatic encode/decode)
- `join()` → `(EndpointSender<T>, EndpointReceiver<R>)` typed channels
- Platform transport: Unix domain sockets (Linux), mach ports (macOS), named pipes (Windows)

## Message Protocol

Define VFS-specific message types implementing MessageBox:

```rust
#[derive(Serialize, Deserialize, TypeUuid)]
#[uuid = "..."]
pub enum VfsRequest {
    // File operations
    Open { path: String, mode: OpenMode, seekable: bool },
    Create { path: String, mode: u32 },
    ReadAt { handle: u64, offset: u64, size: u32 },
    WriteAt { handle: u64, data: Vec<u8>, offset: u64 },
    Seek { handle: u64, pos: SeekFrom },
    Truncate { handle: u64, size: u64 },
    Sync { handle: u64 },
    Close { handle: u64 },

    // Path operations
    Stat { path: String },
    Exists { path: String },
    ReadDir { path: String },
    Mkdir { path: String },
    Remove { path: String },
    Rename { from: String, to: String },
    Chmod { path: String, mode: u32 },
    Symlink { target: String, link: String },
    ReadLink { path: String },

    // Directory handle
    OpenDir { path: String },
    DirList { handle: u64 },
    DirCreateFile { handle: u64, name: String, mode: u32 },
    DirCreateDir { handle: u64, name: String },
    DirRemoveEntry { handle: u64, name: String },
    CloseDir { handle: u64 },

    // Convenience
    ReadFile { path: String },
    WriteFile { path: String, data: Vec<u8> },

    // Capabilities
    Capabilities,
}

#[derive(Serialize, Deserialize, TypeUuid)]
#[uuid = "..."]
pub enum VfsResponse {
    FileHandle(u64),
    DirHandle(u64),
    Data(Vec<u8>),
    Metadata(VfsMetadata),
    DirEntries(Vec<VfsDirEntry>),
    Exists(bool),
    Position(u64),
    Written(usize),
    Read(usize),
    Capabilities(VfsCapabilities),
    Ok,
    Error(String),  // serialized VfsError
}
```

When Arrow serialization (feature 12) is available, bulk data transfers (ReadAt response, WriteAt payload, ReadFile response, DirEntries) can use Arrow IPC format for zero-copy.

### IPC Bus Integration

Uses the existing spec-34 IPC bus infrastructure:

- **`join::<T, R>(options, timeout)`** — returns `(EndpointSender<T>, EndpointReceiver<R>)`. Daemon joins as server (controller_affinity=true), client joins as client.
- **`MessageBox` trait** — blanket impl for `TypeUuid + Serialize + Deserialize + Send + 'static`. Use `#[derive(MessageBox)]` on the VfsRequest/VfsResponse enums (each variant wraps a TypeUuid type), or use the blanket impl directly with `#[derive(TypeUuid)]` on the enums themselves.
- **Message encoding** — bincode via the blanket impl. VfsRequest/VfsResponse encode/decode automatically.
- **Options** — `Options { identifier, label, controller_affinity, token }`. Daemon sets `controller_affinity = true` to register the bus. Client sets `controller_affinity = false` to look up and connect.

```rust
// Daemon side
let opts = Options { identifier: "foundation-vfs".into(), controller_affinity: true, ..Default::default() };
let (tx, mut rx) = join::<VfsResponse, VfsRequest>(opts, None)?;
loop {
    let msg = rx.recv(None)?;
    let response = dispatch(msg.payload, &fs);
    tx.send(Message::new(msg.selector, response))?;
}

// Client side
let opts = Options { identifier: "foundation-vfs".into(), controller_affinity: false, ..Default::default() };
let (tx, mut rx) = join::<VfsRequest, VfsResponse>(opts, Some(Duration::from_secs(5)))?;
```

## Tasks

### Message Types (`src/shared/vfs/ipc_messages.rs`)

- [x] Define `VfsRequest` enum with TypeUuid derive
- [x] Define `VfsResponse` enum with TypeUuid derive
- [x] Ensure all payload types (VfsMetadata, VfsDirEntry, OpenMode, etc.) are Serialize + Deserialize
- [x] Tests: roundtrip encode/decode for all variants (via DirectTransport integration tests)

### Daemon Library (`src/shared/vfs/ipc_daemon.rs`)

- [x] Define `VfsDaemon<F: VfsFileSystem>` struct: wraps VfsFileSystem + handle table
- [x] Implement request dispatch: match VfsRequest → call VfsFileSystem method → return VfsResponse
- [x] File handle table: `HashMap<u64, F::File>` — allocate handle on Open, remove on Close
- [x] Directory handle table: `HashMap<u64, F::Directory>` — same lifecycle
- [x] Implement `VfsDaemon::listen(fs, options)` — join IPC bus as server, loop on recv/dispatch/send
- [x] Handle client disconnect gracefully (close all handles for that client)

### Client Library (`src/shared/vfs/ipc_client.rs`)

- [x] Define `VfsClient` struct: holds EndpointSender + EndpointReceiver
- [x] Implement `VfsFileSystem` for `VfsClient`: each method sends VfsRequest, awaits VfsResponse
- [x] Implement `VfsFile` for `RemoteFile`: holds handle u64, sends ReadAt/WriteAt/Sync/Close
- [x] Implement `VfsDirectory` for `RemoteDirectory`: holds handle u64, sends DirList/DirCreateFile/etc
- [x] Implement `VfsClient::connect(options)` — join IPC bus as client

### Binary (`src/bin/foundation-fs-daemon.rs` or separate crate)

- [ ] CLI argument parsing: root directory, delta store type, listen address/path
- [ ] Instantiate VfsFileSystem (e.g., OverlayFileSystem<NativeFs, DirectoryDelta>)
- [ ] Start VfsDaemon, run until signal

### Tests

- [x] Test: daemon + client in-process — open, read, write, close roundtrip (DirectTransport)
- [x] Test: stat through client matches direct stat (DirectTransport)
- [x] Test: multiple clients sharing one daemon (DirectTransport with shared NativeFs)
- [x] Test: directory operations — mkdir, list, create file in subdir
- [x] Test: create and remove
- [x] Test: rename
- [x] Test: symlink and readlink
- [x] Test: capabilities
- [x] Test: inode operations (inode extraction works; stat_by_inode requires inode-native backend)
- [x] Test: ReadFile/WriteFile convenience methods
- [x] IPC bus transport: `ClientTransport::connect()` and `DaemonTransport::serve()` over real bus
- [x] IPC daemon loop: `run_daemon_loop()` with shutdown signal

## Verification

- [x] 10 integration tests pass (`cargo test --features vfs-ipc,vfs-native --test ipc_vfs`)
- [ ] Binary starts, client connects, filesystem operations work
- [x] `cargo check --features vfs-ipc` passes
