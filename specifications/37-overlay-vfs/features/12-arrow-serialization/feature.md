---
feature_name: "Arrow Serialization (foundation_arrow)"
description: "foundation_arrow — standalone crate providing Arrow-based zero-copy serialization for any Rust type. Works on native, WASI, and WASM. Derive macro for automatic Arrow schema generation. VFS types are the first consumer but the crate is general-purpose."
status: "pending"
priority: "medium"
phase: 3
created: 2026-06-04
updated: 2026-06-04
dependencies:
  - "01-core-traits"
tasks:
  completed: 0
  uncompleted: 14
  total: 14
  completion_percentage: 0%
---

# Feature 12: Arrow Serialization

## Overview

`foundation_arrow` — a standalone crate in `backends/` that provides Arrow-based zero-copy serialization for **any Rust type**. Not scoped to VFS or IPC — it's a general-purpose foundation crate. VFS types are the first consumer but any crate in the workspace can use it.

Must work on all targets: native (Linux, macOS, Windows), WASI, and WASM (browser).

Uses `arrow-rs` (official Apache Arrow Rust implementation) with selective feature enables. Arrow provides:

- **Zero-copy reads** — data laid out in memory as wire format, no deserialization
- **Language-neutral** — same layout across Rust, Python, C++, JavaScript
- **WASM support** — arrow-rs compiles to WASM (both wasi and wasm32-unknown-unknown)
- **IPC streaming format** — designed for inter-process zero-copy transfer
- **Columnar layout** — efficient for batched/tabular data

## Where Arrow Helps VFS

| Operation | Bincode (current IPC) | Arrow |
|-----------|----------------------|-------|
| Small messages (Open, Stat, Close) | Fast, minimal overhead | Overkill — use bincode |
| Large file reads (ReadAt response) | Serialize → copy → deserialize | Zero-copy: mmap or shared memory, read directly |
| Directory listings (many entries) | Vec\<VfsDirEntry\> serialized | Columnar RecordBatch: names, types, sizes as arrays |
| Bulk metadata queries | N × stat roundtrips | Single RecordBatch with all metadata columns |
| Shared memory IPC | Manual layout | Arrow IPC format over MemoryRegion (from spec-34) |

## Tasks

### Core Crate (`backends/foundation_arrow/`)

- [ ] Create `backends/foundation_arrow/` crate with Cargo.toml
- [ ] Selective arrow-rs features only: `arrow-array`, `arrow-schema`, `arrow-ipc`, `arrow-data` (skip compute, csv, json, parquet etc.)
- [ ] Verify builds on native, WASI (`wasm32-wasi`), and WASM (`wasm32-unknown-unknown`)

### Traits and Derive (`foundation_arrow/src/`)

- [ ] Define `ToArrow` trait: convert a type (or Vec of type) to Arrow `RecordBatch`
- [ ] Define `FromArrow` trait: convert Arrow `RecordBatch` back to a type (or Vec of type)
- [ ] Define `ArrowSchema` trait: generate Arrow `Schema` for a type (field names, types)
- [ ] Implement derive macro `#[derive(ArrowSerialize)]` (in `foundation_macros` or companion proc-macro crate) — auto-generates `ToArrow`, `FromArrow`, `ArrowSchema` from struct fields
- [ ] Field type mapping: u8→UInt8, u16→UInt16, u32→UInt32, u64→UInt64, String→Utf8, Vec\<u8\>→Binary, Option\<T\>→Nullable, bool→Boolean, enums→UInt8/dictionary, SystemTime→Timestamp

### Arrow IPC (`foundation_arrow/src/ipc.rs`)

- [ ] Implement Arrow IPC stream writer: `RecordBatch → Vec<u8>` (IPC streaming format)
- [ ] Implement Arrow IPC stream reader: `&[u8] → RecordBatch` (zero-copy from buffer)
- [ ] Shared memory integration helper: write Arrow IPC into a byte buffer (for use with spec-34 `MemoryRegion` or any mmap/shared buffer)

### VFS Type Implementations (in foundation_nativeapis, depends on foundation_arrow)

- [ ] Implement `ArrowSerialize` for `VfsMetadata`
- [ ] Implement `ArrowSerialize` for `VfsDirEntry` / `Vec<VfsDirEntry>` (columnar)
- [ ] Implement `ArrowSerialize` for VFS IPC messages (VfsRequest, VfsResponse)
- [ ] Define `ArrowMessageBox` adapter bridging Arrow-encoded messages with existing IPC MessageBox trait

### Tests

- [ ] Test: arbitrary struct roundtrip through Arrow (not VFS-specific)
- [ ] Test: Vec\<struct\> columnar roundtrip
- [ ] Test: Arrow IPC stream encode/decode
- [ ] Test: VfsMetadata and VfsDirEntry roundtrips
- [ ] Test: WASM compilation succeeds

## Verification

- `cargo check -p foundation_arrow` passes
- `cargo check -p foundation_arrow --target wasm32-unknown-unknown` passes
- `cargo check -p foundation_arrow --target wasm32-wasi` passes
- Zero-copy confirmed: reader accesses Arrow buffer without allocation

## Dependencies

```toml
# backends/foundation_arrow/Cargo.toml
[dependencies.arrow]
version = "53"  # or latest
default-features = false
features = ["arrow-array", "arrow-schema", "arrow-ipc", "arrow-data"]
```

## Uses Across the Workspace

- VFS IPC (zero-copy file data and directory listings)
- Any IPC message type (bulk data transfer)
- Database query results (foundation_db)
- Streaming data pipelines
- Cross-language FFI (Python/JS agents reading Arrow buffers directly)
- HTTP response bodies (columnar API responses)
