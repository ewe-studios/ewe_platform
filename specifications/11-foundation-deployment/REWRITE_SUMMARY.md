# Foundation Deployment Specification Rewrite Summary

**Date:** 2026-04-11
**Status:** Complete

---

## Overview

Rewrote the foundation deployment specification to replace **configuration-driven deployments** with **trait-based deployments** — pure Rust code with zero custom configuration formats.

## What Changed

### Old Approach (Rejected)

- YAML deployment plans (`.deployment-plan.yaml`)
- Custom parsers and validators
- State machine deployment engine
- Complex infrastructure plans

**Why rejected:** Added unnecessary complexity. Users should write Rust code, not YAML.

### New Approach (Feature 35)

```rust
// User defines infrastructure as Rust types
struct MyWorker {
    name: String,
    script: String,
}

// Implement Deployable trait
impl Deployable for MyWorker {
    type Output = WorkerDeployment;
    type Error = DeploymentError;
    
    async fn deploy(&self) -> Result<Self::Output, Self::Error> {
        let client = CloudflareClient::from_env()?;
        client.put_worker_script(&self.name, &self.script).await
    }
}

// Use it
let worker = MyWorker { ... };
let result = worker.deploy().await?;
```

**Benefits:**
- No YAML, TOML, or custom configs
- Full type safety
- Compose with regular Rust code
- Test with regular Rust tests
- Reuse across projects

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│              User's Deployable Implementation               │
│                                                               │
│  struct MyInfrastructure { ... }                             │
│                                                               │
│  impl Deployable for MyInfrastructure {                      │
│      async fn deploy(&self) -> Result<Output, Error> {       │
│          // Call provider clients directly                   │
│      }                                                       │
│  }                                                           │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                  Provider Clients                           │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐          │
│  │ Cloudflare  │  │    GCP      │  │    AWS      │  ...     │
│  │   Client    │  │   Client    │  │   Client    │          │
│  └─────────────┘  └─────────────┘  └─────────────┘          │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                   State Store (automatic)                    │
│  - Change detection (via provider wrappers)                  │
│  - State persistence (after successful deploy)               │
│  - Rollback support (on failure)                             │
└─────────────────────────────────────────────────────────────┘
```

---

## Feature File Status

### Complete (Verified Implementation)

| Feature | Name | Status | Notes |
|---------|------|--------|-------|
| 01 | foundation-deployment-core | **COMPLETE** | `DeploymentProvider` trait, types, errors |
| 02 | state-stores | **COMPLETE** | All 6 backends with project/stage namespacing |
| 04 | cloudflare-cli-provider | **COMPLETE** | Wrangler CLI wrapper |
| 05 | gcp-cloud-run-cli-provider | **COMPLETE** | gcloud CLI wrapper |
| 06 | aws-lambda-cli-provider | **COMPLETE** | SAM/AWS CLI wrapper |
| 07 | provider-api-clients | **COMPLETE** | API clients for all providers |
| 30 | huggingface-api-provider | **COMPLETE** | HuggingFace Hub API client |

### Rejected (User-Facing API)

| Feature | Name | Reason |
|---------|------|--------|
| 03 | deployment-engine | Replaced by trait-based approach (Feature 35) - engine remains as internal implementation |
| 11 | templates | YAML plan generation rejected - users write Rust code directly |
| 12 | mise-integration | Plan-centric tasks rejected - mise tasks run Rust binaries |
| 31 | deployment-plan-schema | YAML plans rejected in favor of Rust types |
| 32 | deployment-plan-parser | No YAML plans = no parser needed |
| 33 | infrastructure-plan | YAML infra plans rejected - use Rust code |
| 34 | plan-executor | Plan executor rejected - use trait-based deployments |

### Internal Implementation

| Feature | Name | Status | Notes |
|---------|------|--------|-------|
| 03 | deployment-engine | **INTERNAL** | State machine for provider internal use |
| 28 | provider-wrapper | **INTERNAL** | Optional state-tracking wrappers for API clients |

### New

| Feature | Name | Description |
|---------|------|-------------|
| 35 | trait-based-deployments | `Deployable` trait for pure Rust deployments |

---

## Key Design Decisions

### 1. No Configuration Formats

**Decision:** Zero YAML, TOML, or custom configuration formats. Everything is Rust code.

**Rationale:**
- Type safety at compile time
- Refactoring support from IDEs
- Unit test deployments
- Compose with regular Rust patterns

### 2. Deployable Trait

**Decision:** Single trait with associated types for output and errors.

```rust
pub trait Deployable: Send + Sync {
    type Output: Send + Sync;
    type Error: std::error::Error + Send + Sync;
    
    async fn deploy(&self) -> Result<Self::Output, Self::Error>;
}
```

**Rationale:**
- Minimal API surface
- Flexible output types per deployment
- Error handling per deployment

### 3. Provider Clients Called Directly

**Decision:** User implementations call provider clients directly.

**Rationale:**
- No abstraction leakage
- Full control over deployment logic
- Clear dependency chain

### 4. State Store Optional

**Decision:** State stores are available via provider wrappers, but users can also call raw clients.

**Rationale:**
- Transparent change detection when needed
- Automatic state persistence
- Optional - simple deployments may not need it

---

## Implementation Status Summary

| Component | Status |
|-----------|--------|
| Core trait & types | **COMPLETE** |
| State stores | **COMPLETE** |
| CLI providers | **COMPLETE** |
| API clients | **COMPLETE** |
| Deployable trait | **PENDING** (Feature 35) |
| Common output types | **PENDING** (Feature 35) |
| Templates | **PENDING** (update for Rust codegen) |
| Examples | **PENDING** |

---

## Next Steps

1. **Implement Deployable trait** (Feature 35)
2. **Create common output types** (Feature 35)
3. **Update provider clients** to return standard output types
4. **Create example deployments**
5. **Update templates** to generate Rust code

---

## Files Modified

| File | Action |
|------|--------|
| `specifications/11-foundation-deployment/features/03-deployment-engine/feature.md` | Marked as rejected (user-facing), INTERNAL (implementation) |
| `specifications/11-foundation-deployment/features/11-templates/feature.md` | Marked as rejected |
| `specifications/11-foundation-deployment/features/12-mise-integration/feature.md` | Marked as rejected |
| `specifications/11-foundation-deployment/features/28-provider-wrapper/feature.md` | Marked as INTERNAL |
| `specifications/11-foundation-deployment/features/31-deployment-plan-schema/feature.md` | Deleted (rejected) |
| `specifications/11-foundation-deployment/features/32-deployment-plan-parser/feature.md` | Deleted (rejected) |
| `specifications/11-foundation-deployment/features/33-infrastructure-plan/feature.md` | Deleted (rejected) |
| `specifications/11-foundation-deployment/features/34-plan-executor/feature.md` | Deleted (rejected) |
| `specifications/11-foundation-deployment/features/35-trait-based-deployments/feature.md` | Created new |
| `specifications/11-foundation-deployment/REWRITE_SUMMARY.md` | Updated |

---

_Created: 2026-04-11_
_This summary reflects the trait-based deployment architecture._
