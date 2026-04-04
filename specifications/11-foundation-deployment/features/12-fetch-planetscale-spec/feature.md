---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/12-fetch-planetscale-spec"
this_file: "specifications/11-foundation-deployment/features/12-fetch-planetscale-spec/feature.md"

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

## Architecture

Provider implementations live in `backends/foundation_deployment/src/providers/`, **not** in `bin/platform/`. Each provider gets its own directory:

```
backends/foundation_deployment/src/providers/
├── planetscale/
│   ├── mod.rs       # Module declaration (pub mod fetch;)
│   ├── fetch.rs     # Fetch logic, constants, process_spec
│   └── resources/
│       └── mod.rs   # (future) Auto-generated resource types
├── openapi.rs       # Shared OpenAPI 3.x extraction utilities
└── standard/
    └── fetch.rs     # Generic HTTP fetch (used by all standard providers)
```

### PlanetScale-Specific Fetcher

```rust
// backends/foundation_deployment/src/providers/planetscale/fetch.rs

pub const SPEC_URL: &str = "https://api.planetscale.com/v1/openapi-spec";
pub const PROVIDER_NAME: &str = "planetscale";

pub fn fetch_planetscale_specs(output_dir: PathBuf) -> Result<impl StreamIterator<...>, DeploymentError> {
    standard::fetch::fetch_standard_spec(PROVIDER_NAME, SPEC_URL, output_dir)
}

pub fn process_spec(spec: &serde_json::Value) -> ProcessedSpec {
    openapi::process_spec(spec)
}
```

## Tasks

1. **Create PlanetScale provider module**
   - [x] Create `backends/foundation_deployment/src/providers/planetscale/mod.rs`
   - [x] Create `backends/foundation_deployment/src/providers/planetscale/fetch.rs`
   - [x] Implement fetch and process_spec functions

2. **Register in module tree**
   - [x] Add `pub mod planetscale;` to `providers/mod.rs`
   - [x] Wire into fetcher via `configured_providers()` and `create_provider_stream()`

3. **Write unit tests**
   - [x] Test constants, version extraction, endpoint extraction

4. **Integration test**
   - [x] Verify `cargo run -- gen_provider_specs --provider planetscale` works

## Success Criteria

- [x] All 4 tasks completed
- [x] Zero warnings
- [x] Spec fetches and is written correctly to `artefacts/cloud_providers/planetscale/`

## Verification

```bash
cargo test -p foundation_deployment -- resources::planetscale
cargo run -- gen_provider_specs --provider planetscale
```

---

_Created: 2026-03-27_
_Updated: 2026-04-04 - Corrected directory structure to backends/foundation_deployment_
