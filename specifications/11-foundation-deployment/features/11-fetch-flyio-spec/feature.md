---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/11-fetch-flyio-spec"
this_file: "specifications/11-foundation-deployment/features/11-fetch-flyio-spec/feature.md"

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


# Fetch Fly.io OpenAPI Spec

## Iron Law: Zero Warnings

> **All code must compile with zero warnings and pass all lints. No suppression. No exceptions.**

## Overview

Implement the Fly.io OpenAPI spec fetcher using the core infrastructure from Feature 10. Fly.io provides a single OpenAPI 3.0 JSON spec at `https://docs.machines.dev/spec/openapi3.json`.

This feature validates the core fetcher works correctly with a straightforward single-URL provider.

## Fly.io Spec Details

| Property | Value |
|----------|-------|
| URL | `https://docs.machines.dev/spec/openapi3.json` |
| Format | OpenAPI 3.0 JSON |
| Auth Required | No |
| Rate Limits | Standard HTTP rate limits |
| Spec Size | ~500KB |

## Requirements

### Fly.io-Specific Fetcher

```rust
// bin/platform/src/gen_provider_specs/providers/fly_io.rs

use crate::errors::SpecFetchError;  // Import from central errors.rs
use crate::core::{DistilledSpec, SpecEndpoint};  // Core types from core.rs
use foundation_core::wire::simple_http::client::SimpleHttpClient;

pub const FLY_IO_SPEC_URL: &str = "https://docs.machines.dev/spec/openapi3.json";

/// Fetch and distill the Fly.io OpenAPI spec.
pub async fn fetch_flyio_spec(
    client: &SimpleHttpClient,
) -> Result<DistilledSpec, SpecFetchError> {
    let response = client
        .get(FLY_IO_SPEC_URL)
        .send()
        .await
        .map_err(|e| SpecFetchError::Http {
            provider: "fly-io".to_string(),
            source: e,
        })?;

    let raw_spec: serde_json::Value = serde_json::from_str(&response.body)
        .map_err(|e| SpecFetchError::Json {
            provider: "fly-io".to_string(),
            source: e,
        })?;

    let content_hash = compute_sha256(&raw_spec.to_string());
    let version = extract_flyio_version(&raw_spec)
        .unwrap_or_else(|| chrono::Utc::now().format("%Y%m%d").to_string());
    let endpoints = extract_flyio_endpoints(&raw_spec);

    Ok(DistilledSpec {
        provider: "fly-io".to_string(),
        version,
        fetched_at: chrono::Utc::now(),
        source_url: FLY_IO_SPEC_URL.to_string(),
        raw_spec,
        endpoints,
        content_hash,
    })
}

fn extract_flyio_version(spec: &serde_json::Value) -> Option<String> {
    spec.get("info")
        .and_then(|info| info.get("version"))
        .and_then(|v| v.as_str())
        .map(String::from)
}

fn extract_flyio_endpoints(spec: &serde_json::Value) -> Option<Vec<SpecEndpoint>> {
    spec.get("paths")
        .and_then(|paths| paths.as_object())
        .map(|paths_obj| {
            paths_obj
                .iter()
                .flat_map(|(path, path_item)| {
                    let path_item = path_item.as_object()?;
                    let mut endpoints = Vec::new();

                    for method in ["get", "post", "put", "patch", "delete"] {
                        if let Some(operation) = path_item.get(method) {
                            endpoints.push(SpecEndpoint {
                                path: path.clone(),
                                methods: vec![method.to_uppercase()],
                                operation_id: operation
                                    .get("operationId")
                                    .and_then(|v| v.as_str())
                                    .map(String::from),
                                summary: operation
                                    .get("summary")
                                    .and_then(|v| v.as_str())
                                    .map(String::from),
                            });
                        }
                    }

                    endpoints
                })
                .collect()
        })
}

fn compute_sha256(content: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}
```

## Error Handling

**All errors are defined in `errors.rs` at the module root.** Provider-specific code:

1. Imports `SpecFetchError` from the central module: `use crate::errors::SpecFetchError;`
2. Constructs error variants manually (no `?` auto-conversion for custom fields):
   ```rust
   .map_err(|e| SpecFetchError::Http {
       provider: "fly-io".to_string(),
       source: e,
   })
   ```
3. Returns `Result<DistilledSpec, SpecFetchError>` from all fallible functions

## Tasks

1. **Create Fly.io provider module**
   - [ ] Create `bin/platform/src/gen_provider_specs/providers/fly_io.rs`
   - [ ] Implement `fetch_flyio_spec()`
   - [ ] Implement `extract_flyio_version()`
   - [ ] Implement `extract_flyio_endpoints()`

2. **Register in core fetcher**
   - [ ] Add Fly.io to `configured_providers()` list
   - [ ] Wire up `fetch_single_spec()` for Fly.io
   - [ ] Ensure progress reporting works

3. **Write unit tests**
   - [ ] Test version extraction with sample spec
   - [ ] Test endpoint extraction
   - [ ] Test error handling for invalid responses

4. **Integration test**
   - [ ] Run `cargo run -- gen_provider_specs --provider fly-io`
   - [ ] Verify spec is written to `distilled-spec-fly-io/specs/`
   - [ ] Verify manifest is correct

## Success Criteria

- [ ] All 4 tasks completed
- [ ] `cargo clippy` — zero warnings
- [ ] Fly.io spec fetches successfully
- [ ] Endpoints are correctly extracted
- [ ] Version is extracted from spec metadata
- [ ] Change detection works on re-fetch

## Verification

```bash
cd bin/platform

# Fetch Fly.io spec
cargo run -- gen_provider_specs --provider fly-io

# Verify output
ls ../../@formulas/src.rust/src.deployAnywhere/distilled-spec-fly-io/specs/
cat ../../@formulas/src.rust/src.deployAnywhere/distilled-spec-fly-io/specs/_manifest.json
```

---

_Created: 2026-03-27_
