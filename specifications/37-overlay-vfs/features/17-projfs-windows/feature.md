---
feature_name: "ProjFS (Windows Projected File System)"
description: "Windows 10+ native projection — provider callbacks materialize files on demand, auto-promotes placeholders to real files on write. Maps directly to OverlayFileSystem model."
status: "deferred"
priority: "medium"
phase: 6
created: 2026-06-04
updated: 2026-06-04
dependencies:
  - "01-core-traits"
tasks:
  completed: 0
  uncompleted: 0
  total: 0
  completion_percentage: 0%
---

# Feature 17: ProjFS (Windows Projected File System) — **DEFERRED**

> **Status: Deferred** — tombstone for later exploration. Conceptually maps to our OverlayFileSystem model (base = projections, delta = hydrated files), but requires Windows 10+ SDK binding work (`projfs-sys` from SDK headers). No cross-platform Rust crate exists yet.

## Overview

Windows 10+ built-in API for on-demand file materialization. You register a provider callback; Windows intercepts file operations on a directory tree and calls your provider to materialize files lazily. Placeholders (projections) sit on disk until accessed, then your callback hydrates them.

GVFS (Git Virtual File System) uses this to project massive repos without full checkout.

## How It Maps to OverlayFileSystem

| OverlayFileSystem | ProjFS |
|-------------------|--------|
| Base (read-only) files | Placeholder files (projected, on-demand) |
| Delta (writable) layer | Real/hydrated files that shadow placeholders |
| CoW on write | Windows auto-promotes placeholder to real file |
| Whiteout/delete | Remove real file, keep placeholder (or remove entirely) |

## Tasks (Not Started — Deferred)

- [ ] Generate `projfs-sys` bindings from Windows SDK headers
- [ ] Implement `ProjFsMount` wrapper that maps `VfsFileSystem` to ProjFS provider callbacks
- [ ] Handle placeholder creation, hydration, and synchronization
- [ ] Test with Windows external tools (`cmd`, `PowerShell`, etc.)
- [ ] Verify auto-promotion on write matches our CoW semantics

## Verification (Deferred)

- `dir` and PowerShell show projected files in mount directory
- Opening a projected file triggers provider callback and returns content
- Writing to a projected file promotes it to real file in delta
- Unmodified base files remain as placeholders (no disk I/O)
