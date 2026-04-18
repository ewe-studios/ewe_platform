# Shared Resources - Start Here

## Goal

Detect resources (types, schemas) shared across multiple API groups and collocate them in a shared crate to avoid duplication and circular dependencies.

## Why

OpenAPI specs often have common types referenced by multiple API groups:
- `Location` - used by almost every GCP API
- `IAMPolicy` - used by resource management APIs
- `Operation` - used by async operation APIs

Without proper handling:
- Duplicate type definitions
- Type incompatibility errors
- Circular crate dependencies

## Detection Strategy

1. **Track usage** - For each resource, track which groups reference it
2. **Identify shared** - Resources used by 2+ groups are shared
3. **Extract** - Generate shared crate with common types
4. **Wire dependencies** - Group crates depend on shared crate

## First Steps

1. Implement resource usage tracking in analysis
2. Build resource → group dependency graph
3. Design shared crate structure

## Key Files

- `bin/platform/src/gen_api/analysis.rs` - Add resource usage tracking
- `bin/platform/src/gen_api/shared.rs` - New shared resource detection
- `bin/platform/src/gen_api/generate.rs` - Generate shared crate

## Expected Output

```
# Dry run shows shared resources
cargo run --bin ewe_platform gen_api gcp --dry

# Shared Resources: 45 types
# Collocated in: foundation_deployment_gcp_shared
# Size: ~12 KB
```
