---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/14-fetch-gcp-spec"
this_file: "specifications/11-foundation-deployment/features/14-fetch-gcp-spec/feature.md"

status: completed
priority: high
created: 2026-03-27
updated: 2026-04-05

depends_on: ["10-provider-spec-fetcher-core"]

tasks:
  completed: 6
  uncompleted: 0
  total: 6
  completion_percentage: 100%
---


# Fetch GCP Discovery Spec + Resource Generation

## Iron Law: Zero Warnings

> **All code must compile with zero warnings and pass all lints. No suppression. No exceptions.**

## Overview

Implement the GCP Discovery Service fetcher with **resource type generation**. This feature:

1. **Fetches the GCP Discovery directory** from `discovery.googleapis.com`
2. **Extracts Cloud Run v2 API** and other relevant service specs
3. **Stores raw JSON specs** in `artefacts/cloud_providers/gcp/`
4. **Generates Rust resource types** from the GCP Discovery schemas into `resources/`

The generated resources are used by the GCP Cloud Run provider (Feature 05) for type-safe API interactions.

## Architecture

All provider implementations live in `backends/foundation_deployment/src/providers/{provider}/`:

```
backends/foundation_deployment/src/providers/gcp/
├── mod.rs             # Module declaration (pub mod fetch; pub mod provider; pub mod resources;)
├── provider.rs        # DeploymentProvider trait implementation
├── fetch.rs           # Two-stage Discovery API spec fetcher
└── resources/
    ├── mod.rs         # Registers all generated API modules
    ├── run.rs         # Auto-generated: Cloud Run v2 resource types
    ├── compute.rs     # Auto-generated: Compute Engine resource types
    ├── iam.rs         # Auto-generated: IAM resource types
    └── ...            # One flattened .rs file per GCP API
```

Fetched raw specs are stored separately in:
```
artefacts/cloud_providers/gcp/
├── run/openapi.json
├── compute/openapi.json
├── iam/openapi.json
├── _manifest.json
└── ...
```

**GCP resources are flattened**: each API gets a single `{api}.rs` file directly under `resources/`, not a subdirectory.

## GCP Spec Details

| Property | Value |
|----------|-------|
| Directory URL | `https://discovery.googleapis.com/discovery/v1/apis` |
| Format | Discovery Directory + Individual discoveryRestUrl |
| Auth Required | No |
| API Count | 200+ APIs (we filter to relevant ones) |
| Special Handling | Two-stage fetch, parallel document retrieval |

## Requirements

### Spec Fetching + Storage

```rust
// bin/platform/src/gen_provider_specs/providers/gcp.rs

use crate::errors::SpecFetchError;
use crate::core::DistilledSpec;
use foundation_core::valtron::{TaskIterator, TaskIteratorExt};
use std::path::{Path, PathBuf};

pub const GCP_DISCOVERY_URL: &str = "https://discovery.googleapis.com/discovery/v1/apis";

/// Output path for resource specs in foundation_deployment crate.
pub const RESOURCE_SPECS_DIR: &str = "backends/foundation_deployment/src/providers/gcp/resource_specs";

/// GCP APIs relevant for deployment (Cloud Run, IAM, Secret Manager, etc.)
const RELEVANT_APIS: &[&str] = &[
    "run",        // Cloud Run
    "iam",        // IAM service accounts
    "secretmanager",  // Secret Manager
    "artifactregistry",  // Container Registry
    "cloudbuild",  // Cloud Build
    "storage",    // GCS
];

#[derive(Debug, Deserialize, Clone)]
struct GcpApiEntry {
    id: String,
    name: String,
    version: String,
    title: String,
    discoveryRestUrl: String,
    preferred: bool,
}

pub async fn fetch_gcp_specs(
    client: &SimpleHttpClient,
) -> Result<DistilledSpec, SpecFetchError> {
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

    // Filter to relevant APIs
    let relevant_entries: Vec<_> = directory
        .items
        .into_iter()
        .filter(|item| RELEVANT_APIS.contains(&item.name.as_str()))
        .collect();

    println!("Found {} relevant GCP APIs", relevant_entries.len());

    // Stage 2: Fetch all relevant discovery documents in parallel
    let specs = relevant_entries
        .clone()
        .into_task_iter()
        .map_with_progress(|entry| fetch_single_gcp_api(client, entry))
        .buffered(10)
        .collect::<Vec<_>>()
        .await;

    // Copy specs to resource_specs directory
    let dest_dir = PathBuf::from(RESOURCE_SPECS_DIR);
    tokio::fs::create_dir_all(&dest_dir).await?;

    let mut spec_files = Vec::new();
    for (entry, spec_result) in relevant_entries.iter().zip(specs) {
        match spec_result {
            Ok(spec) => {
                let filename = format!("{}-{}.json", entry.name, entry.version);
                let dest_path = dest_dir.join(&filename);
                tokio::fs::write(&dest_path, serde_json::to_string_pretty(&spec)?).await?;
                spec_files.push((filename, entry.clone()));
                println!("  Saved: {}", filename);
            }
            Err(e) => {
                eprintln!("Warning: Failed to fetch {}@{}: {e}", entry.name, entry.version);
            }
        }
    }

    // Write manifest
    write_manifest(&dest_dir, &spec_files).await?;

    Ok(DistilledSpec { /* ... */ })
}

async fn fetch_single_gcp_api(
    client: &SimpleHttpClient,
    entry: GcpApiEntry,
) -> Result<serde_json::Value, SpecFetchError> {
    let response = client
        .get(&entry.discoveryRestUrl)
        .send()
        .await
        .map_err(|e| SpecFetchError::Http {
            provider: format!("gcp/{}", entry.name),
            source: e,
        })?;

    if response.status != 200 {
        return Err(SpecFetchError::BadStatus {
            provider: format!("gcp/{}", entry.name),
            status: response.status,
        });
    }

    let spec: serde_json::Value = serde_json::from_str(&response.body)
        .map_err(|e| SpecFetchError::Json {
            provider: format!("gcp/{}", entry.name),
            source: e,
        })?;

    Ok(spec)
}

async fn write_manifest(dest_dir: &Path, files: &[(String, GcpApiEntry)]) -> Result<(), SpecFetchError> {
    let manifest = serde_json::json!({
        "source": GCP_DISCOVERY_URL,
        "fetched_at": chrono::Utc::now().to_rfc3339(),
        "apis": files.iter().map(|(name, entry)| serde_json::json!({
            "filename": name,
            "id": entry.id,
            "name": entry.name,
            "version": entry.version,
            "title": entry.title,
            "preferred": entry.preferred,
        })).collect::<Vec<_>>(),
    });

    tokio::fs::write(
        dest_dir.join("_manifest.json"),
        serde_json::to_string_pretty(&manifest)?,
    ).await?;

    Ok(())
}
```

### Resource Type Generation (GCP Discovery Format)

```rust
// bin/platform/src/gen_provider_specs/generators/gcp.rs

use serde_json::Value;
use std::{fs, path::Path};

/// Generate Rust resource types from GCP Discovery specs.
pub fn generate_gcp_resources(specs_dir: &Path) -> Result<(), SpecFetchError> {
    let mut generated_code = String::new();

    // Header
    generated_code.push_str("// DO NOT EDIT - Generated from GCP Discovery specs\n");
    generated_code.push_str("// Source: https://discovery.googleapis.com/discovery/v1/apis\n");
    generated_code.push_str("// Generated by: cargo run -- gen_provider_specs --provider gcp\n\n");
    generated_code.push_str("use serde::{Deserialize, Serialize};\n");
    generated_code.push_str("use std::collections::HashMap;\n\n");

    // Load manifest
    let manifest_content = fs::read_to_string(specs_dir.join("_manifest.json"))?;
    let manifest: Value = serde_json::from_str(&manifest_content)?;

    let apis = manifest.get("apis")
        .and_then(|a| a.as_array())
        .expect("manifest must have apis array");

    for api_entry in apis {
        let filename = api_entry.get("filename")
            .and_then(|f| f.as_str())
            .expect("api must have filename");
        let api_name = api_entry.get("name")
            .and_then(|n| n.as_str())
            .expect("api must have name");

        let spec_path = specs_dir.join(filename);
        let spec_content = fs::read_to_string(&spec_path)?;
        let spec: Value = serde_json::from_str(&spec_content)?;

        generated_code.push_str(&format!("// === API: {} ===\n\n", api_name.to_uppercase()));

        // GCP Discovery uses "schemas" under "resources" or top-level "schemas"
        if let Some(schemas) = spec.get("schemas").and_then(|s| s.as_object()) {
            generated_code.push_str(&generate_gcp_schemas(schemas, filename)?);
        }

        // Also check for nested resources
        if let Some(resources) = spec.get("resources").and_then(|r| r.as_object()) {
            generated_code.push_str(&generate_gcp_resources_section(resources, filename)?);
        }
    }

    // Write generated code
    let output_path = "backends/foundation_deployment/src/providers/gcp/resources.rs";
    fs::write(output_path, generated_code)?;

    Ok(())
}

fn generate_gcp_schemas(
    schemas: &serde_json::Map<String, Value>,
    source_file: &str,
) -> Result<String, SpecFetchError> {
    let mut output = String::new();

    for (name, schema) in schemas {
        let struct_name = to_pascal_case(name);
        let doc_comment = format!("/// Generated from: {}#{}", source_file, name);

        output.push_str(&doc_comment);
        output.push_str("\n#[derive(Debug, Clone, Deserialize, Serialize)]\n");
        output.push_str("#[serde(rename_all = \"camelCase\")]\n");
        output.push_str("pub struct ");
        output.push_str(&struct_name);
        output.push_str(" {\n");

        if let Some(props) = schema.get("properties").and_then(|p| p.as_object()) {
            let required = schema.get("id")  // GCP uses "id" for type identification
                .map(|_| vec![])  // GCP Discovery doesn't have "required" array
                .unwrap_or_default();

            for (prop_name, prop_schema) in props {
                let rust_name = to_snake_case(prop_name);
                let rust_type = gcp_schema_to_rust_type(prop_schema);

                // Add serde rename if needed
                if prop_name.contains('_') || !prop_name.is_ascii() {
                    output.push_str(&format!("    #[serde(rename = \"{}\")]\n", prop_name));
                }

                // GCP uses "outputOnly" annotation for read-only fields
                let annotations = prop_schema.get("annotations")
                    .and_then(|a| a.as_object())
                    .map(|a| a.contains_key("outputOnly"))
                    .unwrap_or(false);

                if annotations {
                    output.push_str("    // Output-only field\n");
                }

                output.push_str(&format!("    pub {}: {},\n", rust_name, rust_type));
            }
        }

        output.push_str("}\n\n");
    }

    Ok(output)
}

fn gcp_schema_to_rust_type(schema: &Value) -> String {
    if let Some(obj) = schema.as_object() {
        let type_ = obj.get("type").and_then(|v| v.as_str()).unwrap_or("any");

        match type_ {
            "string" => {
                // Check for format
                let format = obj.get("format").and_then(|v| v.as_str());
                match format {
                    Some("date-time") | Some("google-datetime") => "String".to_string(),  // RFC3339
                    Some("google-duration") => "String".to_string(),  // protobuf.Duration
                    Some("int64") => "i64".to_string(),
                    _ => "String".to_string(),
                }
            }
            "integer" => {
                let format = obj.get("format").and_then(|v| v.as_str()).unwrap_or("i64");
                match format {
                    "int32" => "i32".to_string(),
                    "int64" => "i64".to_string(),
                    "uint32" => "u32".to_string(),
                    "uint64" => "u64".to_string(),
                    _ => "i64".to_string(),
                }
            }
            "number" => "f64".to_string(),
            "boolean" => "bool".to_string(),
            "array" => {
                if let Some(items) = obj.get("items") {
                    let inner_type = gcp_schema_to_rust_type(items);
                    format!("Vec<{}>", inner_type)
                } else {
                    "Vec<Value>".to_string()
                }
            }
            "object" => {
                if obj.contains_key("properties") {
                    "HashMap<String, Value>".to_string()  // Would need inline struct
                } else {
                    "HashMap<String, Value>".to_string()
                }
            }
            "any" => "Value".to_string(),
            _ => "Value".to_string(),
        }
    } else {
        "Value".to_string()
    }
}
```

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

Errors use `DeploymentError` from `foundation_deployment::error`. The fetcher in `bin/platform` converts to `SpecFetchError` at the orchestration layer.

## Tasks

1. **Create GCP provider module** (completed)
   - [x] Create `backends/foundation_deployment/src/providers/gcp/fetch.rs`
   - [x] Implement two-stage Discovery API fetch (directory listing → per-API specs)
   - [x] Implement `GcpDirectoryResponse` and `GcpApiEntry` structs

2. **Implement two-stage fetch** (completed)
   - [x] Implement directory fetch from `https://discovery.googleapis.com/discovery/v1/apis`
   - [x] Implement parallel API document fetch with Valtron `ConcurrentQueueStreamIterator`
   - [x] Handle partial failures gracefully (continue on individual API errors)

3. **Implement file I/O** (completed)
   - [x] Write individual API discovery documents to `artefacts/cloud_providers/gcp/specs/`
   - [x] Write consolidated `_manifest.json`

4. **Register in core fetcher** (completed)
   - [x] Add GCP to provider list in `bin/platform/src/gen_provider_specs/fetcher.rs`
   - [x] Handle special multi-file write logic

5. **Write unit tests** (completed)
   - [x] Test directory parsing
   - [x] Test single API fetch
   - [x] Test manifest generation

6. **Integration test** (completed)
   - [x] Run full GCP fetch (200+ APIs)
   - [x] Verify all files are written correctly

## Success Criteria

- [x] All 6 tasks completed
- [x] Zero warnings
- [x] 200+ GCP APIs fetched successfully
- [x] Partial failures don't crash entire fetch
- [x] Manifest accurately reflects fetched APIs

## Verification

```bash
# Fetch GCP specs (this takes a while due to 200+ APIs)
cargo run -- gen_provider_specs --provider gcp

# Verify artefacts
ls artefacts/cloud_providers/gcp/specs/*.json | wc -l
cat artefacts/cloud_providers/gcp/specs/_manifest.json
```

---

_Created: 2026-03-27_
_Updated: 2026-04-04 - Corrected directory structure to backends/foundation_deployment, marked tasks complete_
