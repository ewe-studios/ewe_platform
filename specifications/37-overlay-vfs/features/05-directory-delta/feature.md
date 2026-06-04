---
feature_name: "DirectoryDelta"
description: "DirectoryDelta — shadow directory-based DeltaStore. Writes go to a .snapshot/ directory as real files. Whiteouts stored as sentinel files (.whiteout.<name>). Simple, inspectable, no extra dependencies."
status: "pending"
priority: "high"
phase: 2
created: 2026-06-04
updated: 2026-06-04
dependencies:
  - "01-core-traits"
  - "04-native-fs"
tasks:
  completed: 0
  uncompleted: 11
  total: 11
  completion_percentage: 0%
---

# Feature 05: DirectoryDelta

## Overview

The simplest real DeltaStore — mirrors the filesystem structure under a shadow directory (e.g., `.snapshot/`). Modified and new files are stored as real files. Whiteouts are sentinel files named `.whiteout.<filename>`. The directory is fully inspectable with standard tools (`ls`, `cat`, etc).

## Tasks

### Core (`src/native/vfs/dir_delta.rs`)

- [ ] Define `DirectoryDelta` struct: wraps a `NativeFs` pointed at the shadow directory
- [ ] Implement `VfsFileSystem` for `DirectoryDelta`: delegates to inner NativeFs
- [ ] Implement `DeltaStore` for `DirectoryDelta`:
  - `add_whiteout(path)`: create sentinel file `.whiteout.<basename>` in parent dir
  - `is_whiteout(path)`: check for sentinel file, also check parent path whiteouts for inheritance
  - `remove_whiteout(path)`: delete sentinel file
  - `list_whiteouts(dir)`: scan for `.whiteout.*` files in directory
  - `flush()`: `sync_all` on directory fds
  - `reset()`: remove entire shadow directory contents
- [ ] Implement `DirectoryDelta::new(shadow_path)` — create shadow dir if not exists
- [ ] Ensure parent directories in shadow are created on write (mirror structure)
- [ ] Metadata preservation: store file metadata alongside content (sidecar `.meta.<name>` files or xattrs)

### Tests (`tests/dir_delta_test.rs`)

- [ ] Test: write creates file in shadow directory
- [ ] Test: whiteout creates sentinel file, is_whiteout returns true
- [ ] Test: reset clears shadow directory
- [ ] Test: end-to-end with OverlayFileSystem<NativeFs, DirectoryDelta>

## Verification

- Tests pass
- Shadow directory structure is inspectable with `ls -la`
- `cargo check -p foundation_nativeapis --features vfs-native` passes
