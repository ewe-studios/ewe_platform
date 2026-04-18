---
name: "shared-resources"
description: "Handle resources shared across multiple clients/providers via collocated shared crates"
status: "pending"
priority: "high"
created: "2026-04-18"
author: "Main Agent"
metadata:
  version: "1.0"
  estimated_effort: "medium"
  tags:
    - shared-resources
    - crate-structure
    - codegen
    - build-optimization
dependencies: ["01-unify-generator", "02-endpoint-grouping"]
features: []
---

# Shared Resources

## Overview

Detect and handle resources (types, schemas) that are shared across multiple API groups by collocating them in a shared crate.

## Problem

In OpenAPI specs, certain resources are referenced by multiple API groups:

```
gcp_run     → uses → CommonPolicy, IAMPolicy, Location
gcp_compute → uses → CommonPolicy, IAMPolicy, Location, Disk, Zone
gcp_jobs    → uses → CommonPolicy, IAMPolicy, Location
```

If we generate each group independently:
- Duplicate type definitions (code bloat)
- Type incompatibility (same name, different definition)
- Circular dependencies between crates

## Solution

Extract shared resources into a collocated shared crate:

```
foundation_deployment_gcp/
├── shared/           ← Common types used by multiple groups
│   ├── policy.rs     ← CommonPolicy, IAMPolicy
│   ├── location.rs   ← Location
│   └── mod.rs
├── run/              ← gcp_run specific types
│   ├── instance.rs
│   └── mod.rs
├── compute/          ← gcp_compute specific types
│   ├── disk.rs
│   ├── zone.rs
│   └── mod.rs
└── jobs/             ← gcp_jobs specific types
    ├── job.rs
    └── mod.rs
```

## Detection Algorithm

### Phase 1: Build Resource Usage Map

```rust
struct ResourceUsage {
    resource_name: String,
    used_by_groups: Vec<String>,
    definition: TypeDefinition,
}

# For each resource in OpenAPI spec:
#   - Track which API groups reference it
#   - Count total references
```

### Phase 2: Identify Shared Resources

```
Shared if:
  - Referenced by >= 2 different groups
  - OR referenced by >= 10 endpoints across groups
```

### Phase 3: Validate Shared Crate Size

```
If shared crate > 500 types:
  - Split by domain (common, iam, location, network)
  - Create nested shared crates
```

### Phase 4: Update Dependencies

```toml
# Each group crate depends on shared
[dependencies]
foundation_deployment_gcp_shared = { path = "../shared" }
```

## Crate Structure

### Multi-Provider Layout

```
backends/
└── foundation_deployment_gcp/
    ├── Cargo.toml              # Workspace member
    └── src/
        ├── shared/             # Shared resources
        │   ├── Cargo.toml      # Nested crate
        │   └── src/
        │       ├── policy.rs
        │       ├── location.rs
        │       └── lib.rs
        ├── run/                # Run API group
        │   ├── Cargo.toml      # Nested crate
        │   └── src/
        │       ├── instance.rs
        │       └── lib.rs
        ├── compute/            # Compute API group
        │   ├── Cargo.toml      # Nested crate
        │   └── src/
        └── lib.rs              # Re-exports
```

### Feature Flag Wiring

```toml
# foundation_deployment_gcp/Cargo.toml
[features]
run = ["dep:run"]
compute = ["dep:compute"]

[dependencies]
shared = { path = "./shared" }
run = { path = "./run", optional = true }
compute = { path = "./compute", optional = true }
```

```rust
// lib.rs
#[cfg(feature = "run")]
pub use run::*;

#[cfg(feature = "compute")]
pub use compute::*;

// Shared always available
pub use shared::*;
```

## Tasks

- [ ] Implement resource usage tracking in analysis phase
- [ ] Build resource → group dependency graph
- [ ] Detect shared resources (used by 2+ groups)
- [ ] Generate shared crate structure
- [ ] Update group crate dependencies
- [ ] Wire shared types in root lib.rs
- [ ] Handle nested shared crates for large shared sets
- [ ] Test with GCP spec (expected: ~45 shared types)
- [ ] Verify circular dependency handling

## Verification

```bash
# Dry run should show shared resources
cargo run --bin ewe_platform gen_api gcp --dry

# Expected output:
# Shared Resources: 45 types
#   → Collocated in foundation_deployment_gcp_shared
#   → Estimated size: 12 KB

# Full generation
cargo run --bin ewe_platform gen_api gcp --features

# Verify shared crate compiles independently
cargo check -p foundation_deployment_gcp_shared

# Verify group crates compile with shared dependency
cargo check -p foundation_deployment_gcp --features "run"
```

## Edge Cases

| Case | Handling |
|------|----------|
| Resource used by all groups | Always include in shared (common types) |
| Resource used by 2 groups only | Still shared - avoid duplication |
| Circular resource references | Detect cycle, extract common base to shared |
| Same name, different schema | Namespace by group (gcp_run::Policy vs gcp_compute::Policy) |
| Shared crate too large (>500 types) | Split into domain-specific shared crates |

---

_Version: 1.0 - Created: 2026-04-18_
