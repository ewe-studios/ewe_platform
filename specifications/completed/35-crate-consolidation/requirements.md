---
description: "Consolidate scattered crates into foundation backends: move html, config, templates into appropriate foundation crates; strip ewe_routing of tokio/axum/server heavy deps for wasm-friendly routing; create foundation_packager from ewe_temple + ewe_templates with all tokio removed."
status: "completed"
priority: "high"
created: 2026-06-01
updated: 2026-06-01 (completed)
author: "Main Agent"
metadata:
  version: "1.0"
  estimated_effort: "large"
  tags:
    - crate-consolidation
    - foundation-packager
    - wasm-compat
    - tokio-removal
    - crate-migration
has_features: true
has_fundamentals: false
builds_on:
  - "specifications/31-wasm-testbed"
  - "specifications/29-wasm-http-client"
related_specs:
  - "specifications/21-http-framework"
  - "specifications/34-native-file-watchers"
tasks:
  completed: 5
  uncompleted: 0
  total: 5
  completion_percentage: 100%
---

# Crate Consolidation & foundation_packager

## Overview

Consolidate five scattered `crates/*` packages into proper `backends/foundation_*` crates, stripping unnecessary async/runtime dependencies (tokio, axum, tower, hyper) from crates that need to work in wasm and `#![no_std]`-adjacent contexts. This is part of the ongoing valtron rework to make foundation crates compile for wasm32-unknown-unknown without pulling in a full async runtime.

## Motivation

The current `crates/` directory holds packages that are foundational in nature but live in the wrong location:

| Current crate | Target location | Reason |
|---|---|---|
| `crates/html` | `backends/foundation_html` | Core HTML generation — used by templating, not web-server-specific |
| `crates/config` | `backends/foundation_config` | TOML-based config loading — universal utility |
| `crates/templates` | absorbed into `foundation_packager` | Thin re-export wrapper; no reason to stay standalone |
| `crates/routing` | evaluated for wasm-friendly subset | Heavy tokio/axum/tower/hyper deps make it unusable in wasm |
| `crates/temple` + `crates/templates` | `backends/foundation_packager` | Package generation from templates — needs tokio stripped, templates inlined |

## Known Issues / Limitations

- `ewe_routing` depends on `axum::body`, `tower::Service`, `tokio::rt`, `tokio::macros` — these are all server-side only. The routing data structures (`RouteSegment`, `SegmentType`, `RouteMethod`) are pure logic that *could* work in wasm if decoupled.
- `ewe_temple` depends on `ewe_templates` which pulls in `minijinja` (ok for wasm) and `tinytemplate` (ok for wasm), but `ewe_temple` itself also pulls `foundation_core` which may have async features.
- `crates/templates` is literally a 2-line reexport crate — it should be absorbed, not moved.
- `crates/html` has no tokio/axum deps already — it's clean for wasm. It just needs relocating.
- `crates/config` has no async deps at all — it's pure `std::fs` + serde + toml. Clean for wasm.

## Decision: routing crate

**Decision:** `crates/routing` will be evaluated feature-by-feature. The route-matching data structures (`RouteSegment`, `SegmentType`, `RouteMethod`, `Servicer`) are pure Rust with no runtime dependency — they can form the core of a wasm-friendly `foundation_routing` crate. The `RouterService` (tower service wrapping for axum) and tests that use `tokio::test` should be gated behind a `server` feature that pulls in tokio/axum/tower.

Compare with `foundation_http` which successfully used a `wasm` feature flag to conditionally pull `wasm-bindgen` and `getrandom/js`. The same pattern applies here: default = pure routing logic, `server` feature = axum/tower/tokio integration.

## Feature Index

1. **[foundation_html](features/01-foundation-html/feature.md)** — Move `crates/html` to `backends/foundation_html`, update workspace
2. **[foundation_config](features/02-foundation-config/feature.md)** — Move `crates/config` to `backends/foundation_config`, update workspace
3. **[foundation_packager](features/03-foundation-packager/feature.md)** — Create from `crates/temple` + `crates/templates`, strip tokio, inline template reexports, vendor TinyTemplate
4. **[foundation_routing wasm subset](features/04-foundation-routing-wasm/feature.md)** — Strip tokio/axum/tower/hyper from routing, gate behind `server` feature
5. **[foundation_conditional](features/05-foundation-conditional/feature.md)** — Replace `crates/trace` with block-level conditional compilation macros that gate entire code blocks on feature flags

---

_Created: 2026-06-01_
