---
name: "endpoint-grouping"
description: "Intelligent OpenAPI endpoint analysis and grouping (10-200 endpoints per group) for optimal compilation"
status: "pending"
priority: "critical"
created: "2026-04-18"
author: "Main Agent"
metadata:
  version: "1.0"
  estimated_effort: "high"
  tags:
    - grouping
    - analysis
    - codegen
    - build-optimization
dependencies: ["01-unify-generator"]
features: []
---

# Endpoint Grouping

## Overview

Intelligently group OpenAPI endpoints into optimally-sized API modules that compile efficiently.

**Problem:** GCP has 306 individual API modules, causing:

- 512,000 lines compiled monolithically
- Feature flag explosion (306 individual flags)
- Poor IDE performance
- Unmanageable crate structure

**Solution:** Automatic endpoint grouping with size constraints:

- **Minimum:** 10 endpoints per group (avoid fragmentation)
- **Maximum:** 200 endpoints per group (avoid monolithic modules)
- **Natural boundaries:** Respect existing API boundaries (run, compute, jobs, etc.)

## Grouping Algorithm

### Phase 1: Detect Natural Groupings

Analyze OpenAPI spec for explicit groupings:

```
GCP OpenAPI paths:
/v2/projects/{project}/locations/{location}/instances/*  → gcp_run
/v2/projects/{project}/locations/{location}/jobs/*       → gcp_jobs
/v1/projects/{project}/zones/{zone}/instances/*          → gcp_compute
```

Group by path prefix patterns:

- `run.googleapis.com/*` → Cloud Run API
- `compute.googleapis.com/*` → Compute Engine API
- `cloudkms.googleapis.com/*` → Cloud KMS API

### Phase 2: Split Large Groups

If a natural group exceeds `max_group_size` (100):

```
# Example: GCP Admin API has 500+ endpoints
# Split by resource type:
gcp_admin_settings     → Settings endpoints (~150)
gcp_admin_audit        → Audit log endpoints (~120)
gcp_admin_orgpolicy    → Org policy endpoints (~100)
gcp_admin_access       → Access control endpoints (~130)
```

### Phase 3: Merge Small Groups

If groups are below `min_group_size` (10):

```
# Merge related small APIs
gcp_oslogin + gcp_osconfig + gcp_ostest → gcp_os_family (~45 endpoints)
```

### Phase 4: Validate Constraints

```
For each group:
  - endpoints >= min_group_size (10)
  - endpoints <= max_group_size (100)
  - resources < 500 (type definitions)
  - clients < 100 (API client functions)
```

## Dry Mode

The `--dry` flag shows grouping analysis without writing files:

```bash
cargo run --bin ewe_platform gen_api gcp --dry

# Output:
# Analyzing GCP OpenAPI spec...
# Found: 1,247 endpoints across 306 APIs
#
# Proposed Grouping:
# ┌────────────────────────────┬──────────┬────────────┬───────────┐
# │ Group                      │Endpoints │ Resources  │ Est. Size │
# ├────────────────────────────┼──────────┼────────────┼───────────┤
# │ gcp_run                    │       45 │         12 │    4.2 KB │
# │ gcp_jobs                   │       38 │          9 │    3.8 KB │
# │ gcp_compute                │      189 │         67 │   18.5 KB │
# │ gcp_aiplatform             │      195 │         82 │   22.1 KB │
# │ ...                        │      ... │        ... │       ... │
# └────────────────────────────┴──────────┴────────────┴───────────┘
#
# Shared Resources: 45 types used across multiple groups
#   → Will be collocated in foundation_deployment_gcp_shared
#
# Total Groups: 28 (was 306)
# Avg Group Size: 44 endpoints
# Estimated compile time: ~2 min (was 2+ hours)
```

## Tasks

- [ ] Read existing OpenAPI normalization in `gen_resources/mod.rs`
- [ ] Implement path prefix detection for natural groupings
- [ ] Implement group splitting for large APIs
- [ ] Implement group merging for small APIs
- [ ] Add size constraint validation
- [ ] Implement `--dry` mode output formatting
- [ ] Add `--min-group-size` and `--max-group-size` CLI options
- [ ] Test with GCP OpenAPI spec (306 APIs)
- [ ] Test with Stripe OpenAPI spec
- [ ] Test with Shopify OpenAPI spec

## Verification

```bash
# Dry run should complete in < 5 seconds
time cargo run --bin ewe_platform gen_api gcp --spec gcp.json --dry

# Should show reasonable grouping (20-40 groups for GCP)
# All groups should be within 10-200 endpoints

# Full generation should produce compilable code
cargo run --bin ewe_platform gen_api gcp --spec gcp.json --features
cargo check -p foundation_deployment --features "gcp,gcp_run"
```

## Edge Cases

| Case                                       | Handling                                                        |
| ------------------------------------------ | --------------------------------------------------------------- |
| Single massive API (5000+ endpoints)       | Split by resource type, then by path depth                      |
| Many tiny APIs (< 5 endpoints each)        | Merge by path prefix or service name                            |
| Circular resource dependencies             | Detect and extract to shared crate                              |
| Resources with same name, different schema | Namespace by group (gcp_run::Instance vs gcp_compute::Instance) |

---

_Version: 1.0 - Created: 2026-04-18_
