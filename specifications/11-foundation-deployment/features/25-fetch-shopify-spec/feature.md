---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/21-fetch-shopify-spec"
this_file: "specifications/11-foundation-deployment/features/21-fetch-shopify-spec/feature.md"

status: pending
priority: medium
created: 2026-04-05

depends_on: ["10-provider-spec-fetcher-core", "20-gen-resource-types"]

tasks:
  completed: 0
  uncompleted: 10
  total: 10
  completion_percentage: 0%
---


# Fetch Shopify GraphQL Schema + Resource Generation

## Iron Law: Zero Warnings

> **All code must compile with zero warnings and pass all lints. No suppression. No exceptions.**

## Overview

Implement a Shopify API schema fetcher and the supporting infrastructure to process **GraphQL schemas** as a first-class spec format alongside OpenAPI.

Shopify does **not** publish an official OpenAPI specification. Their REST Admin API is deprecated (legacy since October 2024; new public apps must use GraphQL exclusively since April 2025). The canonical API is the **GraphQL Admin API**, whose schema is available via public introspection endpoints that require no authentication.

This feature introduces:

1. **GraphQL introspection fetcher** — fetches a full schema via a standard introspection query over plain HTTP POST
2. **Shopify-specific fetcher** — targets Shopify's public introspection proxy, handling versioned endpoints
3. **GraphQL schema processing utilities** — parallel to `openapi.rs`, extracts types, queries, mutations from introspection JSON
4. **GraphQL-to-Rust type generation** — parallel to OpenAPI codegen in Feature 20, generates Rust structs from GraphQL object types, input types, and enums

## Why GraphQL Support Matters

Shopify is the first GraphQL-only provider, but not the last. GitHub, GitLab, Hasura, Fauna, and others use GraphQL as their primary API. Building generic GraphQL infrastructure now means future providers slot in trivially.

## Shopify Spec Details

| Property | Value |
|----------|-------|
| API | GraphQL Admin API |
| Introspection URL | `https://shopify.dev/admin-graphql-direct-proxy/{version}` |
| Format | GraphQL introspection JSON (standard `__schema` response) |
| Auth Required | No (public proxy) |
| Versioned | Yes — quarterly releases (e.g. `2025-04`, `2025-07`, `2025-10`, `2026-01`) |
| Storefront API | `https://shopify.dev/storefront-graphql-direct-proxy/{version}` (separate, not in scope) |
| Notes | The proxy only responds to proper GraphQL POST requests with `Content-Type: application/json`, not browser GET requests |

### Version Strategy

Shopify releases API versions quarterly. The fetcher should:
- Accept an optional `--shopify-version` CLI argument (e.g. `2025-04`)
- Default to the latest known stable version (hardcoded constant, updated periodically)
- Fetch only one version per run (not all historical versions)

## Architecture

### Directory Structure

```
backends/foundation_deployment/src/providers/
├── shopify/
│   ├── mod.rs           # pub mod fetch; pub mod resources;
│   ├── fetch.rs         # Shopify-specific fetcher (introspection query)
│   └── resources/
│       └── mod.rs       # Auto-generated resource types from GraphQL schema
├── graphql.rs           # Shared GraphQL schema extraction utilities
├── openapi.rs           # Existing shared OpenAPI extraction utilities
└── standard/
    └── fetch.rs         # Existing generic HTTP fetch (OpenAPI providers)
```

Fetched schemas are stored in:
```
artefacts/cloud_providers/shopify/
├── schema.json          # Full introspection result (JSON)
└── _manifest.json       # Fetch metadata with spec_files list
```

### How GraphQL Introspection Works Over Plain HTTP

GraphQL introspection is a standard POST request — no special client library needed. The `SimpleHttpClient` or `curl` sends a JSON body containing the introspection query, and the server returns the full schema as JSON.

```
POST https://shopify.dev/admin-graphql-direct-proxy/2025-04
Content-Type: application/json

{
  "query": "{ __schema { queryType { name } mutationType { name } subscriptionType { name } types { ...FullType } directives { name description locations args { ...InputValue } } } } fragment FullType on __Type { kind name description fields(includeDeprecated: true) { name description args { ...InputValue } type { ...TypeRef } isDeprecated deprecationReason } inputFields { ...InputValue } interfaces { ...TypeRef } enumValues(includeDeprecated: true) { name description isDeprecated deprecationReason } possibleTypes { ...TypeRef } } fragment InputValue on __InputValue { name description type { ...TypeRef } defaultValue } fragment TypeRef on __Type { kind name ofType { kind name ofType { kind name ofType { kind name ofType { kind name ofType { kind name ofType { kind name ofType { kind name } } } } } } } }"
}
```

The response is standard JSON:

```json
{
  "data": {
    "__schema": {
      "queryType": { "name": "QueryRoot" },
      "mutationType": { "name": "Mutation" },
      "types": [
        {
          "kind": "OBJECT",
          "name": "Product",
          "fields": [
            { "name": "id", "type": { "kind": "NON_NULL", "ofType": { "kind": "SCALAR", "name": "ID" } } },
            { "name": "title", "type": { "kind": "NON_NULL", "ofType": { "kind": "SCALAR", "name": "String" } } },
            { "name": "description", "type": { "kind": "SCALAR", "name": "String" } }
          ]
        }
      ]
    }
  }
}
```

This is JSON all the way — no GraphQL client library needed.

## Requirements

### 1. GraphQL Introspection Fetcher

A new `graphql_fetch` module in `standard/` that performs a generic GraphQL introspection query via `curl` (same pattern as `standard::fetch` for OpenAPI).

```rust
// backends/foundation_deployment/src/providers/standard/graphql_fetch.rs

use crate::error::DeploymentError;
use foundation_core::valtron::{execute, from_future, StreamIterator, StreamIteratorExt};
use std::path::PathBuf;

/// The standard GraphQL introspection query.
/// This is the same query used by tools like graphql-codegen, Apollo, etc.
/// It retrieves the full schema including types, fields, arguments, enums,
/// interfaces, unions, input types, and directives.
pub const INTROSPECTION_QUERY: &str = r#"{"query":"{ __schema { queryType { name } mutationType { name } subscriptionType { name } types { ...FullType } directives { name description locations args { ...InputValue } } } } fragment FullType on __Type { kind name description fields(includeDeprecated: true) { name description args { ...InputValue } type { ...TypeRef } isDeprecated deprecationReason } inputFields { ...InputValue } interfaces { ...TypeRef } enumValues(includeDeprecated: true) { name description isDeprecated deprecationReason } possibleTypes { ...TypeRef } } fragment InputValue on __InputValue { name description type { ...TypeRef } defaultValue } fragment TypeRef on __Type { kind name ofType { kind name ofType { kind name ofType { kind name ofType { kind name ofType { kind name ofType { kind name ofType { kind name } } } } } } } }"}"#;

/// Fetch a GraphQL schema via introspection query.
///
/// Sends a POST request with the standard introspection query and saves
/// the full JSON response as `schema.json` + `_manifest.json`.
///
/// # Arguments
///
/// * `provider` - Human-readable provider name
/// * `url` - GraphQL endpoint URL
/// * `output_dir` - Directory to write `schema.json` and `_manifest.json`
pub fn fetch_graphql_schema(
    provider: &str,
    url: &str,
    output_dir: PathBuf,
) -> Result<
    impl StreamIterator<D = Result<PathBuf, DeploymentError>, P = ()> + Send + 'static,
    DeploymentError,
> {
    let provider = provider.to_string();
    let url = url.to_string();

    let future = async move {
        tracing::info!("Fetching {provider} GraphQL schema via introspection from {url}");

        // Use curl to POST the introspection query.
        // -X POST: HTTP POST method
        // -H Content-Type: Required for GraphQL endpoints
        // -d: The introspection query body (pre-serialized JSON)
        let output = std::process::Command::new("curl")
            .args([
                "-sL",
                "-X", "POST",
                "-H", "Content-Type: application/json",
                "-d", INTROSPECTION_QUERY,
                &url,
            ])
            .output()
            .map_err(|e| {
                DeploymentError::Generic(format!("{provider}: curl failed to execute: {e}"))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(DeploymentError::Generic(format!(
                "{provider}: curl returned non-zero exit code: {stderr}"
            )));
        }

        let body = String::from_utf8(output.stdout).map_err(|e| {
            DeploymentError::Generic(format!("{provider}: response is not valid UTF-8: {e}"))
        })?;

        // Validate JSON and check for GraphQL errors
        let json_value: serde_json::Value = serde_json::from_str(&body).map_err(|e| {
            DeploymentError::Generic(format!("{provider}: response is not valid JSON: {e}"))
        })?;

        // GraphQL errors are returned in the response body, not HTTP status
        if let Some(errors) = json_value.get("errors") {
            return Err(DeploymentError::Generic(format!(
                "{provider}: GraphQL introspection returned errors: {errors}"
            )));
        }

        // Verify __schema is present in the response
        if json_value.get("data").and_then(|d| d.get("__schema")).is_none() {
            return Err(DeploymentError::Generic(format!(
                "{provider}: introspection response missing data.__schema"
            )));
        }

        // Create output directory
        std::fs::create_dir_all(&output_dir).map_err(|e| {
            DeploymentError::Generic(format!(
                "{provider}: failed to create output directory {}: {e}",
                output_dir.display()
            ))
        })?;

        // Write schema.json (the full introspection result)
        let output_path = output_dir.join("schema.json");
        let pretty = serde_json::to_string_pretty(&json_value).map_err(|e| {
            DeploymentError::Generic(format!("{provider}: failed to serialize JSON: {e}"))
        })?;

        std::fs::write(&output_path, &pretty).map_err(|e| {
            DeploymentError::Generic(format!(
                "{provider}: failed to write {}: {e}",
                output_path.display()
            ))
        })?;

        // Write _manifest.json
        let manifest = serde_json::json!({
            "provider": provider,
            "source": url,
            "format": "graphql-introspection",
            "fetched_at": chrono::Utc::now().to_rfc3339(),
            "spec_files": ["schema.json"],
        });

        let manifest_path = output_dir.join("_manifest.json");
        std::fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&manifest)?,
        )
        .map_err(|e| {
            DeploymentError::Generic(format!(
                "{provider}: failed to write manifest: {e}"
            ))
        })?;

        tracing::info!(
            "{provider} schema saved to {} ({} bytes)",
            output_path.display(),
            pretty.len()
        );

        Ok(output_path)
    };

    let task = from_future(future);

    let stream = execute(task, None)
        .map_err(|e| DeploymentError::Generic(format!("Valtron scheduling failed: {e}")))?;

    Ok(stream.map_pending(|_| ()))
}
```

### 2. Shopify-Specific Fetcher

```rust
// backends/foundation_deployment/src/providers/shopify/fetch.rs

use crate::error::DeploymentError;
use crate::providers::graphql::{self, ProcessedGraphqlSchema};
use crate::providers::standard;
use foundation_core::valtron::StreamIterator;
use std::path::PathBuf;

/// Latest known stable Shopify API version.
/// Updated periodically as Shopify releases new quarterly versions.
pub const DEFAULT_API_VERSION: &str = "2025-04";

/// Shopify Admin API introspection proxy URL template.
/// Replace `{version}` with the API version (e.g. "2025-04").
pub const INTROSPECTION_URL_TEMPLATE: &str =
    "https://shopify.dev/admin-graphql-direct-proxy/{version}";

pub const PROVIDER_NAME: &str = "shopify";

/// Build the introspection URL for a given API version.
pub fn introspection_url(version: &str) -> String {
    INTROSPECTION_URL_TEMPLATE.replace("{version}", version)
}

/// Fetch the Shopify Admin API GraphQL schema.
///
/// Uses the public introspection proxy which requires no authentication.
/// Defaults to `DEFAULT_API_VERSION` if no version is specified.
pub fn fetch_shopify_specs(
    output_dir: PathBuf,
    api_version: Option<&str>,
) -> Result<
    impl StreamIterator<D = Result<PathBuf, DeploymentError>, P = ()> + Send + 'static,
    DeploymentError,
> {
    let version = api_version.unwrap_or(DEFAULT_API_VERSION);
    let url = introspection_url(version);

    standard::graphql_fetch::fetch_graphql_schema(PROVIDER_NAME, &url, output_dir)
}

/// Process a fetched Shopify introspection result.
pub fn process_schema(schema: &serde_json::Value) -> ProcessedGraphqlSchema {
    graphql::process_schema(schema)
}
```

### 3. Shared GraphQL Schema Processing (`graphql.rs`)

Parallel to `openapi.rs`, this module extracts structured metadata from a GraphQL introspection JSON response.

```rust
// backends/foundation_deployment/src/providers/graphql.rs

use serde::{Deserialize, Serialize};

/// A GraphQL type extracted from an introspection result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphqlType {
    /// Type name (e.g. "Product", "OrderConnection").
    pub name: String,
    /// GraphQL type kind: OBJECT, INPUT_OBJECT, ENUM, SCALAR, INTERFACE, UNION.
    pub kind: String,
    /// Human-readable description from the schema.
    pub description: Option<String>,
    /// Fields (for OBJECT and INPUT_OBJECT kinds).
    pub fields: Option<Vec<GraphqlField>>,
    /// Enum values (for ENUM kind).
    pub enum_values: Option<Vec<GraphqlEnumValue>>,
    /// Interfaces this type implements (for OBJECT kind).
    pub interfaces: Option<Vec<String>>,
    /// Possible concrete types (for INTERFACE and UNION kinds).
    pub possible_types: Option<Vec<String>>,
}

/// A field on a GraphQL object or input type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphqlField {
    /// Field name (e.g. "title", "createdAt").
    pub name: String,
    /// Human-readable description.
    pub description: Option<String>,
    /// Resolved type string (e.g. "String", "String!", "[Product!]!").
    pub type_ref: String,
    /// Whether this field is non-null (required).
    pub required: bool,
    /// Whether this field is a list type.
    pub is_list: bool,
    /// Whether this field is deprecated.
    pub is_deprecated: bool,
    /// Deprecation reason if deprecated.
    pub deprecation_reason: Option<String>,
    /// Arguments (for query/mutation fields).
    pub args: Option<Vec<GraphqlArgument>>,
}

/// An argument on a GraphQL field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphqlArgument {
    pub name: String,
    pub description: Option<String>,
    pub type_ref: String,
    pub required: bool,
    pub default_value: Option<String>,
}

/// An enum value in a GraphQL enum type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphqlEnumValue {
    pub name: String,
    pub description: Option<String>,
    pub is_deprecated: bool,
    pub deprecation_reason: Option<String>,
}

/// A query or mutation operation extracted from the root types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphqlOperation {
    /// Operation name (e.g. "products", "orderCreate").
    pub name: String,
    /// "query" or "mutation".
    pub operation_type: String,
    /// Human-readable description.
    pub description: Option<String>,
    /// Return type string.
    pub return_type: String,
    /// Arguments.
    pub args: Vec<GraphqlArgument>,
}

/// Processed result from a GraphQL introspection response.
#[derive(Debug, Clone)]
pub struct ProcessedGraphqlSchema {
    /// All non-introspection types (excludes __Type, __Field, etc.).
    pub types: Vec<GraphqlType>,
    /// Query operations (fields on the query root type).
    pub queries: Vec<GraphqlOperation>,
    /// Mutation operations (fields on the mutation root type).
    pub mutations: Vec<GraphqlOperation>,
    /// Content hash for change detection.
    pub content_hash: String,
}

/// Resolve a GraphQL type reference JSON into a human-readable type string.
///
/// Handles nested NON_NULL and LIST wrappers:
/// - `{ kind: "NON_NULL", ofType: { kind: "SCALAR", name: "String" } }` → `"String!"`
/// - `{ kind: "LIST", ofType: { kind: "NON_NULL", ofType: { kind: "OBJECT", name: "Product" } } }` → `"[Product!]"`
/// - `{ kind: "NON_NULL", ofType: { kind: "LIST", ... } }` → `"[Product!]!"`
pub fn resolve_type_ref(type_ref: &serde_json::Value) -> String {
    // Walk the type reference tree, building the string representation
    // NON_NULL wraps with !, LIST wraps with []
    // Recurse into ofType until we hit a named type (SCALAR, OBJECT, ENUM, etc.)
    todo!()
}

/// Check if a type reference is non-null at the outermost level.
pub fn is_non_null(type_ref: &serde_json::Value) -> bool {
    type_ref.get("kind").and_then(|k| k.as_str()) == Some("NON_NULL")
}

/// Check if a type reference contains a list anywhere in the wrapper chain.
pub fn is_list_type(type_ref: &serde_json::Value) -> bool {
    // Walk the ofType chain looking for LIST
    todo!()
}

/// Extract all user-defined types from an introspection result.
///
/// Filters out:
/// - GraphQL introspection types (names starting with `__`)
/// - Built-in scalars (String, Int, Float, Boolean, ID)
pub fn extract_types(schema: &serde_json::Value) -> Vec<GraphqlType> {
    todo!()
}

/// Extract query operations from the schema's query root type.
pub fn extract_queries(schema: &serde_json::Value) -> Vec<GraphqlOperation> {
    todo!()
}

/// Extract mutation operations from the schema's mutation root type.
pub fn extract_mutations(schema: &serde_json::Value) -> Vec<GraphqlOperation> {
    todo!()
}

/// Process a full introspection JSON response into structured data.
///
/// Entry point parallel to `openapi::process_spec()`.
pub fn process_schema(schema: &serde_json::Value) -> ProcessedGraphqlSchema {
    let inner = schema
        .get("data")
        .and_then(|d| d.get("__schema"))
        .unwrap_or(schema);

    let types = extract_types(inner);
    let queries = extract_queries(inner);
    let mutations = extract_mutations(inner);

    let content_hash = {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        schema.to_string().hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    };

    ProcessedGraphqlSchema {
        types,
        queries,
        mutations,
        content_hash,
    }
}
```

### 4. GraphQL-to-Rust Type Generation

This extends Feature 20's code generator to handle GraphQL schemas alongside OpenAPI specs.

#### Type Mapping: GraphQL Scalars to Rust Types

| GraphQL Scalar | Rust Type |
|---------------|-----------|
| `String` | `String` |
| `Int` | `i64` |
| `Float` | `f64` |
| `Boolean` | `bool` |
| `ID` | `String` |
| `DateTime` | `String` (ISO 8601) |
| `JSON` | `serde_json::Value` |
| `Decimal` | `String` (precision-safe) |
| `URL` | `String` |
| `HTML` | `String` |
| `Money` | `String` |
| `UnsignedInt64` | `u64` |
| Custom scalars | `serde_json::Value` (safe fallback) |

#### Type Mapping: GraphQL Wrappers to Rust

| GraphQL Type | Rust Type |
|-------------|-----------|
| `String!` (non-null) | `String` |
| `String` (nullable) | `Option<String>` |
| `[Product!]!` (non-null list of non-null) | `Vec<Product>` |
| `[Product!]` (nullable list of non-null) | `Option<Vec<Product>>` |
| `[Product]!` (non-null list of nullable) | `Vec<Option<Product>>` |
| `[Product]` (nullable list of nullable) | `Option<Vec<Option<Product>>>` |

#### Generated Code Patterns

**GraphQL OBJECT type:**

```rust
// Generated from GraphQL type "Product"
/// Represents a product in the store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Product {
    /// Globally unique identifier.
    pub id: String,
    /// The title of the product.
    pub title: String,
    /// The description of the product, complete with HTML formatting.
    #[serde(default)]
    pub description_html: Option<String>,
    /// A human-friendly unique string for the product.
    pub handle: String,
    /// The date and time when the product was created.
    #[serde(default, rename = "createdAt")]
    pub created_at: Option<String>,
}
```

**GraphQL ENUM type:**

```rust
// Generated from GraphQL enum "ProductStatus"
/// The possible product statuses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProductStatus {
    /// The product is ready to sell and is available to customers.
    #[serde(rename = "ACTIVE")]
    Active,
    /// The product is no longer being sold and is not available to customers.
    #[serde(rename = "ARCHIVED")]
    Archived,
    /// The product is not yet ready to sell and is not available to customers.
    #[serde(rename = "DRAFT")]
    Draft,
}
```

**GraphQL INPUT_OBJECT type:**

```rust
// Generated from GraphQL input "ProductInput"
/// Specifies the input fields required to create or update a product.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductInput {
    /// The title of the product.
    #[serde(default)]
    pub title: Option<String>,
    /// The description of the product, complete with HTML formatting.
    #[serde(default, rename = "descriptionHtml")]
    pub description_html: Option<String>,
    /// The product status.
    #[serde(default)]
    pub status: Option<ProductStatus>,
}
```

**GraphQL INTERFACE/UNION types:**

Interfaces and unions are represented as Rust enums with `#[serde(untagged)]`:

```rust
// Generated from GraphQL interface "Node"
/// An object with an ID.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Node {
    Product(Product),
    Order(Order),
    Customer(Customer),
    // ... all implementing types
}
```

#### Field Naming: camelCase to snake_case

GraphQL fields use camelCase. The generator converts to snake_case and adds `#[serde(rename = "...")]`:

| GraphQL Field | Rust Field | Serde Annotation |
|--------------|-----------|-----------------|
| `createdAt` | `created_at` | `#[serde(rename = "createdAt")]` |
| `descriptionHtml` | `description_html` | `#[serde(rename = "descriptionHtml")]` |
| `id` | `id` | (none, same in both) |
| `type` | `type_` | `#[serde(rename = "type")]` |

#### Connection Types (Shopify Relay Pattern)

Shopify uses the Relay connection pattern for pagination. The generator should recognize and generate connection types:

```rust
// ProductConnection, ProductEdge, PageInfo are standard Relay types
/// An auto-generated type for paginating through multiple Products.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductConnection {
    /// A list of edges.
    #[serde(default)]
    pub edges: Option<Vec<ProductEdge>>,
    /// Information to aid in pagination.
    #[serde(default, rename = "pageInfo")]
    pub page_info: Option<PageInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductEdge {
    /// A cursor for use in pagination.
    pub cursor: String,
    /// The item at the end of the edge.
    #[serde(default)]
    pub node: Option<Product>,
}
```

### 5. Orchestrator Integration

The `ProviderSpecFetcher` in `bin/platform` needs updates to:

1. Add Shopify to `configured_providers()` — note this uses a URL template, not a static URL
2. Add `"shopify"` case in `create_provider_stream()` — calls `shopify::fetch::fetch_shopify_specs()`
3. Update `enrich_spec()` to detect GraphQL schemas (check for `data.__schema` in JSON) and call `graphql::process_schema()` instead of `openapi::process_spec()`
4. Add `--shopify-version` CLI argument to `gen_provider_specs` subcommand
5. Pass the version through to the Shopify fetcher

```rust
// In fetcher.rs configured_providers():
(shopify::fetch::PROVIDER_NAME, "https://shopify.dev/admin-graphql-direct-proxy"),

// In fetcher.rs create_provider_stream():
"shopify" => {
    let stream = shopify::fetch::fetch_shopify_specs(output_dir, None)
        .map_err(|e| SpecFetchError::Generic(format!("{provider_name}: {e}")))?;
    Ok(Box::new(stream.map_done(move |r| {
        r.map_err(|e| SpecFetchError::Generic(format!("{provider_name}: {e}")))
    })))
}

// In fetcher.rs enrich_spec(): detect format from JSON structure
fn enrich_spec(provider: &str, spec_path: &std::path::Path, spec: &mut DistilledSpec) {
    let Ok(content) = std::fs::read_to_string(spec_path) else { return; };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) else { return; };

    // Detect schema format
    if json.get("data").and_then(|d| d.get("__schema")).is_some() {
        // GraphQL introspection result
        let processed = graphql::process_schema(&json);
        spec.content_hash = processed.content_hash;
        // GraphQL schemas don't have a version field like OpenAPI's info.version
        // The version is in the URL, already captured in source_url
    } else {
        // OpenAPI spec
        let processed = openapi::process_spec(&json);
        // ... existing logic
    }
}
```

### 6. DistilledSpec Format Field

`DistilledSpec` in `core.rs` needs a `format` field to distinguish OpenAPI from GraphQL specs:

```rust
pub struct DistilledSpec {
    pub provider: String,
    pub version: String,
    pub fetched_at: chrono::DateTime<chrono::Utc>,
    pub source_url: String,
    pub raw_spec: serde_json::Value,
    pub endpoints: Option<Vec<SpecEndpoint>>,
    pub content_hash: String,
    pub spec_files: Vec<String>,
    /// Spec format: "openapi" or "graphql-introspection"
    pub format: String,
}
```

## Error Handling

Errors use `DeploymentError` from `foundation_deployment::error`. GraphQL-specific errors to handle:

- **Introspection disabled**: Some endpoints return `{ "errors": [{ "message": "introspection is disabled" }] }` — detect and report clearly
- **Version not found**: Shopify returns 404 for invalid versions — detect and suggest valid versions
- **Malformed response**: Missing `data.__schema` — validate structure before processing

## Tasks

1. **Create GraphQL introspection fetcher**
   - [ ] Create `backends/foundation_deployment/src/providers/standard/graphql_fetch.rs`
   - [ ] Implement `INTROSPECTION_QUERY` constant
   - [ ] Implement `fetch_graphql_schema()` using curl POST
   - [ ] Validate response structure (`data.__schema` present, no `errors`)
   - [ ] Write `schema.json` and `_manifest.json` with `format: "graphql-introspection"`
   - [ ] Register `pub mod graphql_fetch;` in `standard/mod.rs`

2. **Create Shopify provider module**
   - [ ] Create `backends/foundation_deployment/src/providers/shopify/mod.rs`
   - [ ] Create `backends/foundation_deployment/src/providers/shopify/fetch.rs`
   - [ ] Implement `fetch_shopify_specs()` with version parameter
   - [ ] Implement `introspection_url()` for building versioned URLs
   - [ ] Create `backends/foundation_deployment/src/providers/shopify/resources/mod.rs`
   - [ ] Register `pub mod shopify;` in `providers/mod.rs`

3. **Create GraphQL schema processing utilities**
   - [ ] Create `backends/foundation_deployment/src/providers/graphql.rs`
   - [ ] Implement `resolve_type_ref()` — walk NON_NULL/LIST wrappers
   - [ ] Implement `extract_types()` — filter introspection/built-in types
   - [ ] Implement `extract_queries()` and `extract_mutations()`
   - [ ] Implement `process_schema()` entry point
   - [ ] Register `pub mod graphql;` in `providers/mod.rs`
   - [ ] Write unit tests for type resolution, extraction, filtering

4. **Wire into orchestrator**
   - [ ] Add Shopify to `configured_providers()` in `bin/platform/src/gen_provider_specs/fetcher.rs`
   - [ ] Add `"shopify"` case in `create_provider_stream()`
   - [ ] Update `enrich_spec()` to detect and process GraphQL schemas
   - [ ] Add `format` field to `DistilledSpec` in `core.rs`
   - [ ] Add `--shopify-version` CLI argument
   - [ ] Pass version through to Shopify fetcher

5. **Write unit tests for GraphQL fetcher**
   - [ ] Test introspection query is valid JSON
   - [ ] Test URL builder with different versions
   - [ ] Test response validation (missing `__schema`, GraphQL errors)
   - [ ] Test constants are correct

6. **Write unit tests for GraphQL schema processing**
   - [ ] Test `resolve_type_ref()` with nested NON_NULL/LIST wrappers
   - [ ] Test `extract_types()` filters introspection types
   - [ ] Test `extract_queries()` and `extract_mutations()`
   - [ ] Test `process_schema()` with sample Shopify-like schema
   - [ ] Test content hash determinism

7. **Integration test**
   - [ ] Verify `cargo run -- gen_provider_specs --provider shopify` fetches schema
   - [ ] Verify schema is written to `artefacts/cloud_providers/shopify/schema.json`
   - [ ] Verify manifest includes `format: "graphql-introspection"`

8. **Design GraphQL type generation (extends Feature 20)**
   - [ ] Define GraphQL scalar to Rust type mapping
   - [ ] Define nullable/non-null/list wrapper rules
   - [ ] Define camelCase to snake_case field conversion with serde rename
   - [ ] Define ENUM generation with serde rename per variant
   - [ ] Define INPUT_OBJECT generation (all fields optional by default)
   - [ ] Define INTERFACE/UNION generation strategy (`#[serde(untagged)]`)
   - [ ] Define Connection type recognition (Relay pattern)
   - [ ] Document in Feature 20 as a parallel generation path

9. **Implement GraphQL type generation**
   - [ ] Implement GraphQL scalar mapping function
   - [ ] Implement `resolve_type_ref()` to Rust type string
   - [ ] Implement OBJECT struct generation
   - [ ] Implement ENUM generation with serde variants
   - [ ] Implement INPUT_OBJECT generation
   - [ ] Implement INTERFACE/UNION enum generation
   - [ ] Run `rustfmt` on generated files
   - [ ] Verify generated code compiles with zero warnings

10. **Handle edge cases**
    - [ ] Shopify's custom scalars (DateTime, Money, HTML, URL, etc.)
    - [ ] Recursive types (e.g. `MenuItemChildrenConnection` references `MenuItem`)
    - [ ] Very large schema (~4000+ types in Shopify Admin API)
    - [ ] Deprecated fields (include but mark with `#[deprecated]`)
    - [ ] Connection/Edge/PageInfo deduplication (generate once, reuse)

## Success Criteria

- [ ] All 10 tasks completed
- [ ] Zero warnings in `cargo clippy -p foundation_deployment`
- [ ] Shopify schema fetches successfully via `gen_provider_specs --provider shopify`
- [ ] Schema stored in `artefacts/cloud_providers/shopify/schema.json`
- [ ] `_manifest.json` includes `format: "graphql-introspection"` and `spec_files`
- [ ] `graphql.rs` processes schemas correctly (types, queries, mutations extracted)
- [ ] Generated Rust types compile with zero warnings
- [ ] GraphQL infrastructure is generic (can be reused for GitHub, GitLab, etc.)

## Verification

```bash
# Fetch Shopify schema
cargo run -- gen_provider_specs --provider shopify

# Fetch specific version
cargo run -- gen_provider_specs --provider shopify --shopify-version 2025-04

# Verify artefacts
ls artefacts/cloud_providers/shopify/schema.json
cat artefacts/cloud_providers/shopify/_manifest.json

# Run tests
cargo test -p foundation_deployment -- providers::shopify
cargo test -p foundation_deployment -- providers::graphql

# Generate resource types (once generation is implemented)
cargo run -- gen_resource_types --provider shopify
cargo check -p foundation_deployment
```

## Future: Other GraphQL Providers

The GraphQL infrastructure built here is generic. Future providers can be added with minimal effort:

| Provider | Introspection URL | Auth |
|----------|------------------|------|
| GitHub | `https://api.github.com/graphql` | Yes (PAT required) |
| GitLab | `https://gitlab.com/api/graphql` | Yes (PAT required) |
| Hasura | Instance-specific | Yes |
| Fauna | `https://graphql.fauna.com/graphql` | Yes |

Each would need only a provider-specific `fetch.rs` — the `graphql_fetch`, `graphql.rs`, and type generation are reusable.

---

_Created: 2026-04-05_
