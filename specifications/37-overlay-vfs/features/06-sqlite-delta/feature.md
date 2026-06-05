---
feature_name: "SqliteDelta"
description: "SqliteDelta — SQLite-backed DeltaStore. ACID transactions, single-file storage, queryable. Feature-gated behind vfs-sqlite. Implementations may use CAS or chunk storage internally."
status: "pending"
priority: "medium"
phase: 3
created: 2026-06-04
updated: 2026-06-04
dependencies:
  - "01-core-traits"
tasks:
  completed: 0
  uncompleted: 12
  total: 12
  completion_percentage: 0%
---

# Feature 06: SqliteDelta

## Overview

SQLite-backed DeltaStore for durable, queryable, single-file delta storage. Inspired by AgentFS schema (4KB chunks, inode/dentry/data tables). Feature-gated behind `vfs-sqlite` to avoid adding SQLite as a mandatory dependency.

The implementation owns how it stores data — could use AgentFS-style 4KB chunks, content-addressable blocks, or any other strategy. The trait contract is: present complete files at paths.

## Tasks

### Schema Design

- [ ] Design SQLite schema: files table (path, metadata), data table (path + chunk or blob), whiteout table, directory tracking
- [ ] Consider CAS-friendly schema: content-addressed blocks with path → block references

### Core (`src/native/vfs/sqlite_delta.rs` or `src/shared/vfs/sqlite_delta.rs`)

- [ ] Define `SqliteDelta` struct: wraps SQLite connection
- [ ] Implement `SqliteDelta::new(db_path)` — open/create database, run migrations
- [ ] Implement `SqliteDelta::in_memory()` — for testing
- [ ] Implement `VfsFile` for `SqliteFile`: reads chunks from db, writes chunks to db
- [ ] Implement `VfsFileSystem` for `SqliteDelta`: SQL-backed path operations
- [ ] Implement `DeltaStore` for `SqliteDelta`: whiteout table, flush = WAL checkpoint, reset = drop all rows
- [ ] Implement checksum storage in metadata

### Tests

- [ ] Test: store/load file roundtrip
- [ ] Test: large file chunked storage
- [ ] Test: whiteout add/check/remove via SQL
- [ ] Test: reset clears all data
- [ ] Test: end-to-end with OverlayFileSystem<MemoryFs, SqliteDelta>

## Verification

- Tests pass
- `cargo check -p foundation_nativeapis --features vfs-sqlite` passes
- Database file is inspectable with `sqlite3` CLI
