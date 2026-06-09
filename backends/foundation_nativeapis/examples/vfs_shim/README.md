# Example: LD_PRELOAD VFS Shim

## Purpose

Demonstrates the LD_PRELOAD VFS shim — a shared library (`.so`) that intercepts libc filesystem calls (`open`, `read`, `write`, `stat`, etc.) and redirects configured virtual paths through the real VFS stack.

This is the **most transparent** VFS access mechanism — no code changes needed, no FUSE, no kernel modules. Any dynamically linked application gets VFS access automatically.

## Prerequisites

- Feature flags: `vfs-preload` (pulls in `vfs-native`)
- Linux (LD_PRELOAD)
- gcc (for compiling test programs)

## How to Run

```bash
# Run the example (builds the shim, demonstrates all scenarios)
cargo run -p foundation_nativeapis --features vfs-preload --example vfs_shim

# Or use the shim directly:
cargo build -p foundation_nativeapis --features vfs-preload

LD_PRELOAD=target/debug/libfoundation_nativeapis.so \
  FOUNDATION_VFS_PREFIX=/virtual \
  /bin/bash
```

## Architecture

```
Your Application
       │
       ├── open("/virtual/file.txt") ─────┐
       ├── open("/real/path") ──────────┤
       │                                 │
       ▼                                 ▼
  ┌─────────────────────────┐   ┌─────────────────┐
  │  LD_PRELOAD shim (.so)  │   │  real libc open │
  │  intercepts libc calls  │   │  (passthrough)  │
  └───────────┬─────────────┘   └─────────────────┘
              │
              ▼
  ┌─────────────────────────────────────┐
  │  OverlayFileSystem<NativeFs, Delta> │
  │  ├─ base: NativeFs (real directory) │
  │  └─ delta: pluggable store ──────┐  │
  └──────────────────────────────────┼──┘
                                     │
                    ┌────────────────┼────────────────┐
                    ▼                ▼                ▼
              MemoryDelta      LibsqlDelta     DirectoryDelta
              (RAM, fast)      (SQLite, persistent) (files)
```

## Configuration

### Required

| Variable | Description | Example |
|----------|-------------|---------|
| `FOUNDATION_VFS_PREFIX` | Colon-separated virtual path prefixes. Paths starting with these are routed through VFS. | `/virtual` or `/virtual:/sandbox` |

### Optional

| Variable | Description | Default |
|----------|-------------|---------|
| `FOUNDATION_VFS_ROOT` | Base directory for the NativeFs overlay. Virtual paths not found in delta fall through here. | `.` (cwd) |
| `FOUNDATION_VFS_DELTA` | Delta store backend: `memory`, `sqlite`, `turso`, `dir`, `d1`, `r2` | `memory` |
| `FOUNDATION_VFS_DELTA_PATH` | Path for persistent delta stores (sqlite, dir) | `/tmp/vfs-delta.{db,dir}` |

### Backend-specific (only needed for that backend)

| Variable | Backend | Description |
|----------|---------|-------------|
| `FOUNDATION_VFS_TURSO_URL` | turso | libSQL connection URL |
| `FOUNDATION_VFS_TURSO_TOKEN` | turso | Turso auth token |
| `FOUNDATION_VFS_D1_ACCOUNT_ID` | d1 | Cloudflare account ID |
| `FOUNDATION_VFS_D1_API_TOKEN` | d1 | Cloudflare API token |
| `FOUNDATION_VFS_D1_DATABASE_ID` | d1 | D1 database UUID |
| `FOUNDATION_VFS_R2_ACCOUNT_ID` | r2 | Cloudflare account ID |
| `FOUNDATION_VFS_R2_ACCESS_KEY` | r2 | R2 access key ID |
| `FOUNDATION_VFS_R2_SECRET_KEY` | r2 | R2 secret access key |
| `FOUNDATION_VFS_R2_BUCKET` | r2 | R2 bucket name |

## Scenarios

### 1. Sandboxed Development

Run build tools against a virtual overlay — source paths don't change, but writes go to the delta store:

```bash
# Sandbox: all writes to /home/user/project go to memory delta
LD_PRELOAD=target/debug/libfoundation_nativeapis.so \
  FOUNDATION_VFS_PREFIX=/home/user/project \
  FOUNDATION_VFS_ROOT=/home/user/project-real \
  make -C /home/user/project
```

### 2. Testing Without Side Effects

```bash
# Writes to /tmp/test-data go to memory (cleared on exit)
LD_PRELOAD=target/debug/libfoundation_nativeapis.so \
  FOUNDATION_VFS_PREFIX=/tmp/test-data \
  ./my-test-suite
```

### 3. Persistent Virtual Files

```bash
# SQLite delta — survives restart
LD_PRELOAD=target/debug/libfoundation_nativeapis.so \
  FOUNDATION_VFS_PREFIX=/virtual \
  FOUNDATION_VFS_DELTA=sqlite \
  FOUNDATION_VFS_DELTA_PATH=~/.vfs-delta.db \
  /bin/bash

# Directory delta — inspect files with ls/cat
LD_PRELOAD=target/debug/libfoundation_nativeapis.so \
  FOUNDATION_VFS_PREFIX=/virtual \
  FOUNDATION_VFS_DELTA=dir \
  FOUNDATION_VFS_DELTA_PATH=/tmp/vfs-delta \
  /bin/bash
```

### 4. Cloud-Synced Virtual Files

```bash
# Turso (remote SQLite) — share state across machines
LD_PRELOAD=target/debug/libfoundation_nativeapis.so \
  FOUNDATION_VFS_PREFIX=/virtual \
  FOUNDATION_VFS_DELTA=turso \
  FOUNDATION_VFS_TURSO_URL=libsql://your-db.turso.io \
  FOUNDATION_VFS_TURSO_TOKEN=$TURSO_TOKEN \
  /bin/bash
```

### 5. Edge Distribution (Cloudflare D1)

```bash
# D1 (edge SQLite) — serve virtual files from Cloudflare's edge
LD_PRELOAD=target/debug/libfoundation_nativeapis.so \
  FOUNDATION_VFS_PREFIX=/virtual \
  FOUNDATION_VFS_DELTA=d1 \
  FOUNDATION_VFS_D1_ACCOUNT_ID=$CF_ACCOUNT_ID \
  FOUNDATION_VFS_D1_API_TOKEN=$CF_API_TOKEN \
  FOUNDATION_VFS_D1_DATABASE_ID=$D1_DB_ID \
  /bin/bash
```

### 6. Object Storage (Cloudflare R2)

```bash
# R2 (S3-compatible) — store virtual files in cloud object storage
LD_PRELOAD=target/debug/libfoundation_nativeapis.so \
  FOUNDATION_VFS_PREFIX=/virtual \
  FOUNDATION_VFS_DELTA=r2 \
  FOUNDATION_VFS_R2_ACCOUNT_ID=$CF_ACCOUNT_ID \
  FOUNDATION_VFS_R2_ACCESS_KEY=$R2_ACCESS_KEY \
  FOUNDATION_VFS_R2_SECRET_KEY=$R2_SECRET_KEY \
  FOUNDATION_VFS_R2_BUCKET=$R2_BUCKET \
  /bin/bash
```

## Limitations

- **Statically linked binaries** — Not intercepted (Go with raw syscalls, musl-static builds)
- **Raw `syscall()` invocations** — Only libc wrappers are replaced
- **macOS SIP** — `DYLD_INSERT_LIBRARIES` stripped for system binaries under `/usr/bin`, `/usr/sbin`
- **Python/Node internals** — Some runtimes bypass libc (e.g., Python's `io` module uses C internals). Works for `open()` but may not intercept all I/O.

## Key APIs Demonstrated

- `LD_PRELOAD` interposition via `#[unsafe(no_mangle)]` libc function overrides
- `dlsym(RTLD_NEXT, ...)` for real function pointer resolution
- `OverlayFileSystem<NativeFs, Delta>` for transparent overlay
- `DynFs` type erasure for pluggable delta stores at runtime
- `VirtualFdTable` for synthetic FD management

## Related

- Feature spec: `specifications/37-overlay-vfs/features/16-ld-preload-shim/feature.md`
- Source: `src/native/vfs/shim/`
- Delta stores: `src/shared/vfs/memory_delta.rs`, `libsql_delta.rs`, `dir_delta.rs`
