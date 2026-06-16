---
feature: "Tier 3 Infrastructure Publishing"
description: "Publish 6 infrastructure crates (core, http, netio, signals, ui_traits, theme) to crates.io"
status: "pending"
priority: "high"
depends_on: ["01-metadata-completion", "02-tier1-leaf-publishing", "03-tier2-core-substrate-publishing"]
estimated_effort: "small"
created: 2026-06-16
last_updated: 2026-06-16
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 6
  total: 6
  completion_percentage: 0%
---

# Feature 04: Tier 3 Infrastructure Publishing

## WHY: Problem Statement

Tier 3 crates provide the core infrastructure types and traits that domain crates build on: HTTP serving, network I/O, reactive signals, and UI theming.

## WHAT: Crates to Publish

| # | Crate | Version | Description | Depends On |
|---|-------|---------|-------------|------------|
| 1 | `foundation_core` | 0.0.3 | Central crate for all foundation crates — core types and traits | Tier 1 + 2 |
| 2 | `foundation_http` | 0.0.1 | Connection-owned, worker-pooled HTTP serving framework | Tier 1 + 2 |
| 3 | `foundation_netio` | 0.0.1 | Network I/O — async HTTP/WebSocket/SOCKS5, TLS, compression, wasm | Tier 1 + 2 |
| 4 | `foundation_signals` | 0.0.1 | Standalone reactive signal system — R3-style height-ordered graph | Tier 1 + 2 |
| 5 | `foundation_ui_traits` | 0.0.1 | Shared UI types: IntoHtml, Html struct, Part descriptors, DomOp enum | Tier 1 + 2 |
| 6 | `foundation_theme` | 0.0.1 | Design-token theme model + CSS generator shared by theme! macro | Tier 1 + 2 |

## HOW: Per-Crate Publish Steps

For each crate in order above:

1. **Verify metadata** — description, readme, license, repository present
2. **Run tests** — `cargo test -p <crate>` — all must pass
3. **Dry-run publish** — `cargo publish --dry-run -p <crate>` — no errors
4. **Publish** — `cargo publish -p <crate>`
5. **Verify on crates.io**

### Success Criteria

- All 6 crates pass `cargo test` and `cargo publish --dry-run`
- All 6 crates visible on crates.io with correct version
