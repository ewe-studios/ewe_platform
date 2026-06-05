# Feature 19: FjallFs / FjallDelta — Start Guide

## Prerequisites

- Feature 01 (core-traits) must be complete — VfsFileSystem and DeltaStore traits
- Rust toolchain with cargo
- Understanding of LSM-tree concepts (keyspaces, prefix scans, bloom filters)

## Quick Start

1. Add feature flag to `Cargo.toml`:
   ```toml
   vfs-fjall = ["vfs", "dep:fjall", "dep:cacache", "dep:ssri", "dep:scru128", "dep:bincode"]
   ```

2. Start with `config.rs` — define `FjallVfsConfig` and defaults
3. Then `keyspaces.rs` — key encoding/decoding, `FjallDentry` struct
4. Then `path_index.rs` — hierarchical prefix generation and resolution
5. Then `content.rs` — inline vs chunked content logic
6. Then `file_handle.rs` + `directory.rs` — VFS handle types
7. Then `gc.rs` — GC worker thread
8. Then `mod.rs` — wire everything together, implement traits
9. Tests last — build up from unit tests to integration tests

## Implementation Order

```
config.rs → keyspaces.rs → path_index.rs → content.rs → file_handle.rs → directory.rs → gc.rs → mod.rs → tests
```

## Key Design Decisions

- **Four keyspaces**: inodes, idx_path, chunks, whiteouts — each with single concern
- **SCRU128 inodes**: time-ordered, globally unique, monotonic
- **Hierarchical prefix indexing**: O(d) write amplification for instant prefix scan queries
- **Inline small files**: < 4KB stored directly in inode value (single point read)
- **Version-tagged chunks**: enables lazy GC without reference counting
- **cacache for content**: automatic dedup, SRI integrity, no chunk management complexity
