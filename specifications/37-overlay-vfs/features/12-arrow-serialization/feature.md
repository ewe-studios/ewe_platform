---
feature_name: "Arrow Serialization (foundation_arrow)"
description: "foundation_arrow — standalone crate providing Arrow-based zero-copy serialization for any Rust type. Works on native, WASI, and WASM. Derive macro for automatic Arrow schema generation. VFS types are the first consumer but the crate is general-purpose."
status: "in-progress"
priority: "high"
phase: 3
created: 2026-06-04
updated: 2026-06-06
dependencies:
  - "01-core-traits"
tasks:
  completed: 10
  uncompleted: 4
  total: 14
  completion_percentage: 71%

## Global Rule: `foundation_errstacks` Error Handling

All VFS types MUST implement `Debug` and use `VfsResult<T>` (`Result<T, ErrorTrace<VfsError>>`) for errors. Tests use `err.current_context()` for typed error matching. See **plan.md §4d** for the full rule.

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

**Note:** Schema derive macros (`#[derive(ArrowSchema)]`, `#[derive(ArrowJsonSchema)]`, `#[derive(JsonSchema)]`) and the codegen binary have been moved to [Feature 00 — Schema Derive & Codegen Tooling](../00-schema-derive-codegen/feature.md). This feature covers the runtime crate, traits, and IPC layer.

### Core Crate (`backends/foundation_arrow/`)

- [x] Create `backends/foundation_arrow/` crate with Cargo.toml
- [x] Selective arrow-rs features only: `arrow-array`, `arrow-schema`, `arrow-ipc`, `arrow-data`, `arrow-buffer` (skip compute, csv, json, parquet etc.)
- [ ] Verify builds on WASI (`wasm32-wasi`) and WASM (`wasm32-unknown-unknown`)

### Traits (`foundation_arrow/src/traits.rs`)

- [x] Define `ToArrow` trait: convert a type (or Vec of type) to Arrow `RecordBatch`
- [x] Define `FromArrow` trait: convert Arrow `RecordBatch` back to a type (or Vec of type)
- [x] Define `ArrowSchema` trait: generate Arrow `Schema` for a type (field names, types)
- [x] Implement `ArrowValue<T>` helper trait for extracting typed values from `ArrayRef`
- [ ] Implement derive macro `#[derive(ArrowSerialize)]` in `foundation_macros` — auto-generates `ToArrow`, `FromArrow`, `ArrowSchema` from struct fields
- [ ] Field type mapping: u8→UInt8, u16→UInt16, u32→UInt32, u64→UInt64, String→Utf8, Vec\<u8\>→Binary, Option\<T\>→Nullable, bool→Boolean, enums→UInt8/dictionary, SystemTime→Timestamp

### Arrow IPC (`foundation_arrow/src/ipc.rs`)

- [x] Implement Arrow IPC writer: `RecordBatch → Vec<u8>` (IPC file format)
- [x] Implement Arrow IPC reader: `&[u8] → RecordBatch` (via FileReader)
- [x] Implement multi-batch encode/decode: `Vec<RecordBatch> → Vec<u8>`
- [x] Implement `decode_ipc_schema` — extract schema without reading data

### Tests

- [x] Test: single batch encode/decode roundtrip with mixed types (UInt64, Utf8, Float64 nullable, Int64, Binary nullable)
- [x] Test: multi-batch encode/decode
- [x] Test: error on empty batch encode
- [x] Test: error when decoding multi-batch as single
- [ ] Test: VfsMetadata and VfsDirEntry roundtrips
- [ ] Test: WASM compilation succeeds

### Remaining Work

- [ ] Derive macro `#[derive(ArrowSerialize)]` in `foundation_macros` — proc macro for automatic trait generation
- [ ] VFS type implementations — `ToArrow`/`FromArrow` for `VfsMetadata`, `VfsDirEntry`, IPC messages
- [ ] `ArrowMessageBox` adapter — bridge Arrow-encoded messages with existing IPC MessageBox trait
- [ ] WASM/WASI compilation verification

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
