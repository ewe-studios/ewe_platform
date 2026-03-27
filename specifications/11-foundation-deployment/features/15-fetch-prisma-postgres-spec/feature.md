---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/15-fetch-prisma-postgres-spec"
this_file: "specifications/11-foundation-deployment/features/15-fetch-prisma-postgres-spec/feature.md"

status: pending
priority: medium
created: 2026-03-27

depends_on: ["10-provider-spec-fetcher-core"]

tasks:
  completed: 0
  uncompleted: 4
  total: 4
  completion_percentage: 0%
---


# Fetch Prisma Postgres OpenAPI Spec

## Iron Law: Zero Warnings

> **All code must compile with zero warnings and pass all lints. No suppression. No exceptions.**

## Overview

Implement the Prisma Postgres OpenAPI spec fetcher. Prisma Postgres provides a single OpenAPI spec at `https://api.prisma.io/v1/doc`.

## Prisma Postgres Spec Details

| Property | Value |
|----------|-------|
| URL | `https://api.prisma.io/v1/doc` |
| Format | OpenAPI JSON |
| Auth Required | No |
| Rate Limits | Standard HTTP rate limits |

## Requirements

### Prisma Postgres-Specific Fetcher

```rust
// bin/platform/src/gen_provider_specs/providers/prisma_postgres.rs

use crate::errors::SpecFetchError;  // Import from central errors.rs
use crate::core::{DistilledSpec, SpecEndpoint};  // Core types from core.rs
use foundation_core::wire::simple_http::client::SimpleHttpClient;

pub const PRISMA_POSTGRES_SPEC_URL: &str = "https://api.prisma.io/v1/doc";

/// Fetch and distill the Prisma Postgres OpenAPI spec.
pub async fn fetch_prisma_postgres_spec(
    client: &SimpleHttpClient,
) -> Result<DistilledSpec, SpecFetchError> {
    // Standard fetch pattern
}

fn extract_prisma_postgres_version(spec: &serde_json::Value) -> Option<String> {
    spec.get("info")
        .and_then(|info| info.get("version"))
        .and_then(|v| v.as_str())
        .map(String::from)
}

fn extract_prisma_postgres_endpoints(spec: &serde_json::Value) -> Option<Vec<SpecEndpoint>> {
    // Extract from paths object
}
```

## Error Handling

**All errors are defined in `errors.rs` at the module root.** Import and use:
```rust
use crate::errors::SpecFetchError;
```

## Tasks

1. **Create Prisma Postgres provider module**
   - [ ] Create `bin/platform/src/gen_provider_specs/providers/prisma_postgres.rs`
   - [ ] Implement fetch function
   - [ ] Implement version and endpoint extraction

2. **Register in core fetcher**
   - [ ] Add to provider list

3. **Write unit tests**
   - [ ] Test with sample spec structure

4. **Integration test**
   - [ ] Run fetch and verify output

## Success Criteria

- [ ] All 4 tasks completed
- [ ] Zero warnings
- [ ] Spec fetches successfully

## Verification

```bash
cargo run -- gen_provider_specs --provider prisma-postgres
```

---

_Created: 2026-03-27_
