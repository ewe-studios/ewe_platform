---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/11-fetch-flyio-spec"
this_file: "specifications/11-foundation-deployment/features/11-fetch-flyio-spec/feature.md"

status: completed
priority: medium
created: 2026-03-27

depends_on: ["10-provider-spec-fetcher-core"]

tasks:
  completed: 4
  uncompleted: 0
  total: 4
  completion_percentage: 100%
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

## Architecture

Provider implementations live in `backends/foundation_deployment/src/providers/`, **not** in `bin/platform/`. Each provider gets its own directory:

```
backends/foundation_deployment/src/providers/
├── fly_io/
│   ├── mod.rs       # Module declaration (pub mod fetch;)
│   ├── fetch.rs     # Fetch logic, constants, process_spec
│   └── resources/
│       └── mod.rs   # (future) Auto-generated resource types
├── openapi.rs       # Shared OpenAPI 3.x extraction utilities
└── standard/
    └── fetch.rs     # Generic HTTP fetch (used by all standard providers)
```

The `bin/platform/src/gen_provider_specs/fetcher.rs` orchestrates fetching by importing from `foundation_deployment::providers::fly_io`.

## Requirements

### Fly.io-Specific Fetcher

```rust
// backends/foundation_deployment/src/providers/fly_io/fetch.rs

use crate::error::DeploymentError;
use crate::providers::openapi::{self, ProcessedSpec};
use crate::providers::standard;
use foundation_core::valtron::StreamIterator;
use std::path::PathBuf;

pub const SPEC_URL: &str = "https://docs.machines.dev/spec/openapi3.json";
pub const PROVIDER_NAME: &str = "fly-io";

/// Fetch the Fly.io OpenAPI spec.
/// Delegates to standard::fetch for the actual HTTP download.
pub fn fetch_fly_io_specs(
    output_dir: PathBuf,
) -> Result<
    impl StreamIterator<D = Result<PathBuf, DeploymentError>, P = ()> + Send + 'static,
    DeploymentError,
> {
    standard::fetch::fetch_standard_spec(PROVIDER_NAME, SPEC_URL, output_dir)
}

/// Process a fetched Fly.io spec into version, endpoints, and content hash.
pub fn process_spec(spec: &serde_json::Value) -> ProcessedSpec {
    openapi::process_spec(spec)
}
```

### Shared OpenAPI Extraction

Version and endpoint extraction is shared across all standard providers via `openapi.rs`:

```rust
// backends/foundation_deployment/src/providers/openapi.rs

pub fn extract_version(spec: &serde_json::Value) -> Option<String>;
pub fn extract_endpoints(spec: &serde_json::Value) -> Option<Vec<ApiEndpoint>>;
pub fn compute_content_hash(content: &str) -> String;
pub fn process_spec(spec: &serde_json::Value) -> ProcessedSpec;
```

## Error Handling

Errors use `DeploymentError` from `foundation_deployment::error`, not `SpecFetchError` from `bin/platform`. The fetcher in `bin/platform` converts `DeploymentError` to `SpecFetchError` at the orchestration layer.

## Tasks

1. **Create Fly.io provider module**
   - [x] Create `backends/foundation_deployment/src/providers/fly_io/mod.rs`
   - [x] Create `backends/foundation_deployment/src/providers/fly_io/fetch.rs`
   - [x] Implement `fetch_fly_io_specs()` delegating to `standard::fetch`
   - [x] Implement `process_spec()` delegating to `openapi::process_spec`

2. **Register in module tree**
   - [x] Add `pub mod fly_io;` to `providers/mod.rs`
   - [x] Wire into `fetcher.rs` in `bin/platform` via `configured_providers()` and `create_provider_stream()`

3. **Write unit tests**
   - [x] Test constants are correct
   - [x] Test version extraction with sample spec
   - [x] Test endpoint extraction
   - [x] Test minimal/empty spec handling

4. **Integration test**
   - [x] Verify `cargo run -- gen_provider_specs --provider fly-io` fetches spec
   - [x] Verify spec is written to `artefacts/cloud_providers/fly-io/openapi.json`
   - [x] Verify manifest is correct

## Success Criteria

- [x] All 4 tasks completed
- [x] `cargo clippy -p foundation_deployment` — zero warnings
- [x] `cargo test -p foundation_deployment -- resources::fly_io` — all pass
- [x] Fly.io spec fetches successfully
- [x] Endpoints are correctly extracted via shared `openapi.rs`
- [x] Version is extracted from spec metadata
- [x] Change detection works via content hash

## Verification

```bash
# Run tests
cargo test -p foundation_deployment -- resources::fly_io

# Fetch Fly.io spec
cd bin/platform
cargo run -- gen_provider_specs --provider fly-io

# Verify output
ls ../../artefacts/cloud_providers/fly-io/openapi.json
cat ../../artefacts/cloud_providers/fly-io/_manifest.json
```

---

_Created: 2026-03-27_
_Updated: 2026-04-04 - Corrected directory structure to backends/foundation_deployment_
