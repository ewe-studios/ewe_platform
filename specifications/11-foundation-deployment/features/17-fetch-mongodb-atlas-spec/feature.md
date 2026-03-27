---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/17-fetch-mongodb-atlas-spec"
this_file: "specifications/11-foundation-deployment/features/17-fetch-mongodb-atlas-spec/feature.md"

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


# Fetch MongoDB Atlas OpenAPI Spec

## Iron Law: Zero Warnings

> **All code must compile with zero warnings and pass all lints. No suppression. No exceptions.**

## Overview

Implement the MongoDB Atlas OpenAPI spec fetcher. MongoDB Atlas provides their Admin API v2 spec at `https://www.mongodb.com/docs/api/doc/atlas-admin-api-v2.json`.

## MongoDB Atlas Spec Details

| Property | Value |
|----------|-------|
| URL | `https://www.mongodb.com/docs/api/doc/atlas-admin-api-v2.json` |
| Format | OpenAPI JSON |
| Auth Required | No |
| Rate Limits | Standard HTTP rate limits |

## Requirements

### MongoDB Atlas-Specific Fetcher

```rust
// bin/platform/src/gen_provider_specs/providers/mongodb_atlas.rs

use crate::errors::SpecFetchError;  // Import from central errors.rs
use crate::core::{DistilledSpec, SpecEndpoint};  // Core types from core.rs
use foundation_core::wire::simple_http::client::SimpleHttpClient;

pub const MONGODB_ATLAS_SPEC_URL: &str = "https://www.mongodb.com/docs/api/doc/atlas-admin-api-v2.json";

/// Fetch and distill the MongoDB Atlas OpenAPI spec.
pub async fn fetch_mongodb_atlas_spec(
    client: &SimpleHttpClient,
) -> Result<DistilledSpec, SpecFetchError> {
    // Standard fetch pattern
}

fn extract_mongodb_atlas_version(spec: &serde_json::Value) -> Option<String> {
    spec.get("info")
        .and_then(|info| info.get("version"))
        .and_then(|v| v.as_str())
        .map(String::from)
}

fn extract_mongodb_atlas_endpoints(spec: &serde_json::Value) -> Option<Vec<SpecEndpoint>> {
    // MongoDB Atlas has many resource paths - extract from paths object
}
```

## Error Handling

**All errors are defined in `errors.rs` at the module root.** Import and use:
```rust
use crate::errors::SpecFetchError;
```

## Tasks

1. **Create MongoDB Atlas provider module**
   - [ ] Create `bin/platform/src/gen_provider_specs/providers/mongodb_atlas.rs`
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
cargo run -- gen_provider_specs --provider mongodb-atlas
```

---

_Created: 2026-03-27_
