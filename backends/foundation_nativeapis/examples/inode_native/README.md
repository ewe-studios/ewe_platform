# Example: Inode Native

## Purpose

Demonstrates the inode-based VFS API in `MemoryFs`, showing how every file and directory is assigned a unique inode number, how to perform reverse lookups from inode back to path, and how inodes are preserved across rename operations. This example is essential for understanding content-addressable filesystem patterns and hard-link-like behavior.

## Prerequisites

- Linux or macOS (Unix-style inode semantics)
- Rust toolchain with workspace dependencies
- Feature flag: `vfs`

## How to Run

```bash
cargo run -p foundation_nativeapis --features vfs --example inode_native
```

## Architecture

The example uses `MemoryFs` — an in-memory implementation of the `VfsFileSystem` trait — to demonstrate three core inode-related capabilities:

1. **Inode allocation** — Every call to `create()` or `mkdir()` assigns a unique, monotonically increasing inode number. The root directory always starts at inode 1.

2. **Reverse lookup** — `path_by_inode(ino)` resolves an inode number back to its canonical path. This is useful for deduplication, link tracking, and forensic analysis.

3. **Inode-preserving stat** — `stat_by_inode(ino)` returns metadata without needing the path, enabling efficient bulk queries when you only have inode references.

The code creates a small directory tree, queries inodes, performs a rename, and verifies that the inode remains stable while the path changes.

## Expected Output

```
=== Inode-Native VFS Example ===

Root inode: 1
Inodes: /projects=2, app.rs=3, lib.rs=4
path_by_inode(3) = "/projects/app.rs"
stat_by_inode: size=12, type=File

Rename /projects/app.rs → /projects/main.rs
Inode before: 3, after: 3 (preserved: true)
path_by_inode(3) now = "/projects/main.rs"

/projects entries:
  File "main.rs" inode=3
  File "lib.rs" inode=4

=== Done ===
```

## Key APIs Demonstrated

- `VfsFileSystem::inode(path)` — Resolve a path to its inode number
- `VfsFileSystem::path_by_inode(ino)` — Reverse lookup: inode to canonical path
- `VfsFileSystem::stat_by_inode(ino)` — Get metadata by inode without needing the path
- `VfsFileSystem::rename(from, to)` — Rename a file while preserving its inode
- `VfsDirectory::list()` — List directory entries with their inode numbers

## Where to Use This

- **Content-addressable storage** — Track files by their content hash mapped to inode
- **Deduplication systems** — Detect when two paths point to the same underlying data
- **Build systems** — Incremental compilation based on inode-stable file identity across renames
- **Forensic tools** — Audit trail reconstruction when filenames change but identity persists
- **Hard link simulation** — Understand how multiple paths could reference the same inode (not yet implemented, but the pattern is demonstrated)

## Related

- Feature spec: `specifications/37-overlay-vfs/features/01-memory-fs/feature.md`
- Source: `src/shared/vfs/memory_fs.rs`
- Trait definition: `src/shared/vfs/traits.rs` (`VfsFileSystem`)
