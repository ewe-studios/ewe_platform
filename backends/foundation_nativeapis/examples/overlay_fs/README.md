# Example: OverlayFs

## Purpose

Demonstrates the overlay filesystem pattern using `OverlayFileSystem` — a union mount that layers a writable `MemoryDelta` over a read-only base `MemoryFs`. This example covers copy-on-write (CoW) semantics, whiteout-based deletion, merged directory listing, and delta lifecycle management (flush/reset). It's the core pattern for implementing mutable snapshots over immutable bases.

## Prerequisites

- Rust toolchain with workspace dependencies
- Feature flag: `vfs`

## How to Run

```bash
cargo run -p foundation_nativeapis --features vfs --example overlay_fs
```

## Architecture

The overlay filesystem composes two layers:

1. **Base layer** — A read-only `MemoryFs` containing initial content. This represents a "committed" snapshot or template.

2. **Delta layer** — A writable `MemoryDelta` that stores modifications. Initially empty, it accumulates:
   - **New files** — Created directly in the delta
   - **Copy-on-write entries** — When a base file is modified, it's copied to the delta with changes applied, shadowing the base version
   - **Whiteouts** — Tombstone markers that hide base files without modifying them

3. **Merged view** — `OverlayFileSystem` presents a unified namespace where:
   - Delta entries take precedence over base entries
   - Whiteouts hide base entries
   - Directory listings merge both layers (delta wins, whiteouts exclude)

The example demonstrates:
- **Pass-through reads** — Reading unmodified base files
- **Copy-on-write writes** — Modifying a base file creates a delta override
- **New file creation** — Writing to the delta layer
- **Whiteout deletion** — Removing a base file adds a whiteout marker
- **Merged directory listing** — Seeing base + delta entries minus whiteouts
- **Delta inspection** — Querying whiteout status
- **Lifecycle management** — Flush persists delta state; reset clears it

## Expected Output

```
=== OverlayFileSystem Example ===

Base layer: 3 files in /src + /README.md
Read base file: "fn main() {}"
After CoW write: "fn main() { println!(\"Hello!\"); }"
Overlay version: 1
Created /tests/test_main.rs in delta layer
Deleted /src/lib.rs (whiteout in delta)
Base still has lib.rs: true

Merged /src listing:
  File "main.rs"
  File "lib.rs"  # hidden by whiteout but still in base

Delta has whiteout for /src/lib.rs: Some(WhiteoutInfo { .. })
Delta flushed
Delta reset — overlay is back to base state
/src/lib.rs visible again after delta reset

=== Done ===
```

## Key APIs Demonstrated

- `OverlayFileSystem::new(base, delta)` — Compose an overlay from a base FS and delta store
- `OverlayFileSystem::base()` — Access the read-only base filesystem
- `OverlayFileSystem::delta()` — Access the writable delta layer
- `OverlayFileSystem::version()` — Get the current overlay version number
- `DeltaStore::add_whiteout(path, inode)` — Mark a file as deleted in the delta
- `DeltaStore::is_whiteout(path)` — Check if a path has a whiteout marker
- `DeltaStore::list_whiteouts(prefix)` — Enumerate whiteouts under a prefix
- `DeltaStore::remove_whiteout(path)` — Remove a whiteout (undelete)
- `DeltaStore::flush()` — Persist delta state
- `DeltaStore::reset()` — Clear all delta changes, reverting to base

## Where to Use This

- **Container image layers** — Implement UnionFS/AUFS-style container overlays
- **Snapshot isolation** — Fork a base filesystem, make changes, commit or discard
- **Version control** — Model commits as delta flushes, branches as separate deltas
- **Package managers** — Overlay installed packages with user modifications
- **Testing sandboxes** — Start from a known base, isolate test mutations, reset cleanly
- **Stateful plugins** — Allow plugins to modify a read-only host filesystem safely

## Related

- Feature spec: `specifications/37-overlay-vfs/features/04-overlay-fs/feature.md`
- Source: `src/shared/vfs/overlay_fs.rs`
- Delta store trait: `src/shared/vfs/delta_store.rs` (`DeltaStore`)
- Base trait: `src/shared/vfs/traits.rs` (`VfsFileSystem`)
