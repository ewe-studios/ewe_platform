---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/13-fetch-cloudflare-spec"
this_file: "specifications/11-foundation-deployment/features/13-fetch-cloudflare-spec/feature.md"

status: pending
priority: high
created: 2026-03-27
updated: 2026-03-28

depends_on: ["10-provider-spec-fetcher-core"]

tasks:
  completed: 0
  uncompleted: 7
  total: 7
  completion_percentage: 0%
---


# Fetch Cloudflare OpenAPI Spec + Resource Generation

## Iron Law: Zero Warnings

> **All code must compile with zero warnings and pass all lints. No suppression. No exceptions.**

## Overview

Implement the Cloudflare OpenAPI spec fetcher with **resource type generation**. This feature:

1. **Fetches the Cloudflare API schemas** from the GitHub repository
2. **Stores raw JSON specs** in `backends/foundation_deployment/src/providers/cloudflare/resource_specs/`
3. **Generates Rust resource types** from the OpenAPI schemas
4. **Generates operation traits** for each resource (CRUD operations)

The generated resources are used by the Cloudflare provider (Feature 04) for type-safe API interactions.

## Output Structure

```
backends/foundation_deployment/src/providers/cloudflare/
├── mod.rs                 # Provider implementation
├── resources.rs           # Generated resource types (DO NOT EDIT)
├── operations.rs          # Generated operation traits (DO NOT EDIT)
└── resource_specs/        # Raw JSON specs used for generation
    ├── workers-api.json
    ├── kv-api.json
    ├── d1-api.json
    ├── r2-api.json
    ├── queues-api.json
    └── _manifest.json     # Generation metadata
```

## Cloudflare Spec Details

| Property | Value |
|----------|-------|
| Source URL | `https://github.com/cloudflare/api-schemas` |
| Format | GitHub Repository (OpenAPI JSON files) |
| Auth Required | No (public repo) |
| Special Handling | Git clone required |

## Requirements

### Spec Fetching + Storage

```rust
// bin/platform/src/gen_provider_specs/providers/cloudflare.rs

use crate::errors::SpecFetchError;
use crate::core::DistilledSpec;
use std::process::Command;
use std::path::{Path, PathBuf};

pub const CLOUDFLARE_API_SCHEMAS_URL: &str = "https://github.com/cloudflare/api-schemas";

/// Output path for resource specs in foundation_deployment crate.
pub const RESOURCE_SPECS_DIR: &str = "backends/foundation_deployment/src/providers/cloudflare/resource_specs";

pub async fn fetch_cloudflare_specs(
    temp_dir: &Path,
    output_dir: &Path,  // Path to distilled-spec-cloudflare repo
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

    let source_dir = temp_dir.join("cloudflare-schemas");
    let dest_dir = PathBuf::from(RESOURCE_SPECS_DIR);

    // Find and copy relevant API specs
    let spec_files = find_cloudflare_api_files(&source_dir)?;

    // Copy specs to resource_specs directory
    tokio::fs::create_dir_all(&dest_dir).await?;
    for (name, src_path) in spec_files {
        let dest_path = dest_dir.join(&name);
        tokio::fs::copy(&src_path, &dest_path).await?;
        println!("  Copied: {name}");
    }

    // Generate manifest
    write_manifest(&dest_dir, &spec_files).await?;

    // Also write to distilled-spec-cloudflare for versioning
    // ... (existing distilled-spec logic)

    Ok(DistilledSpec { /* ... */ })
}

fn find_cloudflare_api_files(source: &Path) -> Result<Vec<(String, PathBuf)>, SpecFetchError> {
    // Find relevant API spec files:
    // - workers-api.json (Worker scripts, services, bindings)
    // - kv.json (KV namespaces)
    // - d1.json (D1 databases)
    // - r2.json (R2 buckets)
    // - queues.json (Queue management)
    // - vectorize.json (Vectorize indexes)
    // - hyperdrive.json (Hyperdrive configs)

    let relevant_prefixes = [
        "workers", "kv", "d1", "r2", "queues", "vectorize", "hyperdrive", "pages"
    ];

    let mut files = Vec::new();
    for entry in walkdir::WalkDir::new(source).into_iter().flatten() {
        if entry.path().extension() == Some("json".as_ref()) {
            let file_name = entry.path().file_name().unwrap().to_string_lossy();
            if relevant_prefixes.iter().any(|p| file_name.starts_with(p)) {
                files.push((file_name.to_string(), entry.path().to_path_buf()));
            }
        }
    }

    Ok(files)
}

async fn write_manifest(dest_dir: &Path, files: &[(String, PathBuf)]) -> Result<(), SpecFetchError> {
    let manifest = serde_json::json!({
        "source": CLOUDFLARE_API_SCHEMAS_URL,
        "fetched_at": chrono::Utc::now().to_rfc3339(),
        "files": files.iter().map(|(name, _)| name).collect::<Vec<_>>(),
    });

    tokio::fs::write(
        dest_dir.join("_manifest.json"),
        serde_json::to_string_pretty(&manifest)?,
    ).await?;

    Ok(())
}
```

### Resource Type Generation

```rust
// bin/platform/src/gen_provider_specs/generators/cloudflare.rs

use serde_json::Value;
use std::{collections::HashMap, fs, path::Path};

/// Generate Rust resource types from Cloudflare OpenAPI specs.
pub fn generate_cloudflare_resources(specs_dir: &Path) -> Result<(), SpecFetchError> {
    let mut generated_code = String::new();

    // Header
    generated_code.push_str("// DO NOT EDIT - Generated from Cloudflare OpenAPI specs\n");
    generated_code.push_str("// Source: https://github.com/cloudflare/api-schemas\n");
    generated_code.push_str("// Generated by: cargo run -- gen_provider_specs --provider cloudflare\n\n");
    generated_code.push_str("use serde::{Deserialize, Serialize};\n");
    generated_code.push_str("use std::collections::HashMap;\n\n");

    // Load and parse each spec
    let manifest_content = fs::read_to_string(specs_dir.join("_manifest.json"))?;
    let manifest: Value = serde_json::from_str(&manifest_content)?;

    let files = manifest.get("files")
        .and_then(|f| f.as_array())
        .expect("manifest must have files array");

    for file in files {
        let file_name = file.as_str().unwrap();
        let spec_path = specs_dir.join(file_name);
        let spec_content = fs::read_to_string(&spec_path)?;
        let spec: Value = serde_json::from_str(&spec_content)?;

        // Extract schemas from components.schemas
        if let Some(schemas) = spec.get("components").and_then(|c| c.get("schemas")) {
            generated_code.push_str(&generate_schemas(schemas, file_name)?);
        }

        // Extract request/response types from paths
        if let Some(paths) = spec.get("paths") {
            generated_code.push_str(&generate_path_types(paths, file_name)?);
        }
    }

    // Write generated code
    let output_path = "backends/foundation_deployment/src/providers/cloudflare/resources.rs";
    fs::write(output_path, generated_code)?;

    Ok(())
}

fn generate_schemas(schemas: &Value, source_file: &str) -> Result<String, SpecFetchError> {
    let mut output = String::new();

    if let Some(obj) = schemas.as_object() {
        for (name, schema) in obj {
            let struct_name = to_pascal_case(name);
            let doc_comment = format!("/// Generated from: {}#{}", source_file, name);

            output.push_str(&doc_comment);
            output.push_str("\n#[derive(Debug, Clone, Deserialize, Serialize)]\n");

            // Handle different schema types
            if let Some(props) = schema.get("properties") {
                output.push_str("pub struct ");
                output.push_str(&struct_name);
                output.push_str(" {\n");

                if let Some(props_obj) = props.as_object() {
                    let required = schema.get("required")
                        .and_then(|r| r.as_array())
                        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
                        .unwrap_or_default();

                    for (prop_name, prop_schema) in props_obj {
                        let rust_name = to_snake_case(prop_name);
                        let rust_type = schema_to_rust_type(prop_schema);
                        let is_required = required.contains(&prop_name.as_str());

                        // Add serde rename if needed
                        if prop_name.contains('_') || prop_name != rust_name {
                            output.push_str(&format!("    #[serde(rename = \"{}\")]\n", prop_name));
                        }

                        if is_required {
                            output.push_str(&format!("    pub {}: {},\n", rust_name, rust_type));
                        } else {
                            output.push_str(&format!("    pub {}: Option<{}>,\n", rust_name, rust_type));
                        }
                    }
                }

                output.push_str("}\n\n");
            }
        }
    }

    Ok(output)
}

fn generate_path_types(paths: &Value, source_file: &str) -> Result<String, SpecFetchError> {
    let mut output = String::new();

    if let Some(obj) = paths.as_object() {
        for (path, path_item) in obj {
            if let Some(item_obj) = path_item.as_object() {
                for method in ["get", "post", "put", "patch", "delete"] {
                    if let Some(operation) = item_obj.get(method) {
                        let operation_id = operation.get("operationId")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Unknown");

                        let method_upper = method.to_uppercase();
                        output.push_str(&format!("/// {} {}\n", method_upper, path));
                        output.push_str(&format!("/// Operation: {}\n", operation_id));

                        // Generate request type from parameters
                        if let Some(params) = operation.get("parameters") {
                            output.push_str(&format!("/// Parameters: {:?}\n", params));
                        }

                        // Generate response type from responses
                        if let Some(responses) = operation.get("responses") {
                            output.push_str(&format!("/// Responses: {:?}\n", responses));
                        }

                        output.push('\n');
                    }
                }
            }
        }
    }

    Ok(output)
}

fn schema_to_rust_type(schema: &Value) -> String {
    if let Some(obj) = schema.as_object() {
        // Check for type
        let type_ = obj.get("type").and_then(|v| v.as_str()).unwrap_or("any");

        match type_ {
            "string" => "String".to_string(),
            "integer" => {
                let format = obj.get("format").and_then(|v| v.as_str()).unwrap_or("i64");
                match format {
                    "int32" => "i32".to_string(),
                    "int64" => "i64".to_string(),
                    _ => "i64".to_string(),
                }
            }
            "number" => "f64".to_string(),
            "boolean" => "bool".to_string(),
            "array" => {
                if let Some(items) = obj.get("items") {
                    let inner_type = schema_to_rust_type(items);
                    format!("Vec<{}>", inner_type)
                } else {
                    "Vec<Value>".to_string()
                }
            }
            "object" => {
                if obj.contains_key("properties") {
                    "/* inline struct */".to_string()
                } else {
                    "HashMap<String, Value>".to_string()
                }
            }
            _ => "Value".to_string(),
        }
    } else {
        "Value".to_string()
    }
}

fn to_pascal_case(s: &str) -> String {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(|s| {
            let mut chars = s.chars();
            chars.next().map(|c| c.to_uppercase().collect::<String>()).unwrap_or_default()
                + chars.as_str()
        })
        .collect()
}

fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                result.push('_');
            }
            result.push(c.to_lowercase().next().unwrap());
        } else if c == '-' || c == '.' {
            result.push('_');
        } else {
            result.push(c);
        }
    }
    result
}
```

### Operation Traits Generation

```rust
// Generated operations trait and implementations

/// Base trait for all Cloudflare API operations.
pub trait CloudflareResource: Sized {
    /// API endpoint path template.
    const ENDPOINT: &'static str;

    /// HTTP method for create.
    const CREATE_METHOD: &'static str = "POST";

    /// HTTP method for read.
    const READ_METHOD: &'static str = "GET";

    /// HTTP method for update.
    const UPDATE_METHOD: &'static str = "PUT";

    /// HTTP method for delete.
    const DELETE_METHOD: &'static str = "DELETE";
}

/// CRUD operations for WorkerScript.
impl CloudflareResource for WorkerScript {
    const ENDPOINT: &'static str = "/accounts/{account_id}/workers/scripts/{script_name}";
}

/// Extension trait for WorkerScript-specific operations.
pub trait WorkerScriptOperations: CloudflareResource {
    async fn create(
        client: &SimpleHttpClient,
        account_id: &str,
        script: &WorkerScript,
    ) -> Result<WorkerScript, SpecFetchError>;

    async fn get(
        client: &SimpleHttpClient,
        account_id: &str,
        script_name: &str,
    ) -> Result<WorkerScript, SpecFetchError>;

    async fn update(
        client: &SimpleHttpClient,
        account_id: &str,
        script_name: &str,
        script: &WorkerScript,
    ) -> Result<WorkerScript, SpecFetchError>;

    async fn delete(
        client: &SimpleHttpClient,
        account_id: &str,
        script_name: &str,
    ) -> Result<(), SpecFetchError>;
}
```

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

6. **Resource type generation**
   - [ ] Create `bin/platform/src/gen_provider_specs/generators/cloudflare.rs`
   - [ ] Implement `generate_schemas()` for components.schemas
   - [ ] Implement `generate_path_types()` for paths
   - [ ] Implement type converters (schema_to_rust_type, to_pascal_case, to_snake_case)
   - [ ] Write generated code to `backends/foundation_deployment/src/providers/cloudflare/resources.rs`

7. **Operation traits generation**
   - [ ] Generate `CloudflareResource` base trait
   - [ ] Generate resource-specific operation traits (e.g., `WorkerScriptOperations`)
   - [ ] Write to `backends/foundation_deployment/src/providers/cloudflare/operations.rs`

## Success Criteria

- [ ] All 7 tasks completed
- [ ] Zero warnings
- [ ] Cloudflare specs fetched completely
- [ ] Multi-file structure handled correctly
- [ ] Resource types generated correctly
- [ ] Operation traits generated correctly
- [ ] Specs stored in `resource_specs/` folder
- [ ] Generated code compiles without errors

## Verification

```bash
cargo run -- gen_provider_specs --provider cloudflare

# Verify resource_specs folder
ls backends/foundation_deployment/src/providers/cloudflare/resource_specs/

# Verify generated code
head -50 backends/foundation_deployment/src/providers/cloudflare/resources.rs
```

---

_Created: 2026-03-27_
_Updated: 2026-03-28_
