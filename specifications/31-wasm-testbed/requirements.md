---
description: "CLI-driven test harness for wasm32-unknown-unknown, supporting browser (Playwright), Deno, and Cloudflare Workers (wrangler) execution, with bindgen and custom modes"
status: "completed"
priority: "high"
created: 2026-05-31
author: "Main Agent"
metadata:
  version: "1.0"
  estimated_effort: "large"
  tags:
    - wasm
    - testing
    - wasm-bindgen
    - playwright
    - deno
    - wrangler
    - cloudflare-workers
  skills: []
  tools:
    - Rust
    - cargo
    - wasm-bindgen
    - Playwright
    - Deno
    - wrangler
    - walrus
has_features: true
has_fundamentals: false
builds_on: "specifications/28-cloudflare-workers-readiness"
related_specs:
  - "specifications/03-wasm-friendly-sync-primitives"
  - "specifications/11-foundation-deployment"
  - "specifications/28-cloudflare-workers-readiness"
features:
  completed: 2
  uncompleted: 0
  total: 2
  completion_percentage: 100%
---

# WASM Testbed Specification

## Overview

A CLI crate `foundation_wasm_testbed` that provides a unified testing interface for wasm32-unknown-unknown projects. It replaces the complexity of `wasm-pack test` and `wasm-bindgen-test-runner` with a simple, predictable model:

- The CLI builds, stages files into the integration directory, then runs the test.
- The test runner (browser or Deno) loads everything via relative paths — no special endpoints, no scanning, no proxying.
- `bindgen-*` modes auto-generate harnesses from `#[wasm_bindgen_test]` macros.
- `web` and `deno` modes use user-provided JS in `integrations/{type}/`.

## Feature Index

| # | Feature | Description | Status |
|---|---|---|---|
| 01 | [wasm-testbed](./features/01-wasm-testbed/feature.md) | CLI-driven test harness for wasm32 execution across browser, Deno, and Cloudflare Workers | completed |
| 02 | [wasm-js-yield-integration-tests](./features/02-wasm-js-yield-integration-tests/feature.md) | wasm-pack integration tests verifying full JS yield stack | completed |

## Architecture

See the feature-level `feature.md` for detailed architecture, directory structure, CLI commands, test mode flows, template files, and design decisions.

### Key Components

- **CLI** (`clap`-based): `wasm-testbed init` and `wasm-testbed test` commands
- **Build pipeline**: cargo build → wasm-bindgen → staging
- **Test runners**: Playwright (browser), Deno, wrangler dev
- **Test discovery**: walrus-based wasm binary parsing for `__wbgt_` exports
- **HTTP server**: `foundation_http` serving integration directories
- **Templates**: embedded via `foundation_macros::EmbedDirectoryAs`

### Test Modes

| Mode | Runner | Harness |
|------|--------|---------|
| `web` | Playwright (Chrome/Firefox/Safari) | User's `integrations/web/index.js` |
| `deno` | Deno | User's `integrations/deno/index.js` |
| `wrangler` | wrangler dev + curl | User's `integrations/wrangler/worker.js` |
| `bindgen-web` | Playwright | Auto-generated from `#[wasm_bindgen_test]` |
| `bindgen-deno` | Deno | Auto-generated from `#[wasm_bindgen_test]` |
| `bindgen-wrangler` | wrangler dev + curl | Auto-generated from `#[wasm_bindgen_test]` |

## Success Criteria

- [x] `cargo check -p foundation_core --target wasm32-unknown-unknown --features js-wasmbindgen` passes
- [x] `wasm_js_yield_integration.rs` created with 4 `#[wasm_bindgen_test]` tests
- [x] `js_yield_returns_immediately` — verifies SpinWaiter returns in < 5ms
- [x] `js_yield_callback_reschedules_executor` — verifies executor re-entry after setTimeout
- [x] `concurrent_d1_queries_make_progress` — verifies concurrent tasks overlap in wall time
- [x] `no_deadlock_with_js_yield` — verifies multi-iteration task completes without deadlock

## Dependencies

- `01-core-wasm-compat` (from spec 28) — base wasm compatibility
- `08-wasm-oauth-manager` (from spec 28) — wasm-bindgen OAuth patterns

---

_Created: 2026-05-31_
_Structure: Feature-based (has_features: true)_
