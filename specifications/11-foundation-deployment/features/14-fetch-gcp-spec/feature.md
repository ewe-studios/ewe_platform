---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/14-fetch-gcp-spec"
this_file: "specifications/11-foundation-deployment/features/14-fetch-gcp-spec/feature.md"

status: pending
priority: medium
created: 2026-03-27

depends_on: ["10-provider-spec-fetcher-core"]

tasks:
  completed: 0
  uncompleted: 6
  total: 6
  completion_percentage: 0%
---


# Fetch GCP Discovery Spec

## Iron Law: Zero Warnings

> **All code must compile with zero warnings and pass all lints. No suppression. No exceptions.**

## Overview

Implement the GCP (Google Cloud Platform) Discovery Service fetcher. GCP is unique because it returns a **directory of APIs** rather than a single spec. The fetcher must:
1. Fetch the discovery directory
2. Extract all API entries
3. Fetch each API's discovery document in parallel
4. Consolidate into versioned snapshots

This is the most complex fetcher due to the multi-stage, multi-file nature.

## GCP Spec Details

| Property | Value |
|----------|-------|
| Directory URL | `https://discovery.googleapis.com/discovery/v1/apis` |
| Format | Discovery Directory + Individual discoveryRestUrl |
| Auth Required | No |
| API Count | 200+ APIs |
| Special Handling | Two-stage fetch, parallel document retrieval |

## Requirements

### GCP-Specific Fetcher

```rust
// bin/platform/src/gen_provider_specs/providers/gcp.rs

use crate::errors::SpecFetchError;  // Import from central errors.rs
use crate::core::{DistilledSpec, SpecEndpoint};  // Core types from core.rs
use foundation_core::valtron::{TaskIterator, TaskIteratorExt};

pub const GCP_DISCOVERY_URL: &str = "https://discovery.googleapis.com/discovery/v1/apis";

#[derive(Debug, Deserialize)]
struct GcpDirectoryResponse {
    items: Vec<GcpApiEntry>,
}

#[derive(Debug, Deserialize, Clone)]
struct GcpApiEntry {
    id: String,
    name: String,
    version: String,
    title: String,
    description: String,
    discoveryRestUrl: String,
    preferred: bool,
}

#[derive(Debug, Serialize)]
struct GcpDistilledSpec {
    provider: String,
    fetched_at: chrono::DateTime<chrono::Utc>,
    source_url: String,
    api_count: usize,
    apis: Vec<GcpApiSnapshot>,
    content_hash: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct GcpApiSnapshot {
    name: String,
    version: String,
    title: String,
    preferred: bool,
    filename: String,
    discovery_doc: serde_json::Value,
}

pub async fn fetch_gcp_specs(
    client: &SimpleHttpClient,
) -> Result<GcpDistilledSpec, SpecFetchError> {
    // Stage 1: Fetch the directory
    let dir_response = client
        .get(GCP_DISCOVERY_URL)
        .send()
        .await
        .map_err(|e| SpecFetchError::Http {
            provider: "gcp".to_string(),
            source: e,
        })?;

    let directory: GcpDirectoryResponse = serde_json::from_str(&dir_response.body)
        .map_err(|e| SpecFetchError::Json {
            provider: "gcp".to_string(),
            source: e,
        })?;

    println!("Found {} GCP APIs", directory.items.len());

    // Stage 2: Fetch all discovery documents in parallel
    let apis = directory
        .items
        .clone()
        .into_task_iter()
        .map_with_progress(|entry| fetch_single_gcp_api(client, entry))
        .buffered(20) // Fetch 20 APIs in parallel
        .collect::<Vec<_>>()
        .await;

    // Separate successes from failures
    let (successes, failures): (Vec<_>, Vec<_>) = apis.into_iter().partition(|r| r.is_ok());

    let mut api_snapshots = Vec::new();
    for Ok(snapshot) in successes {
        api_snapshots.push(snapshot);
    }

    // Log failures but don't fail entire operation
    for Err((entry, err)) in failures {
        eprintln!("Warning: Failed to fetch {}@{}: {err}", entry.name, entry.version);
    }

    let consolidated = serde_json::json!({
        "apis": api_snapshots.iter().map(|s| {
            serde_json::json!({
                "name": s.name,
                "version": s.version,
                "title": s.title,
                "preferred": s.preferred,
            })
        }).collect::<Vec<_>>()
    });

    let content_hash = compute_sha256(&consolidated.to_string());

    Ok(GcpDistilledSpec {
        provider: "gcp".to_string(),
        fetched_at: chrono::Utc::now(),
        source_url: GCP_DISCOVERY_URL.to_string(),
        api_count: api_snapshots.len(),
        apis: api_snapshots,
        content_hash,
    })
}

async fn fetch_single_gcp_api(
    client: &SimpleHttpClient,
    entry: GcpApiEntry,
) -> Result<GcpApiSnapshot, (GcpApiEntry, SpecFetchError)> {
    let response = client
        .get(&entry.discoveryRestUrl)
        .send()
        .await
        .map_err(|e| {
            (
                entry.clone(),
                SpecFetchError::Http {
                    provider: format!("gcp/{}", entry.name),
                    source: e,
                },
            )
        })?;

    if response.status != 200 {
        return Err((
            entry.clone(),
            SpecFetchError::BadStatus {
                provider: format!("gcp/{}", entry.name),
                status: response.status,
            },
        ));
    }

    let discovery_doc: serde_json::Value = serde_json::from_str(&response.body)
        .map_err(|e| {
            (
                entry.clone(),
                SpecFetchError::Json {
                    provider: format!("gcp/{}", entry.name),
                    source: e,
                },
            )
        })?;

    let filename = format!("{}-{}.json", entry.name, entry.version);

    Ok(GcpApiSnapshot {
        name: entry.name,
        version: entry.version,
        title: entry.title,
        preferred: entry.preferred,
        filename,
        discovery_doc,
    })
}

/// Write GCP specs - creates individual files for each API
pub async fn write_gcp_specs(
    spec: &GcpDistilledSpec,
    output_dir: &Path,
) -> Result<(), SpecFetchError> {
    let specs_dir = output_dir.join("specs");
    tokio::fs::create_dir_all(&specs_dir).await?;

    // Write each API's discovery document
    for api in &spec.apis {
        let path = specs_dir.join(&api.filename);
        let json = serde_json::to_string_pretty(&api.discovery_doc)?;
        tokio::fs::write(&path, json).await?;
    }

    // Write consolidated manifest
    let manifest = serde_json::json!({
        "provider": "gcp",
        "fetched_at": spec.fetched_at,
        "api_count": spec.api_count,
        "content_hash": spec.content_hash,
        "apis": spec.apis.iter().map(|a| serde_json::json!({
            "name": a.name,
            "version": a.version,
            "title": a.title,
            "preferred": a.preferred,
            "filename": a.filename,
        })).collect::<Vec<_>>(),
    });

    tokio::fs::write(
        specs_dir.join("_manifest.json"),
        serde_json::to_string_pretty(&manifest)?,
    ).await?;

    Ok(())
}
```

## Error Handling

**All errors are defined in `errors.rs` at the module root.** GCP uses multiple error variants:

```rust
use crate::errors::SpecFetchError;

// HTTP errors
SpecFetchError::Http { provider: "gcp".to_string(), source: e }
SpecFetchError::BadStatus { provider: "gcp/compute".to_string(), status: 404 }

// JSON parsing errors
SpecFetchError::Json { provider: "gcp".to_string(), source: e }

// File I/O errors (via #[from] auto-conversion)
tokio::fs::write(...).await?  // std::io::Error -> SpecFetchError::WriteFile
```

## Tasks

1. **Create GCP provider module**
   - [ ] Create `bin/platform/src/gen_provider_specs/providers/gcp.rs`
   - [ ] Implement `GcpDirectoryResponse` and `GcpApiEntry` structs
   - [ ] Implement `GcpDistilledSpec` and `GcpApiSnapshot` structs

2. **Implement two-stage fetch**
   - [ ] Implement directory fetch
   - [ ] Implement parallel API document fetch with Valtron
   - [ ] Handle partial failures gracefully

3. **Implement file I/O**
   - [ ] Implement `write_gcp_specs()` for multi-file output
   - [ ] Write individual API discovery documents
   - [ ] Write consolidated manifest

4. **Register in core fetcher**
   - [ ] Add GCP to provider list
   - [ ] Handle special multi-file write logic

5. **Write unit tests**
   - [ ] Test directory parsing
   - [ ] Test single API fetch
   - [ ] Test manifest generation

6. **Integration test**
   - [ ] Run full GCP fetch (may take 30+ seconds for 200+ APIs)
   - [ ] Verify all files are written correctly

## Success Criteria

- [ ] All 6 tasks completed
- [ ] Zero warnings
- [ ] 200+ GCP APIs fetched successfully
- [ ] Partial failures don't crash entire fetch
- [ ] Manifest accurately reflects fetched APIs

## Verification

```bash
cd bin/platform

# Fetch GCP specs (this takes a while)
cargo run -- gen_provider_specs --provider gcp

# Count fetched APIs
ls ../../@formulas/src.rust/src.deployAnywhere/distilled-spec-gcp/specs/*.json | wc -l

# View manifest
cat ../../@formulas/src.rust/src.deployAnywhere/distilled-spec-gcp/specs/_manifest.json
```

---

_Created: 2026-03-27_
