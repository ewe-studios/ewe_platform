# Feature 19: FjallFs / FjallDelta — Start Guide

## Prerequisites

- Feature 01 (core-traits) must be complete — VfsFileSystem and DeltaStore traits
- Rust toolchain with cargo
- Understanding of LSM-tree concepts (keyspaces, prefix scans, bloom filters)
- Read the SCRU128 ↔ u64 Version Bridge section in feature.md — this is the key gap resolved since spec drafting

## Quick Start

1. Add feature flag to `Cargo.toml`:
   ```toml
   vfs-fjall = ["vfs", "dep:fjall", "dep:cacache", "dep:ssri", "dep:scru128", "dep:bincode"]
   ```

2. Start with `version.rs` — the SCRU128 ↔ u64 bridge (resolves the trait gap first)
3. Then `config.rs` — define `FjallVfsConfig` and defaults
4. Then `keyspaces.rs` — key encoding/decoding, `FjallDentry` struct
5. Then `path_index.rs` — hierarchical prefix generation and resolution
6. Then `content.rs` — inline vs chunked content logic with cacache
7. Then `file_handle.rs` + `directory.rs` — VFS handle types
8. Then `gc.rs` — GC worker thread
9. Then `mod.rs` — wire everything together, implement traits (sync wrappers around sync fjall/cacache)
10. Tests last — build up from unit tests to integration tests

## Implementation Order

```
version.rs → config.rs → keyspaces.rs → path_index.rs → content.rs → file_handle.rs → directory.rs → gc.rs → mod.rs → tests
```

**Start with `version.rs`** — this resolves the version type mismatch between the DeltaStore trait (`u64`) and the internal storage (`Scru128Id`). Get the packing/unpacking functions + ordering guarantee tests passing before touching fjall.

## Key Design Decisions

- **Four keyspaces**: inodes, idx_path, chunks, whiteouts — each with single concern
- **SCRU128 inodes**: time-ordered, globally unique, monotonic
- **Hierarchical prefix indexing**: O(d) write amplification for instant prefix scan queries
- **Inline small files**: < 4KB stored directly in inode value (single point read)
- **Version-tagged chunks**: enables lazy GC without reference counting
- **cacache for content**: automatic dedup, SRI integrity, no chunk management complexity
- **u64 ↔ Scru128Id bridge**: pack_version/unpack_version preserve ordering, enable round-trip with DeltaStore trait

## Async Strategy

fjall and cacache are synchronous crates. Write core logic synchronously. Wrap each `VfsFileSystem` / `DeltaStore` method body in valtron's blocking executor. Do not introduce a tokio dependency. The async surface exists so valtron can compose VFS operations with other async work.

## Known Gaps (Resolved)

- ~~DeltaStore uses `u64`, spec uses `Scru128Id`~~ → `version.rs` bridges with `pack_version` / `unpack_version`
- ~~`VfsMetadata.version` is `u64`~~ → `From<&FjallDentry> for VfsMetadata` unpacks SCRU128 → u64
- ~~Feature flag missing from plan.md~~ → `vfs-fjall` added to Phase 3 feature flags
