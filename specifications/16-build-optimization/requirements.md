---
description: "Reduce ewe_deployables build times by 95%+ through unified API generator with intelligent endpoint grouping and feature flag scoping"
status: "pending"
priority: "high"
created: 2026-04-18
updated: 2026-04-18
author: "Main Agent"
metadata:
  version: "2.0"
  estimated_effort: "high"
  tags:
    - build-optimization
    - rust
    - compilation
    - cranelift
    - feature-flags
    - code-generation
  skills: []
  tools:
    - Rust
    - cargo
    - cargo clippy
has_features: true
has_fundamentals: false
builds_on: "specifications/11-foundation-deployment"
related_specs:
  - "specifications/11-foundation-deployment/features/26-gen-provider-clients"
  - "specifications/11-foundation-deployment/features/24-gen-resource-types"
features:
  completed: 0
  uncompleted: 4
  total: 4
  completion_percentage: 0%
---

# Build Optimization Specification

## Overview

This specification addresses the **extremely slow build times** (approaching hours) for the `ewe_deployables` crate and its dependency `foundation_deployment`. 

### Root Cause

The current generator architecture is **split across three separate commands**:
- `gen_resource_types` - Generates resource types
- `gen_provider_clients` - Generates API clients  
- `gen_provider_wrappers` - Generates provider wrappers

This split has created an **incomplete picture** of OpenAPI endpoint relationships, leading to:
- 306 individual GCP API modules with no intelligent grouping
- 512,000 lines of code in a single monolithic crate
- Resources, clients, and providers generated independently without understanding their dependencies
- Feature flags that exist but cannot be effectively wired due to lack of grouping intelligence

### The Solution: Unified `gen_api` Command

Replace the three split generators with a **single unified `api` subcommand** that:

1. **Analyzes** the full OpenAPI specification
2. **Groups** related endpoints intelligently (not too small <10, not too large >200)
3. **Maps** resource → client → provider relationships
4. **Generates** feature-guarded crates with correct scoping
5. **Handles** shared resources via collocated shared crates

## Expected Impact

| Metric | Before | After |
|--------|--------|-------|
| Clean build (dev) | 2+ hours | 2-5 minutes |
| Incremental build | 5 minutes | 15-30 seconds |
| `ewe_deployables` check | 30 minutes | 1 minute |
| **Build time reduction** | - | **95%+** |

---

## Feature Index

### Pending Features (0/4 completed)

1. **[unify-generator](./features/01-unify-generator/feature.md)** - Combine gen_resource_types, gen_provider_clients, gen_provider_wrappers into single `gen_api` command
2. **[endpoint-grouping](./features/02-endpoint-grouping/feature.md)** - Intelligent OpenAPI endpoint analysis and grouping (10-200 endpoints per group)
3. **[shared-resources](./features/03-shared-resources/feature.md)** - Handle resources shared across multiple clients/providers via collocated shared crates
4. **[cranelift-incremental](./features/04-cranelift-incremental/feature.md)** - Enable Cranelift backend and incremental compilation

---

## Tasks

### Phase 1: Unified Generator Architecture

- [ ] Read existing generator sources: `bin/platform/src/gen_resources/{types,clients,provider_wrappers}.rs`
- [ ] Design unified `gen_api` command structure
- [ ] Implement OpenAPI spec analysis and normalization
- [ ] Implement endpoint grouping algorithm
- [ ] Implement resource → client → provider relationship mapping
- [ ] Consolidate generation logic under single command

### Phase 2: Intelligent Endpoint Grouping

- [ ] Analyze GCP OpenAPI spec for natural groupings (run, jobs, compute, etc.)
- [ ] Implement automatic grouping for specs without clear boundaries
- [ ] Enforce group size constraints (10-200 endpoints per group)
- [ ] Generate feature flags per group
- [ ] Wire feature flags to generated module compilation

### Phase 3: Shared Resource Handling

- [ ] Detect resources shared across multiple groups
- [ ] Create shared crate structure for collocated resources
- [ ] Update dependency graph to reference shared crates
- [ ] Handle circular dependency edge cases
- [ ] Verify shared resource crate sizes are manageable

### Phase 4: Enable Cranelift + Incremental

- [ ] Set `incremental = true` in workspace `Cargo.toml`
- [ ] Enable `codegen-backend = "cranelift"` in `.cargo/config.toml`
- [ ] Verify Cranelift is active in build output
- [ ] Benchmark clean build time improvement

---

## Success Criteria

- [ ] `cargo run --bin ewe_platform gen_api --dry` shows grouping analysis without writing files
- [ ] `cargo check -p ewe_deployables --features "cloudflare,gcp_run"` completes in < 2 minutes
- [ ] Group sizes are within 10-200 endpoints range
- [ ] Shared resources are correctly collocated
- [ ] Incremental builds complete in < 30 seconds
- [ ] All existing tests pass

---

## Agent Rules Reference

### Mandatory Rules for All Agents

Load these rules from `.agents/rules/`:

| Rule | File | Purpose |
|------|------|---------|
| 01 | `.agents/rules/01-rule-naming-and-structure.md` | File naming conventions |
| 02 | `.agents/rules/02-rules-directory-policy.md` | Directory policies |
| 03 | `.agents/rules/03-dangerous-operations-safety.md` | Dangerous operations safety |
| 04 | `.agents/rules/04-work-commit-and-push-rules.md` | Work commit and push rules |

### Role-Specific Rules

| Agent Type | Additional Rules to Load |
|------------|--------------------------|
| **Review Agent** | `.agents/rules/06-specifications-and-requirements.md` |
| **Implementation Agent** | `.agents/rules/13-implementation-agent-guide.md`, stack file |
| **Verification Agent** | `.agents/rules/08-verification-workflow-complete-guide.md`, stack file |
| **Documentation Agent** | `.agents/rules/06-specifications-and-requirements.md` |

### Stack Files

Load from `.agents/stacks/`:
- **Language**: Rust → `.agents/stacks/rust.md`

---

## File Organization Reminder

ONLY these files allowed:
1. requirements.md - Requirements with tasks
2. LEARNINGS.md - All learnings
3. REPORT.md - All reports
4. VERIFICATION.md - Verification
5. PROGRESS.md - Current status (delete at 100%)
6. fundamentals/, features/, templates/ (optional)

FORBIDDEN: Separate learning/report/verification files

Consolidation: All learnings → LEARNINGS.md, All reports → REPORT.md

See Rule 06 "File Organization" for complete policy.
