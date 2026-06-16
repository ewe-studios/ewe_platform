---
feature: "Tier 2 Core Substrate Publishing"
description: "Publish 3 core substrate crates (compact, config, logging) to crates.io"
status: "pending"
priority: "high"
depends_on: ["01-metadata-completion", "02-tier1-leaf-publishing"]
estimated_effort: "small"
created: 2026-06-16
last_updated: 2026-06-16
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 3
  total: 3
  completion_percentage: 0%
---

# Feature 03: Tier 2 Core Substrate Publishing

## WHY: Problem Statement

Tier 2 crates build on Tier 1 leaf crates. They provide the cross-platform substrate (time, entropy, RNG), configuration loading, and logging infrastructure that Tier 3+ crates depend on.

## WHAT: Crates to Publish

| # | Crate | Version | Description | Depends On |
|---|-------|---------|-------------|------------|
| 1 | `foundation_compact` | 0.1.0 | Cross-platform substrate: time polyfill + vendored entropy + RNG + scru128 IDs | Tier 1 (nostd, macros, rng) |
| 2 | `foundation_config` | 0.1.0 | TOML-based configuration file loading — pure std::fs + serde + toml | Tier 1 |
| 3 | `foundation_logging` | 0.0.1 | Structured logging built on tracing — configurable subscribers with env-filter | Tier 1 |

## HOW: Per-Crate Publish Steps

For each crate in order above:

1. **Verify metadata** — description, readme, license, repository present
2. **Run tests** — `cargo test -p <crate>` — all must pass
3. **Dry-run publish** — `cargo publish --dry-run -p <crate>` — no errors
4. **Publish** — `cargo publish -p <crate>`
5. **Verify on crates.io**

### Success Criteria

- All 3 crates pass `cargo test` and `cargo publish --dry-run`
- All 3 crates visible on crates.io with correct version
