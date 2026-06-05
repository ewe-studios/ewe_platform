---
feature_name: "DirectoryDelta"
description: "DirectoryDelta — shadow directory-based DeltaStore. Writes go to a shadow directory as real files. Whiteouts stored as .wh.<name> sentinel files with version content. In-memory cache for fast lookups."
status: "done"
priority: "high"
phase: 2
created: 2026-06-04
updated: 2026-06-05
dependencies:
  - "01-core-traits"
  - "04-native-fs"
tasks:
  completed: 8
  uncompleted: 0
  total: 8
  completion_percentage: 100%
---

# Feature 05: DirectoryDelta

## Overview

The simplest persistent DeltaStore — mirrors the filesystem structure under a shadow directory. Modified and new files are stored as real files. Whiteouts are `.wh.<basename>` sentinel files containing the version number. Uses NativeFs internally. Directory listing filters out sentinel files via FilteredDirectory wrapper.

## Implemented Files

| File | Contents |
|------|----------|
| `src/native/vfs/dir_delta.rs` | DirectoryDelta, FilteredDirectory, whiteout scanning/creation, sentinel path validation |
| `tests/vfs_dir_delta_tests.rs` | 9 tests covering shadow writes, whiteout sentinels, inheritance, scanning, reset, filtering, end-to-end overlay |

## Key Design Decisions

**In-memory whiteout cache:** `RwLock<HashMap<String, u64>>` built at construction by scanning for `.wh.*` files. Updated on add/remove. Avoids filesystem round-trips for every `is_whiteout` check.

**Sentinel path security:** `whiteout_sentinel_path` validates the resolved sentinel path `starts_with(shadow_root)` — defense-in-depth beyond normalize_vfs_path's `..` rejection.

**FilteredDirectory:** Wraps NativeDirectory, hides `.wh.*` sentinels from `list()` and `get_entry()`. Delegates all other methods unchanged.

**Whiteout format:** `.wh.<basename>` file containing version number as UTF-8 text. Recoverable after restart via scanning.

## Tasks

- [x] DirectoryDelta struct with inner NativeFs + RwLock<HashMap> whiteout cache
- [x] Construction: create shadow dir, scan existing `.wh.*` sentinels recursively
- [x] DeltaStore: add_whiteout (create sentinel + update cache), is_whiteout (cache with ancestor inheritance), remove_whiteout, list_whiteouts, reset
- [x] VfsFileSystem delegation to inner NativeFs
- [x] FilteredDirectory wrapper hiding `.wh.*` from listings
- [x] Sentinel path validation against shadow root
- [x] End-to-end test with OverlayFileSystem<NativeFs, DirectoryDelta>
- [x] 9 tests: shadow write, sentinel creation, cache check, inheritance, scan on construction, reset, listing filter, path traversal rejection, full overlay workflow

## Test Coverage (9 tests)

**Shadow storage (1):** write creates file in shadow directory

**Whiteout ops (4):** sentinel creation with version content, cache check, ancestor inheritance, scan existing sentinels on construction

**Lifecycle (1):** reset clears files and whiteouts

**Listing (1):** directory listing excludes `.wh.*` sentinels

**Security (1):** path traversal in whiteout rejected

**End-to-end (1):** OverlayFileSystem<NativeFs, DirectoryDelta> — read base, create new, CoW modify, delete with whiteout, reset restores base

---

_Created: 2026-06-04 | Completed: 2026-06-05_
