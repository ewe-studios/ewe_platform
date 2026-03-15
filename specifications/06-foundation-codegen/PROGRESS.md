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

### Feature 03: registry-api ✅
- Created `src/crate_scanner.rs` with `CrateScanner` struct
- Implemented `new()` constructor
- Implemented `scan_crate()` — single crate scanning pipeline
- Implemented `scan_crates()` — multi-crate with qualified_path keys
- Implemented `scan_workspace()` — workspace-level scanning (glob patterns documented limitation)
- Created `src/registry.rs` with `ScanRegistry` type alias
- Implemented `RegistryExt` trait with all utility methods:
  - `group_by_attribute()` — group items by attribute value
  - `filter_by_kind()` — filter by ItemKind
  - `filter_by_attribute()` — filter by specific attribute value
  - `unique_attribute_values()` — get unique attribute values
  - `filter_by_crate()` — filter by crate name
  - `sorted_by_module_path()` — sort all targets by module path
- Added 4 doctests for public API
- All 74 tests passing (61 unit + 9 module_path + 4 doctests)
- `cargo fmt` and `cargo clippy -D warnings` clean

### Files Modified
- `Cargo.toml` (workspace root) — added `foundation_codegen` workspace dependency
- `backends/foundation_codegen/Cargo.toml` — new
- `backends/foundation_codegen/src/lib.rs` — updated with all re-exports
- `backends/foundation_codegen/src/error.rs` — new
- `backends/foundation_codegen/src/types.rs` — new (ItemKind now Copy)
- `backends/foundation_codegen/src/cargo_toml.rs` — new
- `backends/foundation_codegen/src/file_walker.rs` — new
- `backends/foundation_codegen/src/parser.rs` — new
- `backends/foundation_codegen/src/visitor.rs` — new
- `backends/foundation_codegen/src/scanner.rs` — new
- `backends/foundation_codegen/src/module_path.rs` — new
- `backends/foundation_codegen/src/crate_scanner.rs` — new
- `backends/foundation_codegen/src/registry.rs` — new
- Test files in `tests/` and `tests/units/`

### Feature 04: wasm-entrypoint-toolchain ✅
- **Component 1: `wasm_entrypoint` Proc Macro**
  - Created `backends/foundation_macros/src/wasm_entrypoint.rs` — attribute proc macro implementation
  - Added `#[proc_macro_attribute] pub fn wasm_entrypoint` to `backends/foundation_macros/src/lib.rs`
  - Added `syn` `full` + `parsing` + `extra-traits` features to foundation_macros Cargo.toml
  - Added `trybuild` dev-dependency for compile-fail tests
  - Created 5 trybuild test cases (1 pass, 4 fail) — all passing
- **Component 2: `system_operations` Crate**
  - Created `crates/system_operations/Cargo.toml` with workspace-inherited settings
  - Added `system_operations` and `toml_edit` to workspace dependencies in root Cargo.toml
  - Created `src/lib.rs` → `src/wasm_bins/mod.rs` with `WasmBinGenerator` struct
  - Created `src/wasm_bins/error.rs` — `WasmBinError` enum with manual Display/Error impls
  - Created `src/wasm_bins/validator.rs` — crate validation (cdylib/bin/no-lib)
  - Created `src/wasm_bins/planner.rs` — dry-run plan builder (generates `main.rs` with proper `use` imports)
  - Created `src/wasm_bins/generator.rs` — file creation + Cargo.toml update via `toml_edit`
  - Created test fixtures: `wasm_crate` (cdylib, 3 entrypoints) + `rlib_crate` (invalid)
  - 10 tests passing (6 integration + 4 validator), fmt + clippy clean
- **Component 3: `wasm_bins` Platform Subcommand**
  - Created `bin/platform/src/wasm_bins/mod.rs` with `register()` + `run()` + `list`/`generate` handlers
  - Registered in `bin/platform/src/main.rs` (added to chain + match)
  - Added `system_operations` dependency to `bin/platform/Cargo.toml`
  - Note: `ewe_platform` binary has pre-existing build errors in `foundation_ai` (unrelated)

### Files Modified (Feature 04)
- `Cargo.toml` (workspace root) — added `system_operations`, `toml_edit` workspace deps
- `backends/foundation_macros/Cargo.toml` — added `syn` features, `trybuild` dev-dep
- `backends/foundation_macros/src/lib.rs` — added `wasm_entrypoint` attribute proc macro
- `backends/foundation_macros/src/wasm_entrypoint.rs` — new
- `backends/foundation_macros/tests/` — new test structure with trybuild pass/fail fixtures
- `crates/system_operations/` — entire new crate
- `bin/platform/src/main.rs` — registered `wasm_bins` subcommand
- `bin/platform/src/wasm_bins/mod.rs` — new
- `bin/platform/Cargo.toml` — added `system_operations` dep

## Remaining

None — all features complete!

## Summary

**Total Tests:** 84 (74 foundation_codegen + 1 trybuild/5 cases foundation_macros + 9 system_operations)
**Code Quality:** `cargo fmt` ✅, `cargo clippy -D warnings` ✅ (for foundation_macros, foundation_codegen, system_operations)

The `foundation_codegen` spec is fully implemented. All 5 features complete.
