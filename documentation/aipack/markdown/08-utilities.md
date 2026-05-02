---
title: Utilities — simple-fs, uuid-extra, pretty-sqlite, dinf, htmd, webdev, webtk
---

# Utility Crates

This document covers the smaller utility crates in the collection: filesystem access, UUID encoding, SQLite pretty printing, directory info CLI, HTML-to-Markdown conversion, and web development tools.

## simple-fs — Filesystem API

Source: `rust-simple-fs/src/` — 38 files, ~1263 lines.

A high-level filesystem API with convenient JSON/TOML/YAML serialization, glob-based file listing, and safe removal operations.

### Core Types

| Type | Source | Purpose |
|------|--------|---------|
| `SPath` | `spath.rs` | Path abstraction with normalization |
| `SFile` | `file.rs` | File read/write operations |
| `SDir` | `dir.rs` | Directory operations (create, list, remove) |
| `Smeta` | `common/smeta.rs` | File metadata wrapper |

### Feature Traits

Source: `rust-simple-fs/src/featured/`.

| Feature | Purpose |
|---------|---------|
| `with_json` | Load/save files as JSON, NDJSON streaming |
| `with_toml` | Load/save files as TOML |

```rust
// Load a struct from a JSON file
let config = Config::load_from_json_file("config.json")?;

// Save a struct to a JSON file
config.save_to_json_file("config.json")?;
```

### File Listing

Source: `rust-simple-fs/src/list/`. Glob-pattern file listing with sorting:

```rust
let files = globs_file_iter(&["**/*.rs", "!**/target/**"], &options)?;
```

### Safe Removal

Source: `rust-simple-fs/src/safer/`. Safe file removal with:
- Trash (move to recycle bin) instead of permanent delete
- Guards to prevent accidental deletion of important paths
- Configurable safety checks

**Aha:** The `safer` module prevents catastrophic mistakes like `rm -rf /` by maintaining a whitelist of "safe to remove" paths and rejecting operations on system directories. The trash implementation uses platform-specific APIs (freedesktop.org trash spec on Linux, Trash API on macOS, Recycle Bin on Windows) rather than simple file moves.

### Span Reading

Source: `rust-simple-fs/src/span/`. Read specific line/CSV ranges from files:

```rust
let lines = read_line_spans("file.rs", &[LineSpan { from: 10, to: 20 }])?;
```

### Directory Reshaping

Source: `rust-simple-fs/src/reshape/`. Normalize and collapse directory structures.

## uuid-extra — UUID Encoding

Source: `rust-uuid-extra/src/` — 994 lines.

Base64 and Base58 encoding utilities for UUIDs. Compact representations for URLs and identifiers:

```rust
let uuid = Uuid::new_v4();
let base64 = uuid.to_base64();  // e.g., "ABC123xyz..."
let base58 = uuid.to_base58();  // e.g., "7nW8pQ..."
```

Safe for URL-safe identifiers that are shorter than standard UUID strings.

## pretty-sqlite — SQLite Pretty Printer

Source: `rust-pretty-sqlite/src/` — 207 lines.

Pretty-prints SQLite query results as formatted tables for test and dev output. Uses `rusqlite` for queries and `prettytable` for formatting.

## dinf — Directory Info CLI

Source: `rust-dinf/src/` — 572 lines.

Command-line tool for directory information:
- File counts by type
- Total size
- Directory tree visualization
- Configurable depth and patterns

## htmd — HTML to Markdown

Source: `htmd/src/` — 1278 lines. Author: letmutex.

Converts HTML to Markdown, inspired by turndown.js. Uses `html5ever` for HTML parsing. Supports:
- Headings, paragraphs, lists, tables
- Code blocks with language hints
- Links, images, blockquotes
- Custom element handling

## webdev — Local Development Server

Source: `rust-webdev/src/` — 314 lines.

Simple local web server for localhost development. NOT FOR PRODUCTION. Serves static files from a local directory with:
- MIME type detection
- Directory listing
- CORS headers

## webtk — Web Asset Toolkit

Source: `rust-webtk/src/` — 56 lines.

Utility to transpile, generate, and mix web assets. Provides CLI commands for asset pipeline operations.

## What to Read Next

Continue with the [README](README.md) for the documentation index.
