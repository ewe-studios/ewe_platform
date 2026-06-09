# Example: VFS SQLite Delta (LD_PRELOAD)

## Purpose

Demonstrates the LD_PRELOAD VFS shim — a shared library that intercepts libc filesystem calls (`open`, `read`, `write`, `stat`, etc.) and redirects them to a virtual filesystem backed by a SQLite delta store. This example proves persistence: files written in one process survive and are readable in a completely separate process, all without modifying the application binary.

## Prerequisites

- Linux x86_64 or aarch64 (ptrace/interception support)
- GCC or Clang for compiling the test C program
- Rust toolchain with workspace dependencies
- Feature flags: `vfs-preload` and `vfs-sqlite`

## How to Run

```bash
cargo run -p foundation_nativeapis --features "vfs-preload,vfs-sqlite" --example vfs_sqlite_delta
```

## Architecture

The example consists of three components:

1. **LD_PRELOAD shim** (`libfoundation_nativeapis.so`) — A shared library built with the `vfs-preload` feature. It uses `LD_PRELOAD` to inject itself before libc in the dynamic linker's resolution order, intercepting calls like:
   - `open()` → Routes to VFS overlay
   - `read()` / `write()` → Delegates to delta store
   - `stat()` → Queries VFS metadata
   - `readdir()` → Lists merged directory entries

2. **SQLite delta store** — Files written through the shim are persisted in a SQLite database (`/tmp/vfs-sqlite-example.db`). The delta store handles chunked blob storage, whiteout tracking, and transactional consistency.

3. **Test program** — A small C program compiled on-the-fly that performs standard POSIX I/O. It demonstrates:
   - Writing virtual files
   - Reading them back
   - Creating multiple files
   - Simulating a "restart" by running a new process that reads the same virtual paths

The Rust driver orchestrates the demo by:
- Compiling the C test program
- Running it multiple times with `LD_PRELOAD` set
- Passing environment variables to configure the VFS prefix (`/virtual`) and delta backend (`sqlite`)
- Displaying output and shim log messages

## Expected Output

```
=== LD_PRELOAD VFS Shim: SQLite Delta Store ===

Shim: /path/to/target/debug/libfoundation_nativeapis.so
SQLite: /tmp/vfs-sqlite-example.db

┌─────────────────────────────────────────────────────────┐
│ Step 1: Write virtual files (SQLite delta)              │
└─────────────────────────────────────────────────────────┘

Wrote 30 bytes to /virtual/persistent.txt

┌─────────────────────────────────────────────────────────┐
│ Step 2: Read them back (same process)                   │
└─────────────────────────────────────────────────────────┘

Read from /virtual/persistent.txt:
  Hello from SQLite delta store!

┌─────────────────────────────────────────────────────────┐
│ Step 3: Write multiple files                            │
└─────────────────────────────────────────────────────────┘

  wrote /virtual/notes.txt
  wrote /virtual/config.ini

┌─────────────────────────────────────────────────────────┐
│ Step 4: Simulate restart (new process, same DB)         │
│ This proves persistence — data survives process exit   │
└─────────────────────────────────────────────────────────┘

Read from /virtual/persistent.txt:
  Hello from SQLite delta store!

┌─────────────────────────────────────────────────────────┐
│ Step 5: Stat a virtual file                             │
└─────────────────────────────────────────────────────────┘

/virtual/persistent.txt: size=30 bytes, mode=644

┌─────────────────────────────────────────────────────────┐
│ SQLite database on disk                                 │
└─────────────────────────────────────────────────────────┘

  24.0 KB  /tmp/vfs-sqlite-example.db

┌─────────────────────────────────────────────────────────┐
│ Try with Turso (remote libSQL)                          │
│                                                         │
│  export FOUNDATION_VFS_TURSO_URL=libsql://your-db...   │
│  export FOUNDATION_VFS_TURSO_TOKEN=your-token          │
│  FOUNDATION_VFS_DELTA=turso                            │
└─────────────────────────────────────────────────────────┘
```

## Key APIs Demonstrated

- `LD_PRELOAD` — Dynamic linker trick to intercept libc calls
- `FOUNDATION_VFS_PREFIX` — Configure the virtual mount point (e.g., `/virtual`)
- `FOUNDATION_VFS_DELTA` — Select the delta backend (`sqlite` or `turso`)
- `FOUNDATION_VFS_DELTA_PATH` — Path to the SQLite database file
- `open(path, flags, mode)` — Intercepted and routed to VFS
- `read(fd, buf, count)` — Reads from virtual files via delta store
- `write(fd, buf, count)` — Writes to virtual files via delta store
- `stat(path, &st)` — Returns VFS metadata as `struct stat`
- `readdir(dir)` — Lists merged directory contents

## Where to Use This

- **Legacy application virtualization** — Give old apps modern FS features without recompilation
- **Sandboxing** — Confine apps to a virtual filesystem that persists but doesn't pollute the host
- **Transparent persistence** — Make CLI tools stateful by persisting their output directories
- **Testing isolation** — Each test gets its own virtual FS, no cleanup needed
- **Container escape prevention** — Lock processes into a virtual root that can't access real paths
- **Audit logging** — Record every filesystem operation for compliance and debugging

## Environment Variables

| Variable | Purpose | Example |
|----------|---------|---------|
| `LD_PRELOAD` | Load the shim before libc | `target/debug/libfoundation_nativeapis.so` |
| `FOUNDATION_VFS_PREFIX` | Virtual mount point | `/virtual` |
| `FOUNDATION_VFS_DELTA` | Delta backend type | `sqlite` |
| `FOUNDATION_VFS_DELTA_PATH` | SQLite DB path | `/tmp/vfs.db` |

## Related

- Feature spec: `specifications/37-overlay-vfs/features/09-ptrace-interceptor/feature.md`
- Source: `src/native/vfs/ptrace/nix_backend.rs` (interception logic)
- Preload shim: `src/native/vfs/preload/mod.rs`
- Delta store: `src/shared/vfs/libsql_delta.rs`
