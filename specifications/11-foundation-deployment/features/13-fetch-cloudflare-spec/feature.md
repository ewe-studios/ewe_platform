---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/13-fetch-cloudflare-spec"
this_file: "specifications/11-foundation-deployment/features/13-fetch-cloudflare-spec/feature.md"

status: pending
priority: medium
created: 2026-03-27

depends_on: ["10-provider-spec-fetcher-core"]

tasks:
  completed: 0
  uncompleted: 5
  total: 5
  completion_percentage: 0%
---


# Fetch Cloudflare OpenAPI Spec

## Iron Law: Zero Warnings

> **All code must compile with zero warnings and pass all lints. No suppression. No exceptions.**

## Overview

Implement the Cloudflare OpenAPI spec fetcher. Cloudflare's API schemas are hosted in a GitHub repository (`https://github.com/cloudflare/api-schemas`), requiring special handling compared to direct JSON endpoints.

## Cloudflare Spec Details

| Property | Value |
|----------|-------|
| URL | `https://github.com/cloudflare/api-schemas` |
| Format | GitHub Repository (OpenAPI JSON files) |
| Auth Required | No (public repo) |
| Special Handling | Clone repo or use GitHub API |

## Requirements

### Cloudflare-Specific Fetcher

Two approaches:

**Option A: Clone GitHub repo (recommended for completeness)**

```rust
// bin/platform/src/gen_provider_specs/providers/cloudflare.rs

use crate::errors::SpecFetchError;  // Import from central errors.rs
use crate::core::DistilledSpec;  // Core types from core.rs
use std::process::Command;

pub const CLOUDFLARE_API_SCHEMAS_URL: &str = "https://github.com/cloudflare/api-schemas";

pub async fn fetch_cloudflare_specs(
    temp_dir: &Path,
    output_dir: &Path,
) -> Result<DistilledSpec, SpecFetchError> {
    // Clone the repo
    let clone_output = Command::new("git")
        .args(["clone", "--depth", "1", CLOUDFLARE_API_SCHEMAS_URL, "cloudflare-schemas"])
        .current_dir(temp_dir)
        .output()
        .map_err(|e| SpecFetchError::Git {
            repo: CLOUDFLARE_API_SCHEMAS_URL.to_string(),
            reason: format!("Failed to clone: {e}"),
        })?;

    if !clone_output.status.success() {
        return Err(SpecFetchError::Git {
            repo: CLOUDFLARE_API_SCHEMAS_URL.to_string(),
            reason: String::from_utf8_lossy(&clone_output.stderr).to_string(),
        });
    }

    // Find all OpenAPI JSON files in the cloned repo
    let spec_files = find_openapi_files(temp_dir.join("cloudflare-schemas"));

    // Consolidate into single distillation
    let mut consolidated = serde_json::Map::new();
    for file in spec_files {
        let content = tokio::fs::read_to_string(&file).await?;
        let spec: serde_json::Value = serde_json::from_str(&content)?;
        // Merge specs...
    }

    let content_hash = compute_sha256(&consolidated.to_string());

    Ok(DistilledSpec {
        provider: "cloudflare".to_string(),
        version: chrono::Utc::now().format("%Y%m%d").to_string(),
        fetched_at: chrono::Utc::now(),
        source_url: CLOUDFLARE_API_SCHEMAS_URL.to_string(),
        raw_spec: serde_json::Value::Object(consolidated),
        endpoints: None, // Complex multi-spec structure
        content_hash,
    })
}

fn find_openapi_files(dir: &Path) -> Vec<PathBuf> {
    // Recursively find all .json files that look like OpenAPI specs
    // Check for "openapi": "3" or "swagger" keys
}
```

**Option B: Use GitHub Contents API**

```rust
// Fetch specific known schema files via GitHub API
// GET /repos/cloudflare/api-schemas/contents/...
// Less data transfer but requires knowing exact file paths
```

## Error Handling

**All errors are defined in `errors.rs` at the module root.** Cloudflare uses the `Git` variant:
```rust
use crate::errors::SpecFetchError;

// Git clone failure
SpecFetchError::Git {
    repo: CLOUDFLARE_API_SCHEMAS_URL.to_string(),
    reason: format!("Failed to clone: {e}"),
}
```

## Tasks

1. **Create Cloudflare provider module**
   - [ ] Create `bin/platform/src/gen_provider_specs/providers/cloudflare.rs`
   - [ ] Implement git clone approach
   - [ ] Implement OpenAPI file discovery
   - [ ] Implement spec consolidation

2. **Register in core fetcher**
   - [ ] Add Cloudflare to provider list
   - [ ] Handle multi-file spec structure

3. **Write unit tests**
   - [ ] Test file discovery logic
   - [ ] Test spec consolidation

4. **Integration test**
   - [ ] Run full fetch and verify output
   - [ ] Verify all Cloudflare API schemas are captured

5. **Handle edge cases**
   - [ ] Handle git clone failures gracefully
   - [ ] Handle repo structure changes
   - [ ] Implement shallow clone for speed

## Success Criteria

- [ ] All 5 tasks completed
- [ ] Zero warnings
- [ ] Cloudflare specs fetched completely
- [ ] Multi-file structure handled correctly

## Verification

```bash
cargo run -- gen_provider_specs --provider cloudflare
```

---

_Created: 2026-03-27_
