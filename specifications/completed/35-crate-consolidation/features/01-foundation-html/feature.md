---
feature: "foundation_html"
description: "Move crates/html to backends/foundation_html, update workspace Cargo.toml, update all consumers"
status: "completed"
priority: "high"
depends_on: []
estimated_effort: "small"
created: 2026-06-01
last_updated: 2026-06-01
author: "Main Agent"
tasks:
  completed: 6
  uncompleted: 0
  total: 6
  completion_percentage: 100%
---

# Feature: foundation_html

## Overview

Move `crates/html` (currently `ewe_html`) to `backends/foundation_html` with package name `foundation_html`. This crate is already clean — no tokio, axum, tower, or hyper dependencies. It contains HTML markup generation and parsing utilities used by templating and web layers.

## Current state

- **Location:** `backends/foundation_html/`
- **Package name:** `foundation_html`
- **Dependencies:** `foundation_core`, `foundation_conditional`, `regex`, `lazy-regex`, `phf`, `strum`, `lazy_static`, `lazycell`, `tracing`, `tracing-subscriber`, `anyhow`, `thiserror`
- **Modules:** `markup`, `parsers`, `router`
- **Tests:** benchmarks in `benches/cwikipedia.rs`

## Completed Tasks

1. ✅ Moved `crates/html/` → `backends/foundation_html/`
2. ✅ Updated `Cargo.toml`: renamed package to `foundation_html`, updated keywords
3. ✅ Added to `workspace.dependencies` as `foundation_html = { path = "./backends/foundation_html", version = "0.0.1" }`
4. ✅ Found all crates that depend on `ewe_html` and updated to `foundation_html`
5. ✅ Replaced `ewe_trace::debug!/error!` with `tracing::debug!/error!` in parsers.rs, markup.rs
6. ✅ Replaced `extern crate ewe_trace` with `extern crate foundation_conditional` in lib.rs
7. ✅ Ran `cargo check` to verify compilation — all green

## Cargo.toml target

```toml
[package]
name = "foundation_html"
version = "0.0.1"
description = "HTML generation and parsing utilities for web and non-web contexts"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
keywords = ["html", "markup", "parsing"]
```

---

_Created: 2026-06-01_
