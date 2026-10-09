# 16 — foundation_deployment Split & Consolidation

**Date:** 2026-07-10
**Status:** Resolved

## Decision

Fix `foundation_deployment` by splitting it cleanly in two:

- **`foundation_deployment`** — the shared library crate. Keeps OpenAPI utilities
  (code-gen helpers, spec fetching/normalization, JSON schema extraction),
  shared API types (`ApiError`, `ApiResponse`, `Operation`), `ProviderClient`
  (StateStore + HTTP bridge), and the `Deployable`/`Deploying` trait (the
  valtron-native async API). No provider implementations. No shell executor.

- **`foundation_deployment_platform`** — the orchestration crate (spec 53
  decision 03). Gets the `Provider` trait (redesigned with associated types),
  all provider backends (QEMU, UTM, Docker), SSH/WinRM, bootstrap, image
  management, state persistence, the CLI, the `ShellExecutor`, and the
  `DeploymentProvider` trait (CLI-wrapper API).

- **Broken sibling shells** — `foundation_deployment_aws`, `_cloudflare`,
  `_gcp`, `_flyio`, `_neon`, `_planetscale`, `_supabase`, `_stripe`,
  `_prisma`, `_mongoatlas` are all broken (empty lib.rs, zero deps, can't
  compile). Fix `_cloudflare` now (decision 15), keep `_huggingface` as-is
  (it works). All other broken shells are ignored — they stay in the
  workspace but are not compiled or maintained until a consumer needs them.

## Table of Contents

1. [Current state](#current-state)
2. [The split](#the-split)
3. [What moves where](#what-moves-where)
4. [Sibling crate disposition](#sibling-crate-disposition)
5. [Trait consolidation](#trait-consolidation)
6. [Implementation plan](#implementation-plan)

---

## Current state

`foundation_deployment` has a split personality problem:

| Role | Modules | Status |
|------|---------|--------|
| **OpenAPI code-gen utilities** | `json_schema`, `resource_info`, `providers/openapi`, `providers/standard/*` | Works, tested, used by code generator |
| **Shared API types** | `providers/common/api_types` (`ApiError`, `ApiResponse`, `Operation`) | Works, used by all generated code |
| **State + HTTP bridge** | `provider_client` (`ProviderClient<S, R>`) | Works |
| **Shell executor** | `core/shell` (`ShellExecutor`) | Works, used by orphaned AWS code |
| **Older deployment trait** | `core/traits` (`DeploymentProvider`) | Zero implementors (AWS is broken) |
| **Newer deployment trait** | `traits` (`Deployable`, `Deploying`) | Zero implementors |
| **Provider detection** | `config` (`DeploymentTarget`), `core/project` | Only 3 providers (Cloudflare, GCP, AWS) |
| **Deployment output types** | `types`, `core/types` | Tightly coupled to specific providers |
| **Error type** | `error` (`DeploymentError`) | 19 variants, mixed concerns |

Meanwhile, 9 of 11 sibling crates are completely broken:

```
foundation_deployment_aws          — 568 lines of REAL code, ZERO deps → can't compile
foundation_deployment_cloudflare   — 256K lines generated, empty lib.rs, undefined features → can't compile
foundation_deployment_gcp          — 1.75M lines generated, empty lib.rs → can't compile
foundation_deployment_flyio        — 39K lines, empty lib.rs → can't compile
foundation_deployment_neon         — 49K lines, cargo new template in lib.rs → can't compile
foundation_deployment_planetscale  — 57K lines, cargo new template in lib.rs → can't compile
foundation_deployment_supabase     — 74K lines, empty lib.rs → can't compile
foundation_deployment_stripe       — 282K lines, empty lib.rs → can't compile
foundation_deployment_prisma       — 22K lines, empty lib.rs → can't compile
foundation_deployment_mongoatlas   — 82 lines, empty lib.rs → can't compile
foundation_deployment_huggingface  — 2,223 lines, proper deps, WORKS (standalone, no dep on foundation_deployment)
```

---

## The split

```
BEFORE:
  foundation_deployment (shared library + deployment ORCHESTRATION)
  ├── OpenAPI utilities         ← library concern
  ├── Shared API types           ← library concern
  ├── ProviderClient             ← library concern
  ├── ShellExecutor              ← orchestration concern
  ├── DeploymentProvider trait   ← orchestration concern
  ├── Deployable trait           ← library concern (valtron-native)
  ├── DeploymentTarget           ← orchestration concern
  └── DeploymentError            ← mixed

AFTER:
  foundation_deployment (SHARED LIBRARY)
  ├── OpenAPI utilities          (json_schema, resource_info, providers/openapi, providers/standard)
  ├── Shared API types           (providers/common/api_types)
  ├── ProviderClient             (provider_client)
  ├── Deployable trait           (traits.rs — valtron-native async API)
  └── OpenApiError               (error types for API interaction only)

  foundation_deployment_platform (ORCHESTRATION)
  ├── Provider trait             (redesigned with associated types — decision 03)
  ├── ShellExecutor              (moved from foundation_deployment/core/shell)
  ├── DeploymentProvider trait   (moved from foundation_deployment/core/traits — CLI-wrapper API)
  ├── DeploymentTarget           (moved from foundation_deployment/config)
  ├── ProjectScanner             (moved from foundation_deployment/core/project)
  ├── PlatformError              (deployment lifecycle errors — split from DeploymentError)
  ├── Docker runtime             (new — decision 07)
  ├── Provider backends          (QEMU, UTM, Docker — from foundation_testbed/src/vms/)
  ├── Guest infrastructure       (SSH, WinRM, bootstrap, images, state — from foundation_testbed)
  └── CLI                        (platform binary, providers subcommands)
```

---

## What moves where

| Source (current) | Destination | Notes |
|-----------------|-------------|-------|
| `foundation_deployment/src/json_schema.rs` | **Stays** in `foundation_deployment` | |
| `foundation_deployment/src/resource_info.rs` | **Stays** in `foundation_deployment` | |
| `foundation_deployment/src/providers/openapi.rs` | **Stays** in `foundation_deployment` | |
| `foundation_deployment/src/providers/standard/` | **Stays** in `foundation_deployment` | |
| `foundation_deployment/src/providers/common/api_types.rs` | **Stays** in `foundation_deployment` | |
| `foundation_deployment/src/provider_client.rs` | **Stays** in `foundation_deployment` | |
| `foundation_deployment/src/traits.rs` (`Deployable`, `Deploying`) | **Stays** in `foundation_deployment` | valtron-native API |
| `foundation_deployment/src/error.rs` | **Split** | `ApiError`-related stays; deployment lifecycle errors → platform |
| `foundation_deployment/src/core/shell.rs` | → `foundation_deployment_platform` | Shell execution is orchestration |
| `foundation_deployment/src/core/traits.rs` (`DeploymentProvider`) | → `foundation_deployment_platform` | |
| `foundation_deployment/src/core/project.rs` (`ProjectScanner`) | → `foundation_deployment_platform` | |
| `foundation_deployment/src/core/types.rs` | → `foundation_deployment_platform` | Deployment lifecycle types |
| `foundation_deployment/src/config.rs` (`DeploymentTarget`) | → `foundation_deployment_platform` | |
| `foundation_deployment/src/types.rs` | → `foundation_deployment_platform` | Provider-specific output types |
| `foundation_testbed/src/vms/` (everything) | → `foundation_deployment_platform` | Provider trait + all backends + guest infra |
| `foundation_testbed/src/vms/providers/` | → `foundation_deployment_platform/src/providers/` | QEMU, UTM, Docker |
| `foundation_testbed/src/vms/ssh/` | → `foundation_deployment_platform/src/ssh/` | |
| `foundation_testbed/src/vms/winrm/` | → `foundation_deployment_platform/src/winrm/` | |
| `foundation_testbed/src/vms/bootstrap/` | → `foundation_deployment_platform/src/bootstrap/` | |
| `foundation_testbed/src/vms/qemu/` | → `foundation_deployment_platform/src/qemu/` | |
| `foundation_testbed/src/vms/config.rs` | → `foundation_deployment_platform/src/config.rs` | VmProfile |
| `foundation_testbed/src/vms/cli/` | → `foundation_deployment_platform/src/cli/` | |
| `foundation_testbed/src/bin/testbed.rs` | → `foundation_deployment_platform/src/bin/platform.rs` | |
| `foundation_deployment_cloudflare` | → **Fix in place** (decision 15) | Transition from auto-gen to hand-maintained |
| `foundation_deployment_huggingface` | → **Keep as-is** | Only working sibling; standalone is fine |
| `foundation_deployment_aws` | → **Move back** into `foundation_deployment_platform` | Orphaned implementation of `DeploymentProvider` |
| Remaining 8 broken shells | → **Ignore** | Stay in workspace; not compiled, not maintained |

---

## Sibling crate disposition

| Crate | Action | Rationale |
|-------|--------|-----------|
| `foundation_deployment_huggingface` | **Keep** | Works, standalone |
| `foundation_deployment_cloudflare` | **Fix** (decision 15) | Needed by `foundation_proxy` for DNS + TLS |
| `foundation_deployment_aws` | **Move code into `foundation_deployment_platform`** | Orphaned 568-line `DeploymentProvider` impl |
| `foundation_deployment_gcp` | **Ignore** | 1.75M lines generated, no consumer, broken |
| `foundation_deployment_flyio` | **Ignore** | 39K lines generated, no consumer, broken |
| `foundation_deployment_neon` | **Ignore** | 49K lines generated, no consumer, broken |
| `foundation_deployment_planetscale` | **Ignore** | 57K lines generated, no consumer, broken |
| `foundation_deployment_supabase` | **Ignore** | 74K lines generated, no consumer, broken |
| `foundation_deployment_stripe` | **Ignore** | 282K lines generated, no consumer, broken |
| `foundation_deployment_prisma` | **Ignore** | 22K lines generated, no consumer, broken |
| `foundation_deployment_mongoatlas` | **Ignore** | 82 lines, never started, broken |

These broken shells stay in the workspace but are not compiled or maintained.
They exist as reference for the code generator. When a consumer needs one
(e.g., Stripe billing integration), we fix its Cargo.toml (add deps + features)
and make it compile at that point.

---

## Trait consolidation

Two competing deployment traits currently exist with zero implementors:

| Trait | Location | Paradigm | Implementors |
|-------|----------|----------|-------------|
| `DeploymentProvider` (older) | `core/traits.rs` | CLI-wrapper, sync, shell-out | 1 (AWS, broken) |
| `Deployable` (newer) | `traits.rs` | valtron-native, async, TaskIterator | 0 |

### Resolution

1. **`Deployable` stays in `foundation_deployment`** — it's the valtron-native
   async API. Depends on `ProviderClient<S, R>` which stays in the shared
   library. This is the future for OpenAPI-generated code that wants to be
   valtron-compatible.

2. **`DeploymentProvider` moves to `foundation_deployment_platform`** —
   alongside the `Provider` trait (decision 03) and the `ShellExecutor`. These
   three together form the CLI-wrapper deployment path. The orphaned AWS code
   moves into the platform crate behind an `aws` feature.

3. **No new `Deployable` implementors yet** — when spec 53's Phase 4
   (cloud deployment) is implemented, the `DockerProvider` itself could
   implement `Deployable` to participate in the valtron-native deployment
   pipeline. But that's future work.

---

## Implementation plan

### Phase 1: Foundation crates

1. Create `backends/foundation_deployment_platform/Cargo.toml` with deps
   (bollard, tokio, ssh2, foundation_deployment, foundation_core,
   foundation_netio, foundation_macros).
2. Move files from `foundation_deployment` → `foundation_deployment_platform`
   (shell, project, config, traits, types — see table above).
3. Move files from `foundation_testbed/src/vms/` →
   `foundation_deployment_platform/src/` (Provider trait, all backends,
   SSH, WinRM, bootstrap, images, state, CLI — see decision 03).
4. Create `foundation_deployment_platform/src/docker/` module (new:
   ContainerHandle, ContainerConfig, WaitFor, DockerClient, etc.).
5. Fix imports, make it compile as a standalone crate.
6. `foundation_testbed` becomes a thin consumer: depends on
   `foundation_deployment_platform`, keeps `src/wasm/` + thin CLI wrappers.

### Phase 2: Fix foundation_deployment_cloudflare

7. Implement decision 15 phases 1-3 (core types, HTTP client, DNS operations).
8. This makes it the second working sibling (alongside huggingface).

### Phase 3: Ignore broken shells

9. Eight broken siblings (`_gcp`, `_flyio`, `_neon`, `_planetscale`, `_supabase`,
   `_stripe`, `_prisma`, `_mongoatlas`) stay where they are — not compiled, not
   maintained, not moved. Reference material for the code generator only.
10. Move orphaned AWS code into `foundation_deployment_platform/src/providers/aws/`.
11. Exclude broken shells from any workspace-wide builds (they're already
    excluded by the existing workspace config — `backends/*` glob includes them
    but each has a broken Cargo.toml that prevents compilation).

### Phase 4: Clean up foundation_deployment

12. `foundation_deployment` is now a clean shared library — OpenAPI utilities
    + `ProviderClient` + `Deployable` trait + shared API types.
13. Split `error.rs` — API-level errors stay, deployment lifecycle errors
    move to platform.
14. Remove stale feature flags (`aws`, `huggingface`, `mongodb`, `standard`).
15. Update Cargo.toml description: "Shared utilities for deployment providers".
