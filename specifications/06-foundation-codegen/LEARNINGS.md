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
