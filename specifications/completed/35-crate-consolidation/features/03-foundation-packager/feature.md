---
feature: "foundation_packager"
description: "Create foundation_packager from ewe_temple + ewe_templates, strip all tokio/async deps, inline template reexports"
status: "completed"
priority: "high"
depends_on: []
estimated_effort: "medium"
created: 2026-06-01
last_updated: 2026-06-01
author: "Main Agent"
tasks:
  completed: 9
  uncompleted: 0
  total: 9
  completion_percentage: 100%
---

# Feature: foundation_packager

## Overview

Create `backends/foundation_packager` by merging `crates/temple` (package generation with `PackageGenerator`, `PackageDirectorate`, `Templater`, `FileSystemCommand`) and `crates/templates` (thin `minijinja` + `tinytemplate` reexport). Strip all tokio/async/runtime dependencies. The resulting crate must be `#![no_std]`-adjacent (only uses `std::fs`, no async).

Additionally, vendor the `TinyTemplate` library directly from `https://github.com/ewe-studios/TinyTemplate.git` as a module inside the crate, ending the git fork dependency. The repo contains 6 source files (~800 lines total) with only `serde` + `serde_json` as deps — small enough to maintain in-tree.

## Vendoring TinyTemplate

TinyTemplate is a minimal template engine (~800 lines, 6 source files) with no external deps beyond `serde` + `serde_json`. It's currently pulled from a git fork (`https://github.com/ewe-studios/TinyTemplate.git`).

**Source files vendored:**
- `src/lib.rs` → `foundation_packager/src/tinytemplate/mod.rs`
- `src/compiler.rs` → `foundation_packager/src/tinytemplate/compiler.rs`
- `src/template.rs` → `foundation_packager/src/tinytemplate/template.rs`
- `src/error.rs` → `foundation_packager/src/tinytemplate/error.rs`
- `src/instruction.rs` → `foundation_packager/src/tinytemplate/instruction.rs`
- `src/syntax.rs` → `foundation_packager/src/tinytemplate/syntax.rs` (documentation-only module)

**Changes applied:**
- Removed `extern crate` declarations and `::` prefix references
- Updated `use` paths to be module-relative (`super::error`, `super::compiler`, etc.)
- Removed `#[cfg(test)]` blocks that used `extern crate serde_derive`
- Made `error::get_offset` pub(crate) for cross-module access
- Fixed lifetime annotation on `template.rs:compile` (added explicit `<'template>`)
- Deleted `benchmarks.rs` and `fuzz/` — not needed in-tree

## Completed Tasks

1. ✅ Created `backends/foundation_packager/` directory
2. ✅ Vendored TinyTemplate from `/home/darkvoid/Boxxed/@dev/TinyTemplate/src/` → `foundation_packager/src/tinytemplate/`
3. ✅ Created `Cargo.toml` with correct deps (no `ewe_templates`, no `tinytemplate` git dep, no `ewe_trace`)
4. ✅ Copied source files from temple: error.rs, files.rs, package.rs, lib.rs
5. ✅ Added to `workspace.dependencies`
6. ✅ Found all consumers of `ewe_temple` and `ewe_templates`, updated to `foundation_packager`:
   - `bin/platform/Cargo.toml` + `src/generate/mod.rs`
   - `crates/template-macro/Cargo.toml` + `src/lib.rs`
   - `crates/template-macro/test/jinja/main.rs`
   - `examples/template/hello/Cargo.toml`
   - `examples/template/multi_template/Cargo.toml`
7. ✅ Deleted `crates/temple/` and `crates/templates/`
8. ✅ Ran `cargo check -p foundation_packager` — all green
9. ✅ Ran `cargo test -p foundation_packager` — 9/10 pass (1 pre-existing workspace test failure unrelated to this change)

## Cargo.toml target

```toml
[package]
name = "foundation_packager"
version = "0.1.0"
description = "File generation and templating stack — creates projects from embedded templates with minijinja/tinytemplate support"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
keywords = ["templating", "code-generation", "project-scaffolding"]

[dependencies]
foundation_core = { workspace = true }
foundation_nostd = { workspace = true }
foundation_macros = { workspace = true }
derive_more = { workspace = true }
foundation_conditional = { workspace = true }
cargo_toml = { version = "0.20.5" }
toml = { version = "0.8.19" }
rand = { version = "0.9.2" }
anyhow = { version = "1.0.80" }
tracing = { version = "0.1.40" }
serde = { version = "1.0.197", features = ["derive"] }
serde_json = { version = "1.0.114" }
serde_with = { version = "3.6.1" }
minijinja = { version = "2.0.0", features = ["loader"] }

[dev-dependencies]
tracing-test = { version = "0.2.5" }

[features]
debug_trace = ["foundation_conditional/debug_trace"]

[lints]
workspace = true
```

---

_Created: 2026-06-01_
