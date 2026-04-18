# Endpoint Grouping - Start Here

## Goal

Create an intelligent endpoint grouping algorithm that automatically organizes OpenAPI endpoints into optimally-sized API modules.

## Why

The current approach generates one module per API (306 for GCP), causing:
- 512K lines compiled monolithically
- Hours of build time
- Unmanageable feature flag complexity

## Grouping Strategy

### Natural Groupings
First, detect existing API boundaries from the OpenAPI spec:
- Path prefixes (`/v2/projects/.../run/*` → `gcp_run`)
- Service names (`run.googleapis.com` → `gcp_run`)
- Tag groups (OpenAPI `tags` field)

### Constraint Enforcement
Then enforce size constraints:
- **Split** groups > 200 endpoints
- **Merge** groups < 10 endpoints
- **Validate** all groups are 10-200 endpoints

## First Steps

1. Read existing OpenAPI analysis code
2. Implement path prefix detection
3. Build grouping algorithm
4. Add `--dry` mode for preview

## Key Files

- `bin/platform/src/gen_api/grouping.rs` - New grouping logic
- `bin/platform/src/gen_api/analysis.rs` - OpenAPI analysis
- `bin/platform/src/commands.rs` - CLI options

## Expected Output

```
# Dry run preview
cargo run --bin ewe_platform gen_api gcp --dry

# Groups: 28 (was 306)
# All groups within 10-200 endpoints ✓
# Shared resources: 45 types
```
