---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/16-fetch-supabase-spec"
this_file: "specifications/11-foundation-deployment/features/16-fetch-supabase-spec/feature.md"

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


# Fetch Supabase OpenAPI Spec

## Iron Law: Zero Warnings

> **All code must compile with zero warnings and pass all lints. No suppression. No exceptions.**

## Overview

Implement the Supabase OpenAPI spec fetcher. Supabase provides their spec at `https://api.supabase.com/api/v1-json`.

## Supabase Spec Details

| Property | Value |
|----------|-------|
| URL | `https://api.supabase.com/api/v1-json` |
| Format | OpenAPI JSON |
| Auth Required | No |
| Rate Limits | Standard HTTP rate limits |

## Architecture

Provider implementations live in `backends/foundation_deployment/src/providers/`, **not** in `bin/platform/`. Each provider gets its own directory:

```
backends/foundation_deployment/src/providers/
├── supabase/
│   ├── mod.rs       # Module declaration (pub mod fetch;)
│   ├── fetch.rs     # Fetch logic, constants, process_spec
│   └── resources/
│       └── mod.rs   # (future) Auto-generated resource types
├── openapi.rs       # Shared OpenAPI 3.x extraction utilities
└── standard/
    └── fetch.rs     # Generic HTTP fetch (used by all standard providers)
```

### Supabase-Specific Fetcher

```rust
// backends/foundation_deployment/src/providers/supabase/fetch.rs

pub const SPEC_URL: &str = "https://api.supabase.com/api/v1-json";
pub const PROVIDER_NAME: &str = "supabase";

pub fn fetch_supabase_specs(output_dir: PathBuf) -> Result<impl StreamIterator<...>, DeploymentError> {
    standard::fetch::fetch_standard_spec(PROVIDER_NAME, SPEC_URL, output_dir)
}

pub fn process_spec(spec: &serde_json::Value) -> ProcessedSpec {
    openapi::process_spec(spec)
}
```

## Tasks

1. **Create Supabase provider module**
   - [x] Create `backends/foundation_deployment/src/providers/supabase/mod.rs`
   - [x] Create `backends/foundation_deployment/src/providers/supabase/fetch.rs`
   - [x] Implement fetch and process_spec functions

2. **Register in module tree**
   - [x] Add `pub mod supabase;` to `providers/mod.rs`
   - [x] Wire into fetcher

3. **Write unit tests**
   - [x] Test constants, version extraction, endpoint extraction

4. **Integration test**
   - [x] Verify `cargo run -- gen_provider_specs --provider supabase` works

## Success Criteria

- [x] All 4 tasks completed
- [x] Zero warnings
- [x] Spec fetches successfully to `artefacts/cloud_providers/supabase/`

## Verification

```bash
cargo test -p foundation_deployment -- resources::supabase
cargo run -- gen_provider_specs --provider supabase
```

---

_Created: 2026-03-27_
_Updated: 2026-04-04 - Corrected directory structure to backends/foundation_deployment_
