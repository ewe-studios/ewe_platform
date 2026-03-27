---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/18-fetch-neon-spec"
this_file: "specifications/11-foundation-deployment/features/18-fetch-neon-spec/feature.md"

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


# Fetch Neon OpenAPI Spec

## Iron Law: Zero Warnings

> **All code must compile with zero warnings and pass all lints. No suppression. No exceptions.**

## Overview

Implement the Neon OpenAPI spec fetcher. Neon provides their v2 API spec at `https://neon.com/api_spec/release/v2.json`.

This is a well-structured single-file OpenAPI spec, similar to Fly.io and other straightforward providers.

## Neon Spec Details

| Property | Value |
|----------|-------|
| URL | `https://neon.com/api_spec/release/v2.json` |
| Format | OpenAPI JSON |
| Auth Required | No |
| Rate Limits | Standard HTTP rate limits |
| Existing Repo | `distilled-spec-neon` |

## Requirements

### Neon-Specific Fetcher

```rust
// bin/platform/src/gen_provider_specs/providers/neon.rs

use crate::errors::SpecFetchError;  // Import from central errors.rs
use crate::core::{DistilledSpec, SpecEndpoint};  // Core types from core.rs
use foundation_core::wire::simple_http::client::SimpleHttpClient;

pub const NEON_SPEC_URL: &str = "https://neon.com/api_spec/release/v2.json";

/// Fetch and distill the Neon OpenAPI spec.
pub async fn fetch_neon_spec(
    client: &SimpleHttpClient,
) -> Result<DistilledSpec, SpecFetchError> {
    // Standard fetch pattern
}

fn extract_neon_version(spec: &serde_json::Value) -> Option<String> {
    spec.get("info")
        .and_then(|info| info.get("version"))
        .and_then(|v| v.as_str())
        .map(String::from)
}

fn extract_neon_endpoints(spec: &serde_json::Value) -> Option<Vec<SpecEndpoint>> {
    // Extract from paths object
    // Neon has well-organized endpoints for:
    // - Projects (CRUD)
    // - Branches
    // - Endpoints
    // - Databases
    // - Roles
    // - Operations
}
```

## Error Handling

**All errors are defined in `errors.rs` at the module root.** Import and use:
```rust
use crate::errors::SpecFetchError;
```

## Tasks

1. **Create Neon provider module**
   - [ ] Create `bin/platform/src/gen_provider_specs/providers/neon.rs`
   - [ ] Implement fetch function
   - [ ] Implement version and endpoint extraction

2. **Register in core fetcher**
   - [ ] Add to provider list

3. **Write unit tests**
   - [ ] Test with sample spec structure
   - [ ] Test endpoint extraction (Neon has good structure for testing)

4. **Integration test**
   - [ ] Run fetch and verify output
   - [ ] Compare with existing `distilled-spec-neon` repo

## Success Criteria

- [ ] All 4 tasks completed
- [ ] Zero warnings
- [ ] Spec fetches successfully
- [ ] Endpoints correctly extracted

## Verification

```bash
cargo run -- gen_provider_specs --provider neon

# Compare with existing
cat ../../@formulas/src.rust/src.deployAnywhere/distilled-spec-neon/specs/openapi.json
```

---

_Created: 2026-03-27_
