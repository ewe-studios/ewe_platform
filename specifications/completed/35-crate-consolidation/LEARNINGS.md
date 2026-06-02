# Learnings

## TinyTemplate vendoring: directory module convention

- Rust expects `src/tinytemplate/mod.rs` for directory-based modules, not `lib.rs`. Initially created `lib.rs` which caused "module not found" errors. Renaming to `mod.rs` fixed it immediately.

## Module-relative paths in vendored code

- When vendoring a crate that was previously a standalone `extern crate`, all `use ::module::*` paths must become `super::module::*` (or `crate::module::*` from the parent). The `::` prefix means "crate root" which no longer exists as a separate crate.

## ewe_trace was unnecessary wrapper

- `tracing::info!`/`debug!`/`error!`/`warn!` are already no-op when no subscriber is registered. The `ewe_trace` crate wrapped these behind feature flags but provided zero additional value. Direct `tracing::` calls are the correct approach.

## foundation_http already provides complete routing

- `foundation_http` has a full generic `Router<S>`, `RouteSegment<S>`, `SegmentType`, `RouteMethod<S>` implementation. The `ewe_routing` crate's `server` feature was redundant — nothing needed a separate routing stack. Deleting `ewe_routing` reduced duplication.

## template.rs lifetime fix

- `pub fn compile(text: &'template str) -> Result<Template<'template>>` needs an explicit `<'template>` on the return type, otherwise Rust infers an anonymous lifetime and warns about hidden lifetimes.

## Rust directory module: mod.rs not lib.rs

- For a module defined as a directory (e.g., `src/tinytemplate/`), Rust looks for `mod.rs`, not `lib.rs`. `lib.rs` is only for crate roots.

## Workspace Cargo.toml linter stripping

- The workspace linter/cargo check stripped newly added workspace dependencies from the root `Cargo.toml`. Had to re-add `foundation_conditional`, `foundation_config`, `foundation_html`, `foundation_packager` after the fact.
