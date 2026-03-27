---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/12-fetch-planetscale-spec"
this_file: "specifications/11-foundation-deployment/features/12-fetch-planetscale-spec/feature.md"

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


# Fetch PlanetScale OpenAPI Spec

## Iron Law: Zero Warnings

> **All code must compile with zero warnings and pass all lints. No suppression. No exceptions.**

## Overview

Implement the PlanetScale OpenAPI spec fetcher. PlanetScale provides their spec at `https://api.planetscale.com/v1/openapi-spec`.

## PlanetScale Spec Details

| Property | Value |
|----------|-------|
| URL | `https://api.planetscale.com/v1/openapi-spec` |
| Format | OpenAPI JSON |
| Auth Required | No |
| Rate Limits | Standard HTTP rate limits |

## Requirements

### PlanetScale-Specific Fetcher

```rust
// bin/platform/src/gen_provider_specs/providers/planetscale.rs

use crate::errors::SpecFetchError;  // Import from central errors.rs
use crate::core::{DistilledSpec, SpecEndpoint};  // Core types from core.rs
use foundation_core::wire::simple_http::client::SimpleHttpClient;

pub const PLANETSCALE_SPEC_URL: &str = "https://api.planetscale.com/v1/openapi-spec";

/// Fetch and distill the PlanetScale OpenAPI spec.
pub async fn fetch_planetscale_spec(
    client: &SimpleHttpClient,
) -> Result<DistilledSpec, SpecFetchError> {
    // Standard fetch pattern (same as Fly.io)
    // PlanetScale uses standard OpenAPI format
}

fn extract_planetscale_version(spec: &serde_json::Value) -> Option<String> {
    spec.get("info")
        .and_then(|info| info.get("version"))
        .and_then(|v| v.as_str())
        .map(String::from)
}

fn extract_planetscale_endpoints(spec: &serde_json::Value) -> Option<Vec<SpecEndpoint>> {
    // Extract from paths object
}
```

## Error Handling

**All errors are defined in `errors.rs` at the module root.** Provider-specific code imports:
```rust
use crate::errors::SpecFetchError;
```

## Tasks

1. **Create PlanetScale provider module**
   - [ ] Create `bin/platform/src/gen_provider_specs/providers/planetscale.rs`
   - [ ] Implement `fetch_planetscale_spec()`
   - [ ] Implement version and endpoint extraction

2. **Register in core fetcher**
   - [ ] Add PlanetScale to provider list
   - [ ] Wire up fetch function

3. **Write unit tests**
   - [ ] Test with sample PlanetScale spec structure

4. **Integration test**
   - [ ] Run fetch and verify output

## Success Criteria

- [ ] All 4 tasks completed
- [ ] Zero warnings
- [ ] Spec fetches and is written correctly

## Verification

```bash
cargo run -- gen_provider_specs --provider planetscale
```

---

_Created: 2026-03-27_
