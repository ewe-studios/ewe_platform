# Unify Generator - Start Here

## Goal

Combine three split generator commands into a single `gen_api` command that understands the full OpenAPI spec and generates correctly grouped, feature-guarded code.

## Why

The current split (`gen_resource_types`, `gen_provider_clients`, `gen_provider_wrappers`) generates code independently without understanding:
- Resource → Client → Provider relationships
- Shared resources across API groups
- Intelligent grouping boundaries

This led to 306 individual API modules with no grouping intelligence.

## First Steps

1. Read existing generators to understand current logic:
   - `bin/platform/src/gen_resources/types.rs`
   - `bin/platform/src/gen_resources/clients.rs`
   - `bin/platform/src/gen_resources/provider_wrappers.rs`

2. Design the unified `gen_api` command structure

3. Implement analysis phase first (can run with `--dry`)

## Key Files

- `bin/platform/src/commands.rs` - Add new `gen_api` subcommand
- `bin/platform/src/gen_api/mod.rs` - New unified generator module
- `bin/platform/src/gen_api/analysis.rs` - OpenAPI analysis
- `bin/platform/src/gen_api/grouping.rs` - Endpoint grouping
- `bin/platform/src/gen_api/generate.rs` - Code generation

## Expected Output

```bash
# Dry run shows what would be generated
cargo run --bin ewe_platform gen_api gcp --spec gcp.json --dry

# Groups: 25 (10-200 endpoints each)
# Shared resources: 45 types
# Total files: 312
# Estimated compile time: ~2 min (was 2+ hours)
```
