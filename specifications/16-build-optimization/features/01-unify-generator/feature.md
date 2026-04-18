---
name: "unify-generator"
description: "Combine gen_resource_types, gen_provider_clients, gen_provider_wrappers into single gen_api command"
status: "pending"
priority: "critical"
created: 2026-04-18
author: "Main Agent"
metadata:
  version: "2.0"
  estimated_effort: "high"
  tags:
    - generator
    - codegen
    - refactoring
    - build-optimization
dependencies: []
features: []
---

# Unify Generator

## Overview

The current generator architecture is **split across three separate commands** under `gen_resources`:

**Current split:**
```
cargo run --bin ewe_platform gen_resources types       # Types only
cargo run --bin ewe_platform gen_resources clients     # Clients only  
cargo run --bin ewe_platform gen_resources providers   # Provider wrappers only
```

**Problem:** Three independent generators produce disjoint artifacts:
- `types.rs` generates resource types with no knowledge of which clients use them
- `clients.rs` generates client functions with no knowledge of which provider wrappers need them
- `provider_wrappers.rs` generates wrapper methods with no knowledge of the client functions

This creates:
- No intelligent grouping - all 306 GCP APIs in one monolithic crate
- Feature flags defined but unwirable (can't scope types/clients/providers together)
- 512,000 lines compiling monolithically when any `gcp` feature is enabled

**Target unified command:**
```
cargo run --bin ewe_platform gen_api gcp --spec gcp-openapi.json
```

**Target unified architecture:**

A **single generator** that produces cohesive units per API group, where each endpoint's types, client functions, and provider wrapper are generated together in a repeating pattern:

```rust
// For each API group (e.g., "gcp_run"):

// =============================================================================
// ENDPOINT: instances.get
// =============================================================================

// --- Resource types ---
pub struct Instance { ... }
pub struct GetInstanceResponse { ... }
pub struct GetInstanceArgs { ... }

// --- Client functions ---
pub fn instances_get_builder(...) -> ClientRequestBuilder { ... }
pub fn instances_get_task(...) -> TaskIterator { ... }
pub fn instances_get_execute(...) -> StreamIterator { ... }
pub fn instances_get(...) -> StreamIterator { ... }

// --- ProviderClient wrapper method ---
impl<S, R> ProviderClient<S, R> {
    pub fn gcp_run_instances_get(&self, args: &GetInstanceArgs) -> Result<...> {
        instances_get_execute(instances_get_builder(...))
    }
}

// =============================================================================
// ENDPOINT: instances.create
// =============================================================================

// --- Resource types ---
pub struct CreateInstanceRequest { ... }
pub struct CreateInstanceResponse { ... }
pub struct CreateInstanceArgs { ... }

// --- Client functions ---
pub fn instances_create_builder(...) -> ClientRequestBuilder { ... }
pub fn instances_create_task(...) -> TaskIterator { ... }
pub fn instances_create_execute(...) -> StreamIterator { ... }
pub fn instances_create(...) -> StreamIterator { ... }

// --- ProviderClient wrapper method ---
impl<S, R> ProviderClient<S, R> {
    pub fn gcp_run_instances_create(&self, args: &CreateInstanceArgs) -> Result<...> {
        instances_create_execute(instances_create_builder(...))
    }
}
```

This ensures:
- **Each endpoint is fully self-contained** - types + clients + wrapper in one place
- **Finding implementation requires looking at ONE section per endpoint** - no jumping between files
- **Feature flags scope all artifacts together** - `gcp_run` enables complete endpoints
- **Unused groups don't compile at all** - true zero-cost unused code

## Architecture

### Current State (Broken)

**Three split generators producing disjoint artifacts:**
```
bin/platform/src/gen_resources/
├── types.rs            # Generates types only
├── clients.rs          # Generates functions only
└── provider_wrappers.rs # Generates wrapper structs only
```

Each operates independently → no grouping intelligence → 306 APIs in one crate.

### Target Architecture (Unified)

**Single generator producing cohesive units per API group:**

```
foundation_openapi/src/
├── unified_generator.rs    # SINGLE generator that produces per-endpoint units:
│                           # 1. Resource types
│                           # 2. Client functions
│                           # 3. ProviderClient impl block
├── analyzer/
│   ├── grouping.rs         # Group endpoints (10-200 per group)
│   └── shared.rs           # Detect shared resources
└── ...
```

**Output structure per provider:**
```
backends/foundation_deployment/src/providers/gcp/
├── mod.rs                  # Re-exports with feature guards
├── shared/                 # Types used by multiple groups
│   └── mod.rs              # Common types (Location, IAMPolicy, etc.)
├── run/                    # gcp_run group (~15 endpoints)
│   └── mod.rs              # Resources + Clients + ProviderClient impl per endpoint
├── compute/                # gcp_compute group (~180 endpoints)
│   └── mod.rs              # Resources + Clients + ProviderClient impl per endpoint
└── jobs/                   # gcp_jobs group (~12 endpoints)
    └── mod.rs              # Resources + Clients + ProviderClient impl per endpoint
```

**Each group module structure (per-endpoint cohesion):**
```rust
// providers/gcp/run/mod.rs
#![cfg(feature = "gcp_run")]

// === ENDPOINT 1: instances.get ===

// Resource types
pub struct Instance { ... }
pub struct GetInstanceResponse { ... }

// Args type for convenience functions
pub struct GetInstanceArgs { ... }

// Client functions
pub fn instances_get_builder(...) -> ClientRequestBuilder { ... }
pub fn instances_get_task(...) -> TaskIterator { ... }
pub fn instances_get_execute(...) -> StreamIterator { ... }
pub fn instances_get(...) -> StreamIterator { ... }

// ProviderClient impl
impl<S, R> ProviderClient<S, R> {
    pub fn gcp_run_instances_get(&self, args: &GetInstanceArgs) -> Result<...> {
        instances_get_execute(instances_get_builder(...))
    }
}

// === ENDPOINT 2: instances.create ===
// ... same pattern ...
```

**Feature flag wiring:**
```toml
# foundation_deployment/Cargo.toml
[features]
gcp = ["gcp_run", "gcp_compute", "gcp_jobs"]
gcp_run = []
gcp_compute = []
gcp_jobs = []
```

```rust
// providers/gcp/mod.rs
#[cfg(feature = "gcp_run")]
pub mod run;
#[cfg(feature = "gcp_compute")]
pub mod compute;
#[cfg(feature = "gcp_jobs")]
pub mod jobs;
```

### CLI Interface

```
Usage: ewe_platform gen_api <PROVIDER> --spec <PATH> [OPTIONS]

Arguments:
  <PROVIDER>    Provider name (gcp, cloudflare, stripe, etc.)
  --spec        Path to OpenAPI spec JSON file

Options:
  --output-dir    Output directory (default: backends/foundation_deployment/src/providers)
  --dry-run       Analyze and show grouping without writing files
  --features      Generate with per-group feature flags (default: true)
  --min-group-size=N   Minimum endpoints per group (default: 10)
  --max-group-size=N   Maximum endpoints per group (default: 200)
```

## Requirements

### Phase 1: Implement Unified Generator in foundation_openapi

Create `foundation_openapi/src/unified_generator.rs`:

```rust
pub struct UnifiedGenerator {
    artefacts_dir: PathBuf,
    output_dir: PathBuf,
}

impl UnifiedGenerator {
    /// Generate all artifacts for a provider as cohesive per-endpoint units.
    pub fn generate(&self, provider: &str) -> Result<(), GenError> {
        // 1. Load and analyze spec
        // 2. Group endpoints (10-200 per group)
        // 3. Detect shared resources
        // 4. For each group, generate module with per-endpoint units:
        //    For each endpoint:
        //      a. Resource types (structs, Args types)
        //      b. Client functions (builder, task, execute, convenience)
        //      c. ProviderClient impl block with wrapper method
        // 5. Generate shared/ module for cross-group types
        // 6. Generate mod.rs with feature guards
    }
}
```

**Key insight:** The generator produces ONE module per group where each endpoint contains:
- Types (response types, Args types)
- Client functions (builder, task, execute, convenience)
- ProviderClient impl block (wrapper method that calls the client function)

All three are guarded by the same feature flag. Unused groups don't compile at all.

### Phase 2: Endpoint Grouping Algorithm

In `foundation_openapi/src/analyzer/grouping.rs`:

```rust
pub fn group_endpoints(endpoints: &[EndpointInfo]) -> Vec<ApiGroup> {
    // 1. Group by path prefix (/v1/projects → "projects")
    // 2. Merge groups < 10 endpoints with adjacent groups
    // 3. Split groups > 200 endpoints by sub-prefix
    // 4. Return grouped endpoints
}
```

### Phase 3: Shared Resource Detection

In `foundation_openapi/src/analyzer/shared.rs`:

```rust
pub fn detect_shared_resources(groups: &[ApiGroup]) -> Vec<String> {
    // 1. Track which types each group uses
    // 2. Types used by 2+ groups are "shared"
    // 3. Return list of shared type names
}
```

### Phase 4: Feature Flag Wiring

Update `foundation_deployment/Cargo.toml`:

```toml
[features]
# Provider-level features enable groups
gcp = ["gcp_run", "gcp_compute", "gcp_jobs"]
gcp_run = []
gcp_compute = []

# Each group is independent - compile only when enabled
```

Update `foundation_deployment/src/providers/gcp/mod.rs`:

```rust
#[cfg(feature = "gcp_run")]
pub mod run;

#[cfg(feature = "gcp_compute")]
pub mod compute;
```

### Phase 5: CLI Command

Create `bin/platform/src/gen_api.rs`:

```rust
pub fn run(args: &GenApiArgs) -> Result<()> {
    let generator = UnifiedGenerator::new(artefacts_dir, output_dir);
    generator.generate(&args.provider)?;
    Ok(())
}
```

## Verification

```bash
# Phase 1: Verify unified generator produces correct output
cargo run --bin ewe_platform gen_api gcp --spec gcp-openapi.json

# Should generate:
# - providers/gcp/run/mod.rs (resources + clients + impl per endpoint)
# - providers/gcp/compute/mod.rs (resources + clients + impl per endpoint)
# - providers/gcp/jobs/mod.rs (resources + clients + impl per endpoint)

# Phase 2: Dry run - should show grouping analysis
cargo run --bin ewe_platform gen_api gcp --spec gcp-openapi.json --dry-run

# Should show:
# - Number of groups detected
# - Endpoints per group
# - Resources per group  
# - Shared resources identified

# Phase 3: Verify feature flag scoping works
cargo check -p foundation_deployment --features "gcp_run"    # Should compile only run (~15 endpoints)
cargo check -p foundation_deployment --features "gcp_compute" # Should compile only compute (~180 endpoints)

# Expected: 
# - gcp_run only: ~3K LOC, <2 sec compile
# - gcp_compute only: ~35K LOC, <10 sec compile
# - All gcp (no flags): nothing compiles (gcp default = all groups)
```

## Risks

| Risk | Mitigation |
|------|------------|
| Breaking existing workflows | Keep old `gen_resources` commands functional during transition |
| Complex refactoring | Implement incrementally - one provider at a time |
| Grouping algorithm edge cases | `--dry-run` mode allows preview before committing |
| Shared resource complexity | Start without shared extraction, add in Phase 3 |

---

_Version: 2.0 - Updated: 2026-04-18 - Unified per-endpoint generation model_
