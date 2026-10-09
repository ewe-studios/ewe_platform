---
feature: "Tier 5 Higher-Order Crates Publishing"
description: "Publish 11 higher-order crates (ai, arrow, browser, cedar, deployment, html, nativeapis, openapi, ui_components, wasm, wasm_ui) to crates.io"
status: "pending"
priority: "high"
depends_on: ["01-metadata-completion", "02-tier1-leaf-publishing", "03-tier2-core-substrate-publishing", "04-tier3-infra-publishing", "05-tier4-domain-publishing"]
estimated_effort: "medium"
created: 2026-06-16
last_updated: 2026-06-16
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 11
  total: 11
  completion_percentage: 0%
---

# Feature 06: Tier 5 Higher-Order Crates Publishing

## WHY: Problem Statement

Tier 5 crates are the highest-level crates — they provide AI integration, Arrow serialization, browser testing, Cedar policy integration, deployment abstractions, HTML generation, native API bindings, OpenAPI processing, UI components, and WASM runtime support. These depend on all lower tiers.

## WHAT: Crates to Publish

| # | Crate | Version | Description | Depends On |
|---|-------|---------|-------------|------------|
| 1 | `foundation_ai` | 0.0.1 | AI/LLM integration — model providers, prompt management, tool use | Tier 1-4 |
| 2 | `foundation_arrow` | 0.0.1 | Arrow zero-copy serialization — native, WASI, WASM | Tier 1-4 |
| 3 | `foundation_browser` | 0.0.1 | Pure-Rust browser test driver (CDP) — JSON-RPC protocol | Tier 1-4 |
| 4 | `foundation_cedar` | 0.0.1 | Cedar policy engine — fine-grained authorization with DB-backed storage | Tier 1-4 |
| 5 | `foundation_deployment` | 0.0.1 | Multi-cloud deployment providers | Tier 1-4 |
| 6 | `foundation_html` | 0.0.1 | Web and non-web HTML generation | Tier 1-4 |
| 7 | `foundation_nativeapis` | 0.0.1 | Cross-platform native APIs: I/O polling, io_uring, file watching | Tier 1-4 |
| 8 | `foundation_openapi` | 0.0.1 | OpenAPI spec processing and normalization | Tier 1-4 |
| 9 | `foundation_ui_components` | 0.1.0 | Headless UI components over foundation_wasm_ui — data-attribute styling | Tier 1-4 |
| 10 | `foundation_wasm` | 0.0.2 | nostd interface runtime for WASM and JS interoperability | Tier 1-4 |
| 11 | `foundation_wasm_ui` | 0.1.0 | DOM/window/animation bindings + WASM protocol implementations | Tier 1-4 |

## HOW: Per-Crate Publish Steps

For each crate in order above:

1. **Verify metadata** — description, readme, license, repository present
2. **Run tests** — `cargo test -p <crate>` — all must pass
3. **Dry-run publish** — `cargo publish --dry-run -p <crate>` — no errors
4. **Publish** — `cargo publish -p <crate>`
5. **Verify on crates.io**

### Success Criteria

- All 11 crates pass `cargo test` and `cargo publish --dry-run`
- All 11 crates visible on crates.io with correct version
