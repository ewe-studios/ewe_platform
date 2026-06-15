---
feature_name: "Schema Derive & Codegen Tooling"
description: "Three derive macros (#[derive(ArrowSchema)], #[derive(ArrowJsonSchema)], #[derive(JsonSchema)]) in foundation_macros for compile-time schema generation. Traits in foundation_arrow. Rename foundation_wasmtools → foundation_codegentools and add Arrow/JSON schema codegen binary for static /sdk artifact generation."
status: "done"
priority: "high"
phase: 3
created: 2026-06-06
updated: 2026-06-06
dependencies:
  - "12-arrow-serialization"
tasks:
  completed: 39
  uncompleted: 0
  total: 39
  completion_percentage: 100%
---

# Feature 00: Schema Derive & Codegen Tooling

## Overview

Three compile-time schema derive macros and a codegen binary for static artifact generation. The derives inspect struct fields at proc-macro expansion time and emit function bodies with hardcoded constructors — no runtime reflection, no field scanning, just pre-built schema values returned on call.

### Three Traits, Clear Dependency Chain

```
ArrowSchema          → returns arrow_schema::Schema (native Arrow types)
    ↓ (depends on)
ArrowJsonSchema      → returns serde_json::Value (Arrow type definitions as JSON)

JsonSchema           → returns serde_json::Value (standard W3C/IETF JSON Schema)
```

These are separate because Arrow's type system is richer than JSON Schema's:

| Rust type | JSON Schema | Arrow JSON Schema |
|-----------|-------------|-------------------|
| `u64` | `{"type": "integer", "format": "uint64"}` | `{"data_type": "UInt64"}` |
| `String` | `{"type": "string"}` | `{"data_type": "Utf8"}` |
| `Vec<u8>` | `{"type": "array", "items": {"type": "integer"}}` | `{"data_type": "Binary"}` |
| `Option<f64>` | `{"type": "number"}` (nullable) | `{"data_type": "Float64", "nullable": true}` |
| `SystemTime` | `{"type": "string", "format": "date-time"}` | `{"data_type": "Timestamp(Millisecond, Some(\"UTC\"))"}` |

Collapsing them would lose Arrow-specific precision (UInt64 vs Int64, Utf8 vs LargeUtf8, Timestamp with precision/timezone, dictionary encoding).

### Where Things Live

| What | Where | Why |
|------|-------|-----|
| `ArrowSchema` trait | `foundation_arrow` | Needs `arrow_schema::Schema` return type |
| `ArrowJsonSchema` trait | `foundation_arrow` (feature-gated `jsonschema`) | Depends on `ArrowSchema`, returns JSON of Arrow types |
| `JsonSchema` trait | `foundation_jsonschema` | General-purpose, nothing to do with Arrow |
| `#[derive(ArrowSchema)]` | `foundation_macros` | All proc/derive macros go here |
| `#[derive(ArrowJsonSchema)]` | `foundation_macros` | All proc/derive macros go here |
| `#[derive(JsonSchema)]` | `foundation_macros` | All proc/derive macros go here |
| Codegen binary | `foundation_codegentools` | Unified codegen crate (renamed from `foundation_wasmtools`) |

**No circular dependency:** `foundation_macros` never depends on `foundation_arrow` or `foundation_jsonschema`. The derives emit tokens that *reference* trait paths like `foundation_arrow::ArrowSchema` and `foundation_jsonschema::JsonSchema` — same pattern as scaffold (`scaffold!()` trait in `foundation_nostd`, derive in `foundation_macros`).

## Derive Behavior

### `#[derive(ArrowSchema)]`

Inspects struct fields, generates `impl ArrowSchema`:

```rust
#[derive(ArrowSchema)]
struct VfsMetadata {
    size: u64,
    name: String,
    permissions: u32,
    checksum: Option<Vec<u8>>,
}

// Generated at compile time:
impl ArrowSchema for VfsMetadata {
    fn fields() -> Vec<arrow_schema::Field> {
        vec![
            arrow_schema::Field::new("size", arrow_schema::DataType::UInt64, false),
            arrow_schema::Field::new("name", arrow_schema::DataType::Utf8, false),
            arrow_schema::Field::new("permissions", arrow_schema::DataType::UInt32, false),
            arrow_schema::Field::new("checksum", arrow_schema::DataType::Binary, true),
        ]
    }

    fn schema() -> arrow_schema::Schema {
        arrow_schema::Schema::new(Self::fields())
    }
}
```

### `#[derive(ArrowJsonSchema)]`

Requires `ArrowSchema`. Emits `impl ArrowJsonSchema` that returns the Arrow schema as a pre-built `serde_json::Value`:

```rust
// Generated — hardcoded JSON constructors, no runtime computation:
impl ArrowJsonSchema for VfsMetadata {
    fn arrow_json_schema() -> serde_json::Value {
        serde_json::json!({
            "fields": [
                {"name": "size", "data_type": "UInt64", "nullable": false},
                {"name": "name", "data_type": "Utf8", "nullable": false},
                {"name": "permissions", "data_type": "UInt32", "nullable": false},
                {"name": "checksum", "data_type": "Binary", "nullable": true}
            ]
        })
    }
}
```

### `#[derive(JsonSchema)]`

Independent of Arrow. Emits standard W3C/IETF JSON Schema:

```rust
// Generated — hardcoded JSON constructors:
impl JsonSchema for VfsMetadata {
    fn json_schema() -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "size": {"type": "integer", "format": "uint64"},
                "name": {"type": "string"},
                "permissions": {"type": "integer", "format": "uint32"},
                "checksum": {"type": "string", "format": "binary", "nullable": true}
            },
            "required": ["size", "name", "permissions"]
        })
    }
}
```

## Type Mapping Reference

### Arrow Field Mapping (`#[derive(ArrowSchema)]`)

| Rust type | Arrow `DataType` | Nullable |
|-----------|-------------------|----------|
| `u8` | `UInt8` | false |
| `u16` | `UInt16` | false |
| `u32` | `UInt32` | false |
| `u64` | `UInt64` | false |
| `i8` | `Int8` | false |
| `i16` | `Int16` | false |
| `i32` | `Int32` | false |
| `i64` | `Int64` | false |
| `f32` | `Float32` | false |
| `f64` | `Float64` | false |
| `bool` | `Boolean` | false |
| `String` | `Utf8` | false |
| `Vec<u8>` | `Binary` | false |
| `Option<T>` | (inner type) | true |
| `SystemTime` | `Timestamp(Millisecond, Some("UTC"))` | false |

### JSON Schema Mapping (`#[derive(JsonSchema)]`)

| Rust type | JSON Schema |
|-----------|-------------|
| `u8`, `u16`, `u32`, `u64` | `{"type": "integer", "format": "uint{N}"}` |
| `i8`, `i16`, `i32`, `i64` | `{"type": "integer", "format": "int{N}"}` |
| `f32`, `f64` | `{"type": "number", "format": "float{N}"}` |
| `bool` | `{"type": "boolean"}` |
| `String` | `{"type": "string"}` |
| `Vec<u8>` | `{"type": "string", "format": "binary"}` |
| `Option<T>` | (inner schema, removed from `required`) |
| `SystemTime` | `{"type": "string", "format": "date-time"}` |

## Codegen Binary: `foundation_codegentools`

Rename `foundation_wasmtools` → `foundation_codegentools`. Existing `wasm_bins` module stays untouched. Add new `schema_gen` module for Arrow/JSON schema codegen.

The binary scans a crate's source, finds all structs with `#[derive(ArrowSchema)]` and/or `#[derive(JsonSchema)]`, and writes static artifacts to an `/sdk` directory beside `/src`:

```
my_crate/
  src/
    types.rs          # has #[derive(ArrowSchema, JsonSchema)]
  sdk/                # generated by foundation_codegentools
    jsonschema/
      VfsMetadata.json
      VfsDirEntry.json
    arrow_schema/
      VfsMetadata.json
      VfsDirEntry.json
```

### CLI Interface

```bash
# Both schemas
foundation_codegentools schema --crate ./backends/foundation_nativeapis --jsonschema --arrow-schema

# JSON Schema only
foundation_codegentools schema --crate ./backends/foundation_nativeapis --jsonschema

# Arrow schema only
foundation_codegentools schema --crate ./backends/foundation_nativeapis --arrow-schema
```

### How It Works

1. Walk `/src/**/*.rs` files, parse with `syn`
2. Find structs with `#[derive(ArrowSchema)]` or `#[derive(JsonSchema)]`
3. Extract field names and types
4. Apply the same type mapping tables as the derives
5. Write `.json` files to `/sdk/{jsonschema,arrow_schema}/`

This gives non-Rust consumers (Python, JS, API docs tooling) access to the schemas without running Rust code.

## Tasks

### `ArrowJsonSchema` trait (`foundation_arrow`)

- [x] Define `ArrowJsonSchema` trait in `foundation_arrow/src/traits.rs` — `fn arrow_json_schema() -> serde_json::Value`
- [x] `ArrowJsonSchema` requires `ArrowSchema` as supertrait
- [x] Add `serde_json` dep to `foundation_arrow/Cargo.toml`

### `JsonSchema` trait (`foundation_jsonschema`)

- [x] Define `JsonSchema` trait in `backends/foundation_jsonschema/src/schema_type.rs` — `fn json_schema() -> serde_json::Value`
- [x] Add `mod schema_type` and `pub use` to `lib.rs`

### Shared utilities (`foundation_macros`)

- [x] Create `src/crate_paths.rs` — `foundation_arrow_path()`, `foundation_jsonschema_path()`, `serde_json_path()` via `proc-macro-crate`
- [x] Create `src/schema_fields.rs` — `StructField`, `extract_struct_fields()`, `Option<T>`/`Vec<u8>`/`SystemTime` detection

### `#[derive(ArrowSchema)]` (`foundation_macros/src/arrow_schema.rs`)

- [x] Parse struct fields: name, type, Option wrapper detection
- [x] Map Rust types to `arrow_schema::DataType` variants (see type mapping table)
- [x] Generate `impl ArrowSchema` with `fields()` and `schema()` methods
- [x] Handle `Option<T>` → nullable: true
- [x] Handle `Vec<u8>` → Binary (not List<UInt8>)
- [x] Handle `SystemTime` → Timestamp(Millisecond)
- [x] Wire into `foundation_macros/src/lib.rs` as `proc_macro_derive`

### `#[derive(ArrowJsonSchema)]` (`foundation_macros/src/arrow_json_schema.rs`)

- [x] Parse struct fields (same field extraction as ArrowSchema)
- [x] Generate `impl ArrowJsonSchema` with pre-built `serde_json::json!()` body
- [x] Arrow type names as strings: "UInt64", "Utf8", "Binary", "Timestamp(Millisecond, UTC)", etc.
- [x] Wire into `foundation_macros/src/lib.rs` as `proc_macro_derive`

### `#[derive(JsonSchema)]` (`foundation_macros/src/json_schema.rs`)

- [x] Parse struct fields (same field extraction)
- [x] Map Rust types to JSON Schema types (see JSON Schema mapping table)
- [x] Generate `impl JsonSchema` with pre-built `serde_json::json!()` body
- [x] Handle `required` array (all non-Option fields)
- [x] Wire into `foundation_macros/src/lib.rs` as `proc_macro_derive`

### Rename `foundation_wasmtools` → `foundation_codegentools`

- [x] Rename crate directory `backends/foundation_wasmtools/` → `backends/foundation_codegentools/`
- [x] Update `Cargo.toml` package name
- [x] Update all workspace references (`Cargo.toml` workspace members, any `use` or `dep` references)
- [x] Verify existing `wasm_bins` module unchanged and tests compile

### Schema codegen binary (`foundation_codegentools`)

- [x] Add `schema_gen` module: syn-based source scanner for `#[derive(ArrowSchema)]` / `#[derive(JsonSchema)]` structs
- [x] Implement struct field extraction from parsed AST
- [x] Implement JSON Schema `.json` file writer
- [x] Implement Arrow Schema `.json` file writer
- [x] Add CLI: `platform schema --crate <path> --jsonschema --arrow-schema`
- [x] Write to `<crate>/sdk/jsonschema/` and `<crate>/sdk/arrow_schema/`

### Tests

- [x] `test_derive_arrow_schema` — struct with mixed types, verify `fields()` returns correct Arrow DataTypes
- [x] `test_derive_arrow_schema_option` — Option<T> fields are nullable
- [x] `test_derive_arrow_json_schema` — verify JSON output matches expected Arrow type strings
- [x] `test_derive_json_schema` — verify standard JSON Schema output
- [x] `test_json_schema_required` — non-Option fields in `required` array, Option fields omitted
- [x] `test_codegen_roundtrip` — scan a test crate, generate `/sdk`, verify `.json` files match derive output (7 tests in `schema_gen_tests.rs`)

## Verification

- `cargo test -p foundation_macros` — all derive tests pass
- `cargo test -p foundation_arrow --features jsonschema` — trait tests pass
- `cargo test -p foundation_codegentools` — existing wasm_bins tests + new schema_gen tests pass
- Generated `/sdk/*.json` files are valid JSON Schema / valid Arrow schema JSON

## References

- Feature 12 (Arrow Serialization) — `foundation_arrow` crate, `ArrowSchema` trait (already exists)
- Feature 18 (Scaffold Macro) — prior art for proc-macro patterns in `foundation_macros`
- [JSON Schema spec](https://json-schema.org/specification) — W3C/IETF standard
- [Arrow Schema](https://arrow.apache.org/docs/format/Columnar.html#schema-message) — Arrow columnar format schema definition

---

_Created: 2026-06-06_
