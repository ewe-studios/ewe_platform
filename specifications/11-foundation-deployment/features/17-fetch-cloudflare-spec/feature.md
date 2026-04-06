---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/13-fetch-cloudflare-spec"
this_file: "specifications/11-foundation-deployment/features/13-fetch-cloudflare-spec/feature.md"

status: completed
priority: high
created: 2026-03-27
updated: 2026-04-05

depends_on: ["10-provider-spec-fetcher-core"]

tasks:
  completed: 5
  uncompleted: 2
  total: 7
  completion_percentage: 71%
---


# Fetch Cloudflare OpenAPI Spec + Resource Generation

## Iron Law: Zero Warnings

> **All code must compile with zero warnings and pass all lints. No suppression. No exceptions.**

## Overview

Implement the Cloudflare OpenAPI spec fetcher with **resource type generation**. This feature:

1. **Fetches the Cloudflare API schemas** from the GitHub repository
2. **Stores raw JSON specs** in `artefacts/cloud_providers/cloudflare/`
3. **Generates Rust resource types** from the OpenAPI schemas into `resources/`
4. **Generates operation traits** for each resource (CRUD operations)

The generated resources are used by the Cloudflare provider (Feature 04) for type-safe API interactions.

## Architecture

All provider implementations live in `backends/foundation_deployment/src/providers/{provider}/`:

```
backends/foundation_deployment/src/providers/cloudflare/
├── mod.rs             # Module declaration (pub mod fetch; pub mod provider; pub mod resources;)
├── provider.rs        # DeploymentProvider trait implementation
├── fetch.rs           # Git-clone based spec fetcher
└── resources/
    └── mod.rs         # Auto-generated resource types (DO NOT EDIT)
```

Fetched raw specs are stored separately in:
```
artefacts/cloud_providers/cloudflare/
├── openapi.json       # Consolidated spec
└── _manifest.json     # Fetch metadata
```

## Cloudflare Spec Details

| Property | Value |
|----------|-------|
| Source URL | `https://github.com/cloudflare/api-schemas` |
| Format | GitHub Repository (OpenAPI JSON files) |
| Auth Required | No (public repo) |
| Special Handling | Git clone required |

## Requirements

### Spec Fetching

```rust
// backends/foundation_deployment/src/providers/cloudflare/fetch.rs

use crate::error::DeploymentError;
use crate::providers::standard;
use foundation_core::valtron::StreamIterator;
use std::path::PathBuf;

pub const CLOUDFLARE_API_SCHEMAS_URL: &str = "https://github.com/cloudflare/api-schemas";

/// Fetch Cloudflare specs by cloning the GitHub repo.
/// Returns a StreamIterator that yields the result when complete.
pub fn fetch_cloudflare_specs(
    temp_dir: PathBuf,
    output_dir: PathBuf,
) -> Result<impl StreamIterator<D = Result<PathBuf, DeploymentError>, P = ()> + Send + 'static, DeploymentError> {
    // 1. Git clone --depth 1 the api-schemas repo
    // 2. Find relevant API spec JSON files
    // 3. Consolidate into openapi.json + _manifest.json
    // 4. Write to output_dir
}
```

### Resource Type Generation

Resource types are generated into `backends/foundation_deployment/src/providers/cloudflare/resources/mod.rs` by the `gen_resource_types` command. This is separate from spec fetching.

## Error Handling

Errors use `DeploymentError` from `foundation_deployment::error`. The fetcher in `bin/platform` converts to `SpecFetchError` at the orchestration layer.

## Tasks

1. **Cloudflare spec fetcher** (completed)
   - [x] Implement git clone approach in `providers/cloudflare/fetch.rs`
   - [x] Implement OpenAPI file discovery
   - [x] Implement spec consolidation

2. **Register in module tree** (completed)
   - [x] Register `pub mod cloudflare;` in `providers/mod.rs`
   - [x] Wire into `fetcher.rs` in `bin/platform`

3. **Write unit tests** (completed)
   - [x] Test file discovery logic
   - [x] Test spec consolidation

4. **Integration test** (completed)
   - [x] Run full fetch and verify output
   - [x] Verify all Cloudflare API schemas are captured

5. **Handle edge cases** (completed)
   - [x] Handle git clone failures gracefully
   - [x] Implement shallow clone for speed (`--depth 1`)

6. **Resource type generation** (pending)
   - [ ] Generate resource types from OpenAPI schemas
   - [ ] Write to `providers/cloudflare/resources/mod.rs`
   - [ ] Fix generated code to compile cleanly (no `#[allow(...)]`)

7. **Operation traits generation** (pending)
   - [ ] Generate `CloudflareResource` base trait
   - [ ] Generate resource-specific operation traits

## Success Criteria

- [x] Cloudflare specs fetched completely
- [x] Multi-file structure handled correctly
- [x] Specs stored in `artefacts/cloud_providers/cloudflare/`
- [ ] Resource types generated correctly into `providers/cloudflare/resources/`
- [ ] Generated code compiles without errors or warnings

## Verification

```bash
# Fetch spec
cargo run -- gen_provider_specs --provider cloudflare

# Verify artefacts
ls artefacts/cloud_providers/cloudflare/openapi.json
cat artefacts/cloud_providers/cloudflare/_manifest.json

# Verify resources (once generation is complete)
cargo check -p foundation_deployment
```

---

_Created: 2026-03-27_
_Updated: 2026-04-05 - Corrected directory structure to providers/cloudflare/_
