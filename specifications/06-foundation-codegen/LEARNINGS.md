# Learnings - Foundation Codegen

## Design Decisions

- **Manual `Error` + `Display` impls instead of `derive_more::Error`**: The workspace uses `derive_more >= 2.0` which doesn't support `#[error(not(source))]` for tuple variants wrapping non-Error types like `PathBuf`. Manual impl gives full control over `source()` delegation.
- **No `derive_more::From` on error enum**: Multiple variants wrap `PathBuf`, causing conflicting `From<PathBuf>` implementations. All error construction is explicit.
- **Workspace `toml` is 0.9** (spec originally said 0.8) — used `{ workspace = true }` to stay consistent.
- **Workspace `derive_more` is `>=2.0` with `full` features** — used `{ workspace = true }`.

## Challenges & Solutions

- **Clippy pedantic**: Workspace enables `clippy::pedantic` as warnings. Required `# Errors` doc sections on fallible public functions, backtick-wrapping identifiers in doc comments, and avoiding `.expect()` in library code.
- **`canonicalize()` in `from_cargo_toml`**: Used early to resolve the path before existence checks, which means the IO error on non-existent files comes from `canonicalize` rather than a separate existence check.

## Patterns Discovered

- Workspace inherits: `edition.workspace = true`, `lints.workspace = true`, `rust-version.workspace = true`, etc.
- Backend crates follow `backends/{crate_name}/` layout, auto-discovered via `backends/*` in workspace members.
- **Tests go in `tests/` directory, NOT inline `#[cfg(test)]`** per `rust-clean-code/testing/skill.md`. Structure: `tests/crate_tests.rs` → `mod units;` → `tests/units/{crate}_{module}_tests.rs`.
- Cargo ignores `tests/mod.rs` — need a named `.rs` file in `tests/` as the entry point.
- All public items need WHY/WHAT/HOW documentation, `# Errors`, and `# Panics` sections per `rust-clean-code/implementation/skill.md`.

## New Learnings (Features 01-02)

- **`syn::parse_file` works outside proc-macro context**: Can parse any Rust source file from a build script or standalone tool, not just in proc macro `TokenStream` handlers.
- **`syn::visit::Visit` for AST traversal**: The visitor pattern automatically walks the entire AST; override specific `visit_item_*` methods to intercept items.
- **`attr.parse_nested_meta` for attribute parsing**: Syn 2.x provides this method for parsing `#[attr(key = "value", flag)]` syntax cleanly.
- **Inline module tracking**: The visitor maintains a stack (`module_stack`) to track nesting from `mod inner { ... }` blocks within a file.
- **Module path resolution rules**: `lib.rs`/`main.rs` → crate root, `mod.rs` → parent directory name, `foo.rs` → `foo` module segment.
- **`ItemKind` should be `Copy`**: Small enum variants without heap data should implement `Copy` to avoid unnecessary `.clone()` calls (clippy lint).
- **Path handling**: Use `&Path` for function arguments, `PathBuf` for struct fields; clippy flags `&PathBuf` as redundant.

## New Learnings (Feature 03)

- **Type aliases for complex types**: `ScanRegistry = HashMap<String, DerivedTarget>` provides a clear, single source of truth for the registry type.
- **Extension traits for utilities**: `RegistryExt` trait adds filtering/grouping methods without cluttering the core type.
- **Qualified paths as keys**: Using `qualified_path` (e.g., `crate::module::Item`) as HashMap keys avoids name collisions across crates.
- **Workspace member parsing**: Can extract `workspace.members` from `Cargo.toml` using `toml::Value` navigation, but glob patterns require manual handling.
- **Doctest hygiene**: Use `no_run` and import statements in doc examples to avoid compilation failures during doctest runs.
- **Merge patterns**: Multi-crate scanning merges results by iterating and re-keying to avoid collisions.
