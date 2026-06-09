# Example: VFS Turso Delta

## Purpose

Demonstrates how to use Turso (distributed libSQL) as a cloud-backed VFS delta store, enabling virtual filesystem data to sync across machines via a remote database. This example covers setup instructions, environment variable configuration, and the programmatic API pattern for integrating Turso into overlay filesystems. It also documents current limitations with the LD_PRELOAD shim.

## Prerequisites

- Rust toolchain with workspace dependencies
- Feature flag: `vfs-turso`
- A Turso account and database (free tier available at https://turso.tech)
- Optional: Turso CLI for database creation (`curl -sSf https://getting.turso.sh | sh`)

## How to Run

```bash
# Without credentials: shows setup instructions
cargo run -p foundation_nativeapis --features vfs-turso --example vfs_turso_delta

# With credentials:
export FOUNDATION_VFS_TURSO_URL=libsql://your-database.turso.io
export FOUNDATION_VFS_TURSO_TOKEN=your-auth-token
cargo run -p foundation_nativeapis --features vfs-turso --example vfs_turso_delta
```

## Architecture

Turso is a managed, distributed version of libSQL designed for edge computing and multi-region applications. The `TursoDelta` implementation stores VFS operations (file creates, writes, deletes) in a remote Turso database, making them accessible from any machine with the same database URL and auth token.

The example handles two modes:

1. **Setup mode** (no credentials) — Prints step-by-step instructions for:
   - Installing the Turso CLI
   - Creating a database
   - Generating an auth token
   - Setting environment variables

2. **Connected mode** (with credentials) — Shows:
   - Connection details
   - Programmatic API usage pattern
   - Current limitations with LD_PRELOAD shim

### Programmatic API Pattern

When using Turso in application code (not via LD_PRELOAD), the pattern is:

```rust
use foundation_nativeapis::shared::vfs::{
    turso_delta::TursoDelta,
    overlay_fs::OverlayFileSystem,
    native_fs::NativeFs,
    VfsFileSystem,
};

// Create overlay with Turso delta
let base = NativeFs::new("/path/to/project").unwrap();
let delta = TursoDelta::new("libsql://your-db.turso.io", Some("token")).unwrap();
let overlay = OverlayFileSystem::new(base, delta);

// All writes go to Turso — accessible from any machine
overlay.write_file("/virtual/file.txt", b"hello").unwrap();
let data = overlay.read_file("/virtual/file.txt").unwrap();
```

### LD_PRELOAD Limitation

The LD_PRELOAD shim currently cannot initialize the valtron runtime required by `TursoDelta`. This is a known limitation tracked in the feature spec. When resolved, the shim will support:

```bash
LD_PRELOAD=target/debug/libfoundation_nativeapis.so \
  FOUNDATION_VFS_PREFIX=/virtual \
  FOUNDATION_VFS_DELTA=turso \
  FOUNDATION_VFS_TURSO_URL=libsql://your-db.turso.io \
  FOUNDATION_VFS_TURSO_TOKEN=your-token \
  /bin/bash
```

## Expected Output (Setup Mode)

```
=== Turso Delta Store Example ===

Turso credentials not set.

┌─────────────────────────────────────────────────────────┐
│ Step 1: Install Turso CLI                               │
└─────────────────────────────────────────────────────────┘

  curl -sSf https://getting.turso.sh | sh
  turso auth login

┌─────────────────────────────────────────────────────────┐
│ Step 2: Create a database                               │
└─────────────────────────────────────────────────────────┘

  turso db create my-vfs

┌─────────────────────────────────────────────────────────┐
│ Step 3: Get connection details                          │
└─────────────────────────────────────────────────────────┘

  turso db show my-vfs --url
  turso db tokens create my-vfs

┌─────────────────────────────────────────────────────────┐
│ Step 4: Set environment variables                       │
└─────────────────────────────────────────────────────────┘

  export FOUNDATION_VFS_TURSO_URL=libsql://my-vfs-user.turso.io
  export FOUNDATION_VFS_TURSO_TOKEN=eyJhbG...

┌─────────────────────────────────────────────────────────┐
│ Step 5: Run the example                                 │
└─────────────────────────────────────────────────────────┘

  cargo run -p foundation_nativeapis \
    --features vfs-turso \
    --example vfs_turso_delta
```

## Expected Output (Connected Mode)

```
=== Turso Delta Store Example ===

Connecting to Turso: libsql://your-db.turso.io

┌─────────────────────────────────────────────────────────┐
│ Turso Connection                                        │
└─────────────────────────────────────────────────────────┘

Turso URL:  libsql://your-db.turso.io
Token:      eyJhbG...

┌─────────────────────────────────────────────────────────┐
│ Programmatic API Pattern                                │
└─────────────────────────────────────────────────────────┘

To use Turso as a VFS delta store in your code:

  use foundation_nativeapis::shared::vfs::{
      turso_delta::TursoDelta,
      overlay_fs::OverlayFileSystem,
      native_fs::NativeFs,
      VfsFileSystem,
  };

  // Create overlay with Turso delta
  let base = NativeFs::new("/path/to/project").unwrap();
  let delta = TursoDelta::new("libsql://your-db.turso.io", Some("your-token")).unwrap();
  let overlay = OverlayFileSystem::new(base, delta);

  // All writes go to Turso — accessible from any machine
  overlay.write_file("/virtual/file.txt", b"hello").unwrap();
  let data = overlay.read_file("/virtual/file.txt").unwrap();

┌─────────────────────────────────────────────────────────┐
│ LD_PRELOAD Shim (future)                                │
└─────────────────────────────────────────────────────────┘

Turso support through the LD_PRELOAD shim requires initializing
the valtron runtime in the shim process. This is a known
limitation — tracked in the feature spec.

  # When available:
  LD_PRELOAD=target/debug/libfoundation_nativeapis.so \
    FOUNDATION_VFS_PREFIX=/virtual \
    FOUNDATION_VFS_DELTA=turso \
    FOUNDATION_VFS_TURSO_URL=libsql://your-db.turso.io \
    FOUNDATION_VFS_TURSO_TOKEN=your-token \
    /bin/bash
```

## Key APIs Demonstrated

- `TursoDelta::new(url, token)` — Create a Turso-backed delta store
- `OverlayFileSystem::new(base, delta)` — Compose with Turso delta for cloud-synced overlays
- `VfsFileSystem::write_file(path, data)` — Writes sync to Turso database
- `VfsFileSystem::read_file(path)` — Reads fetch latest data from Turso
- `FOUNDATION_VFS_TURSO_URL` — Env var for Turso database URL
- `FOUNDATION_VFS_TURSO_TOKEN` — Env var for Turso auth token

## Where to Use This

- **Multi-machine development** — Share virtual filesystem state across team members
- **Edge computing** — Deploy apps that read/write VFS data synced from a central Turso DB
- **Disaster recovery** — VFS data is automatically backed up in Turso's distributed storage
- **CI/CD pipelines** — Seed build artifacts into a Turso DB, share across runners
- **Collaborative editing** — Multiple users edit files in a shared virtual workspace
- **IoT fleets** — Push config updates to device fleets via Turso, read through VFS shim

## Turso Setup Checklist

1. **Install CLI**: `curl -sSf https://getting.turso.sh | sh`
2. **Authenticate**: `turso auth login`
3. **Create DB**: `turso db create my-vfs`
4. **Get URL**: `turso db show my-vfs --url`
5. **Create token**: `turso db tokens create my-vfs`
6. **Set env vars**:
   ```bash
   export FOUNDATION_VFS_TURSO_URL=libsql://my-vfs-user.turso.io
   export FOUNDATION_VFS_TURSO_TOKEN=eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...
   ```

## Related

- Feature spec: `specifications/37-overlay-vfs/features/10-turso-delta/feature.md`
- Source: `src/shared/vfs/turso_delta.rs`
- Delta store trait: `src/shared/vfs/delta_store.rs` (`DeltaStore`)
- Turso docs: https://docs.turso.tech
