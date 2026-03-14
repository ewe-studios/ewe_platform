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

### Files Modified
- `Cargo.toml` (workspace root) — added `foundation_codegen` workspace dependency
- `backends/foundation_codegen/Cargo.toml` — new
- `backends/foundation_codegen/src/lib.rs` — new
- `backends/foundation_codegen/src/error.rs` — new
- `backends/foundation_codegen/src/types.rs` — new
- `backends/foundation_codegen/src/cargo_toml.rs` — new

## Remaining

1. **Feature 01: source-scanner** — File walking, syn parsing, AST visitor
2. **Feature 02: module-path-resolver** — Filesystem paths to Rust module paths
3. **Feature 03: registry-api** — CrateScanner public API, HashMap construction

## Next Immediate Action

Read `specifications/06-foundation-codegen/features/01-source-scanner/feature.md` and implement Feature 01.
