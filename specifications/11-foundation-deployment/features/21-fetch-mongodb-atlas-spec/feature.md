---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/17-fetch-mongodb-atlas-spec"
this_file: "specifications/11-foundation-deployment/features/17-fetch-mongodb-atlas-spec/feature.md"

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

## Architecture

Provider implementations live in `backends/foundation_deployment/src/providers/`, **not** in `bin/platform/`. Each provider gets its own directory:

```
backends/foundation_deployment/src/providers/
├── mongodb_atlas/
│   ├── mod.rs       # Module declaration (pub mod fetch;)
│   ├── fetch.rs     # Fetch logic, constants, process_spec
│   └── resources/
│       └── mod.rs   # (future) Auto-generated resource types
├── openapi.rs       # Shared OpenAPI 3.x extraction utilities
└── standard/
    └── fetch.rs     # Generic HTTP fetch (used by all standard providers)
```

### MongoDB Atlas-Specific Fetcher

```rust
// backends/foundation_deployment/src/providers/mongodb_atlas/fetch.rs

pub const SPEC_URL: &str = "https://www.mongodb.com/docs/api/doc/atlas-admin-api-v2.json";
pub const PROVIDER_NAME: &str = "mongodb-atlas";

pub fn fetch_mongodb_atlas_specs(output_dir: PathBuf) -> Result<impl StreamIterator<...>, DeploymentError> {
    standard::fetch::fetch_standard_spec(PROVIDER_NAME, SPEC_URL, output_dir)
}

pub fn process_spec(spec: &serde_json::Value) -> ProcessedSpec {
    openapi::process_spec(spec)
}
```

## Tasks

1. **Create MongoDB Atlas provider module**
   - [x] Create `backends/foundation_deployment/src/providers/mongodb_atlas/mod.rs`
   - [x] Create `backends/foundation_deployment/src/providers/mongodb_atlas/fetch.rs`
   - [x] Implement fetch and process_spec functions

2. **Register in module tree**
   - [x] Add `pub mod mongodb_atlas;` to `providers/mod.rs`
   - [x] Wire into fetcher

3. **Write unit tests**
   - [x] Test constants, version extraction, endpoint extraction

4. **Integration test**
   - [x] Verify `cargo run -- gen_provider_specs --provider mongodb-atlas` works

## Success Criteria

- [x] All 4 tasks completed
- [x] Zero warnings
- [x] Spec fetches successfully to `artefacts/cloud_providers/mongodb-atlas/`

## Verification

```bash
cargo test -p foundation_deployment -- resources::mongodb_atlas
cargo run -- gen_provider_specs --provider mongodb-atlas
```

---

_Created: 2026-03-27_
_Updated: 2026-04-04 - Corrected directory structure to backends/foundation_deployment_
