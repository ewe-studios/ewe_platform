# Progress - Foundation Codegen

## Completed

### Feature 00: Foundation ✅
- Created `backends/foundation_codegen/Cargo.toml` with workspace-inherited settings
- Added `foundation_codegen` to workspace dependencies in root `Cargo.toml`
- Created `src/lib.rs` with module declarations and re-exports
- Created `src/error.rs` with `CodegenError` enum (manual `Display` + `Error` impls)
- Created `src/types.rs` with `ItemKind`, `Location`, `AttributeValue`, `DerivedTarget`, `CrateMetadata`
- Created `src/cargo_toml.rs` with `CrateMetadata::from_cargo_toml()` implementation
- All 9 tests passing, `cargo fmt` and `cargo clippy -D warnings` clean

### Feature 01: source-scanner ✅
- Created `src/file_walker.rs` with `find_rust_files()` — recursive discovery, skips hidden/target dirs
- Created `src/parser.rs` with `parse_rust_file()` — syn parsing with error handling
- Created `src/visitor.rs` with `MacroFinder` — AST visitor for all item types + inline module tracking
- Implemented attribute argument parsing (string/bool/int/ident/flag/list)
- Created `src/scanner.rs` with `SourceScanner` — `scan_file()` and `scan_directory()` APIs
- All tests passing, `cargo fmt` and `cargo clippy -D warnings` clean

### Feature 02: module-path-resolver ✅
- Created `src/module_path.rs` with `ModulePathResolver` struct
- Implemented `resolve_file_module_path()` — handles lib.rs/main.rs/mod.rs/flat/nested paths
- Implemented `resolve_item_module_path()` — combines file path with inline module nesting
- Implemented `resolve_qualified_path()` — full path including item name
- Added 9 comprehensive tests covering all edge cases
- All tests passing, `cargo fmt` and `cargo clippy -D warnings` clean

### Files Modified
- `Cargo.toml` (workspace root) — added `foundation_codegen` workspace dependency
- `backends/foundation_codegen/Cargo.toml` — new
- `backends/foundation_codegen/src/lib.rs` — new (exports all modules)
- `backends/foundation_codegen/src/error.rs` — new
- `backends/foundation_codegen/src/types.rs` — new (ItemKind now Copy)
- `backends/foundation_codegen/src/cargo_toml.rs` — new
- `backends/foundation_codegen/src/file_walker.rs` — new
- `backends/foundation_codegen/src/parser.rs` — new
- `backends/foundation_codegen/src/visitor.rs` — new
- `backends/foundation_codegen/src/scanner.rs` — new
- `backends/foundation_codegen/src/module_path.rs` — new
- Test files in `tests/` and `tests/units/`

## Remaining

1. **Feature 03: registry-api** — CrateScanner public API, HashMap construction, multi-crate scanning, grouping utilities

## Next Immediate Action

Read `specifications/06-foundation-codegen/features/03-registry-api/feature.md` and implement Feature 03.
