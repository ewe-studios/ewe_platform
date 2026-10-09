---
feature: "crates.io Publishing — name validation and deployment plan"
description: "Validate all 38 crate names against crates.io for conflicts, determine final crate naming, and create feature plans for publishing each crate to crates.io"
status: "in-progress"
priority: "high"
depends_on: []
estimated_effort: "medium"
created: 2026-06-16
last_updated: 2026-06-16
author: "Main Agent"
tasks:
  completed: 3
  uncompleted: 0
  total: 3
  completion_percentage: 100%
---

# Feature: crates.io Publishing Plan

## WHY: Problem Statement

The ewe_platform workspace contains **38 crates** across `backends/` and `infrastructure/` that need to be published to crates.io so they can be consumed by external projects. Before publishing, we must:

1. Validate that no name conflicts exist on crates.io
2. Resolve any naming conflicts with the user
3. Create a deployment plan for each crate (order, dependencies, publish settings)

## WHAT: crates.io Name Validation Results

All 38 crate names have been checked against crates.io (2026-06-16). Results:

### ✅ ALL 38 NAMES AVAILABLE (no conflicts)

| # | Crate Name | Path | Current Version | Status |
|---|---|---|---|---|
| 1 | `foundation_ai` | backends/foundation_ai | 0.0.1 | ✅ Available |
| 2 | `foundation_arrow` | backends/foundation_arrow | 0.0.1 | ✅ Available |
| 3 | `foundation_auth` | backends/foundation_auth | 0.0.1 | ✅ Available |
| 4 | `foundation_browser` | backends/foundation_browser | 0.0.1 | ✅ Available |
| 5 | `foundation_cedar` | backends/foundation_cedar | — | ✅ Available |
| 6 | `foundation_codegen` | backends/foundation_codegen | 0.1.0 | ✅ Available |
| 7 | `foundation_codegentools` | backends/foundation_codegentools | 0.1.0 | ✅ Available |
| 8 | `foundation_compact` | backends/foundation_compact | 0.1.0 | ✅ Available |
| 9 | `foundation_conditional` | backends/foundation_conditional | 0.1.0 | ✅ Available |
| 10 | `foundation_config` | backends/foundation_config | 0.1.0 | ✅ Available |
| 11 | `foundation_core` | backends/foundation_core | 0.0.3 | ✅ Available |
| 12 | `foundation_db` | backends/foundation_db | 0.0.1 | ✅ Available |
| 13 | `foundation_deployment` | backends/foundation_deployment | 0.0.1 | ✅ Available |
| 14 | `foundation_errstacks` | backends/foundation_errstacks | 0.0.1 | ✅ Available |
| 15 | `foundation_html` | backends/foundation_html | 0.0.1 | ✅ Available |
| 16 | `foundation_http` | backends/foundation_http | 0.0.1 | ✅ Available |
| 17 | `foundation_jsonschema` | backends/foundation_jsonschema | 0.0.1 | ✅ Available |
| 18 | `foundation_logging` | backends/foundation_logging | — | ✅ Available |
| 19 | `foundation_macros` | backends/foundation_macros | 0.0.4 | ✅ Available |
| 20 | `foundation_nativeapis` | backends/foundation_nativeapis | 0.0.1 | ✅ Available |
| 21 | `foundation_netio` | backends/foundation_netio | 0.0.1 | ✅ Available |
| 22 | `foundation_nostd` | backends/foundation_nostd | 0.0.4 | ✅ Available |
| 23 | `foundation_openapi` | backends/foundation_openapi | 0.0.1 | ✅ Available |
| 24 | `foundation_packager` | backends/foundation_packager | 0.1.0 | ✅ Available |
| 25 | `foundation_rng` | backends/foundation_rng | — | ✅ Available |
| 26 | `foundation_runtimes` | backends/foundation_runtimes | 0.0.3 | ✅ Available |
| 27 | `foundation_shell` | backends/foundation_shell | — | ✅ Available |
| 28 | `foundation_signals` | backends/foundation_signals | 0.0.1 | ✅ Available |
| 29 | `foundation_testbed` | backends/foundation_testbed | 0.0.1 | ✅ Available |
| 30 | `foundation_testing` | backends/foundation_testing | 0.0.1 | ✅ Available |
| 31 | `foundation_theme` | backends/foundation_theme | 0.0.1 | ✅ Available |
| 32 | `foundation_toolings` | backends/foundation_toolings | 0.0.1 | ✅ Available |
| 33 | `foundation_ui_components` | backends/foundation_ui_components | 0.1.0 | ✅ Available |
| 34 | `foundation_ui_traits` | backends/foundation_ui_traits | 0.0.1 | ✅ Available |
| 35 | `foundation_wasm` | backends/foundation_wasm | 0.0.2 | ✅ Available |
| 36 | `foundation_wasm_ui` | backends/foundation_wasm_ui | 0.1.0 | ✅ Available |
| 37 | `infrastructure_llama_bindings` | infrastructure/llama-bindings | — | ✅ Available |
| 38 | `infrastructure_llama_cpp` | infrastructure/llama-cpp | — | ✅ Available |

> **Note:** Initial scan flagged 4 names (`foundation_core`, `foundation_macros`, `foundation_nostd`, `foundation_wasm`) as potentially taken — but direct verification confirmed they are all available on crates.io (possibly yanked previously or stale cache).

### No renaming required. All current names can be used as-is.

## HOW: Publishing Prerequisites

Before any crate can be published, each needs:

### 1. Required `Cargo.toml` fields for crates.io

Every published crate must have these fields:
- ✅ `name` — all present
- ✅ `version` — all present
- ✅ `edition` — workspace-inherited
- ✅ `license` — workspace-inherited (`Apache-2.0`)
- ✅ `authors` — workspace-inherited (`EweStudios Consulting Limited`)
- ⚠️ `description` — some crates missing or have placeholder text
- ⚠️ `repository` — workspace-inherited, but verify URL is correct
- ⚠️ `readme` — should point to a README.md in each crate
- ⚠️ `documentation` — optional but recommended (docs.rs auto-builds)

### 2. Publish ordering (dependency DAG)

Crates must be published in topological order — leaf dependencies first.

### 3. CI/CD for publishing

- GitHub Actions workflow for automated `cargo publish`
- Requires `CARGO_REGISTRY_TOKEN` secret
- Should run on tagged releases

### 4. Version bumping strategy

- SemVer: `0.0.x` for initial unstable, `0.x.0` for feature additions
- Workspace `cargo workspaces publish` or `release-plz` for coordinated releases

## Sub-Features: Per-Crate Deployment

Each crate below has its own feature file tracking what's needed to publish it.

### Publishing Order (topological)

Based on the dependency graph, crates should be published in these tiers:

#### Tier 1 — Leaf crates (no internal dependencies, publish first)
1. `foundation_errstacks` — error handling
2. `foundation_nostd` — nostd substrate
3. `foundation_macros` — proc macros
4. `foundation_conditional` — conditional logic
5. `foundation_testing` — testing utilities
6. `foundation_rng` — random number generation
7. `infrastructure_llama_bindings` — llama bindings
8. `infrastructure_llama_cpp` — llama cpp bindings

#### Tier 2 — Core substrate
9. `foundation_compact` — cross-platform substrate (time, entropy, RNG, scru128)
10. `foundation_config` — configuration
11. `foundation_logging` — logging/tracing

#### Tier 3 — Infrastructure
12. `foundation_core` — core foundation types and traits
13. `foundation_http` — HTTP utilities
14. `foundation_netio` — network I/O
15. `foundation_signals` — reactive signals
16. `foundation_ui_traits` — UI trait definitions
17. `foundation_theme` — theming

#### Tier 4 — Domain crates
18. `foundation_auth` — authentication
19. `foundation_db` — database
20. `foundation_jsonschema` — JSON Schema validation
21. `foundation_codegen` — code generation
22. `foundation_codegentools` — codegen tools
23. `foundation_packager` — packaging
24. `foundation_runtimes` — runtime abstractions
25. `foundation_shell` — shell/execution
26. `foundation_toolings` — tooling utilities
27. `foundation_testbed` — testbed

#### Tier 5 — Higher-order crates
28. `foundation_ai` — AI/LLM integration
29. `foundation_arrow` — Arrow serialization
30. `foundation_browser` — browser APIs
31. `foundation_cedar` — Cedar policy engine
32. `foundation_deployment` — deployment abstractions
33. `foundation_html` — HTML rendering
34. `foundation_nativeapis` — native API bindings
35. `foundation_openapi` — OpenAPI generation
36. `foundation_ui_components` — UI components
37. `foundation_wasm` — WASM runtime
38. `foundation_wasm_ui` — WASM UI integration

> **Note:** `infrastructure_llama_*` crates may not need publishing if they're thin FFI bindings meant for internal use only. Confirm with user.

## TODOs

- [x] Check all 38 crate names on crates.io for conflicts → **ALL AVAILABLE**
- [x] User review: confirm crate names as-is → **CONFIRMED, no renames**
- [x] Add missing `description` fields to 4 crates (auth, cedar, logging, netio)
- [x] For each crate, create feature file with specific publish checklist
- [ ] Determine publish token/credentials setup
- [ ] Set up CI/CD workflow for automated publishing
