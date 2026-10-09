---
feature: "Tier 4 Domain Crates Publishing"
description: "Publish 10 domain crates (auth, db, jsonschema, codegen, codegentools, packager, runtimes, shell, toolings, testbed) to crates.io"
status: "pending"
priority: "high"
depends_on: ["01-metadata-completion", "02-tier1-leaf-publishing", "03-tier2-core-substrate-publishing", "04-tier3-infra-publishing"]
estimated_effort: "medium"
created: 2026-06-16
last_updated: 2026-06-16
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 10
  total: 10
  completion_percentage: 0%
---

# Feature 05: Tier 4 Domain Crates Publishing

## WHY: Problem Statement

Tier 4 crates provide domain-specific functionality: authentication, database access, JSON Schema validation, code generation, packaging, runtime assets, shell integration, tooling servers, and testbeds. These depend on all lower tiers.

## WHAT: Crates to Publish

| # | Crate | Version | Description | Depends On |
|---|-------|---------|-------------|------------|
| 1 | `foundation_auth` | 0.0.1 | OAuth2/OIDC, JWT, password hashing (Argon2), TOTP, session management | Tier 1-3 |
| 2 | `foundation_db` | 0.0.1 | Unified storage — Turso sync API, D1, R2, in-memory fallback | Tier 1-3 |
| 3 | `foundation_jsonschema` | 0.0.1 | Self-contained JSON Schema validation | Tier 1-3 |
| 4 | `foundation_codegen` | 0.1.0 | Build-time source scanning for macro-annotated Rust items | Tier 1-3 |
| 5 | `foundation_codegentools` | 0.1.0 | WASM binary generation, schema codegen, crate scanning | Tier 1-3 |
| 6 | `foundation_packager` | 0.1.0 | File generation and templating stack — projects from embedded templates | Tier 1-3 |
| 7 | `foundation_runtimes` | 0.0.3 | Central crate for runtime assets accessible as embedded files | Tier 1-3 |
| 8 | `foundation_shell` | 0.0.1 | nushell integrated library for embedding nushell | Tier 1-3 |
| 9 | `foundation_toolings` | 0.0.1 | Development tooling server: reverse proxy, file watcher, hot reload | Tier 1-3 |
| 10 | `foundation_testbed` | 0.0.1 | Holistic test harness: QEMU/KVM VM testbed + wasm32 browser/Deno/Workers | Tier 1-3 |

## HOW: Per-Crate Publish Steps

For each crate in order above:

1. **Verify metadata** — description, readme, license, repository present
2. **Run tests** — `cargo test -p <crate>` — all must pass
3. **Dry-run publish** — `cargo publish --dry-run -p <crate>` — no errors
4. **Publish** — `cargo publish -p <crate>`
5. **Verify on crates.io**

### Success Criteria

- All 10 crates pass `cargo test` and `cargo publish --dry-run`
- All 10 crates visible on crates.io with correct version
