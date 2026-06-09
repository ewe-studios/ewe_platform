# Example: Arrow Serialization

## Purpose

Demonstrates how VFS types (`VfsMetadata`, `VfsDirEntry`) integrate with the Arrow zero-copy serialization layer. This enables batch serialization of directory listings, metadata replication, and analytics pipelines.

## Prerequisites

- Feature flags: `vfs-arrow` (includes `vfs` and `dep:foundation_arrow`)

## How to Run

```bash
cargo run -p foundation_nativeapis --features vfs-arrow --example arrow_serialization
```

## Architecture

The example:

1. Creates sample `VfsMetadata` and `VfsDirEntry` instances
2. Shows how `ArrowSchema` derive provides field mapping
3. Demonstrates batch operations on collections of entries
4. Explains the IPC serialization pipeline

The `foundation_arrow` crate provides `ArrowSchema` derive macros that automatically generate Arrow field schemas for VFS types, enabling:

- **Zero-copy IPC streaming** — Arrow IPC format for bulk metadata transfer
- **Schema introspection** — programmatic access to field names and types
- **Batch operations** — serialize thousands of entries in a single IPC batch

## Expected Output

```
=== Arrow Serialization Example ===

Original VfsMetadata:
  size: 4096
  is_dir: false
  inode: 42
  permissions: 644

Original VfsDirEntry:
  name: main.rs
  inode: 42
  type: File

Arrow schema for VfsMetadata is available via foundation_arrow crate
...
```

## Key APIs Demonstrated

- `VfsArrowSchema` — Arrow schema generation for VFS types
- `VfsMetadata` — file metadata (size, timestamps, permissions, inode)
- `VfsDirEntry` — directory entry (name, inode, type)

## Related

- Feature spec: `specifications/37-overlay-vfs/features/12-arrow-serialization/feature.md`
- Source: `src/shared/vfs/arrow/`, `backends/foundation_arrow/`
