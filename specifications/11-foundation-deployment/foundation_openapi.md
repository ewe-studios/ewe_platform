# Foundation OpenAPI Crate - Feature Specification

```yaml
---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/foundation_openapi"
this_file: "specifications/11-foundation-deployment/foundation_openapi.md"

status: pending
priority: high
created: 2026-04-09

depends_on: []

tasks:
  completed: 0
  uncompleted: 0
  total: 0
  completion_percentage: 0%
---
```

## Overview

**Goal:** Create a centralized `foundation_openapi` crate that serves as the canonical library for processing OpenAPI specifications across the codebase. This crate will handle all OpenAPI/Discovery document parsing, endpoint extraction, type resolution, and normalized output generation.

**Location:** `backends/foundation_openapi/`

**Consumers:**
- `bin/platform` (code generators: `gen_resources/types.rs`, `gen_resources/clients.rs`, `gen_resources/provider_specs.rs`)
- `foundation_deployment` (runtime API clients)
- Any future crates needing OpenAPI processing

---

## Motivation & Lessons Learned

### Problems with Current Approach

1. **Duplicated Logic Across Generators**

   Both `gen_resources/types.rs` and `gen_resources/clients.rs` maintain separate, overlapping implementations for:
   - OpenAPI spec deserialization structures (`OpenApiSpec`, `Schema`, `Operation`, etc.)
   - Endpoint extraction from paths and resources
   - `$ref` resolution logic
   - Composition type handling (allOf/oneOf/anyOf)
   - GCP Discovery vs standard OpenAPI 3.x format detection
   - Type name normalization (PascalCase, snake_case conversions)

   **Example:** The `deserialize_type_field` function appears in both files with identical logic for handling `"type": ["string", "null"]`.

2. **Inconsistent Type Resolution**

   The criteria for what makes a type "generatable" varies between generators:

   ```rust
   // types.rs - includes composition types always
   let has_composition = value.get("allOf").is_some()
       || value.get("oneOf").is_some()
       || value.get("anyOf").is_some();

   // clients.rs - different logic for union types
   let is_union_type = (schema.any_of.is_some() || schema.one_of.is_some())
       && schema.properties.is_none();
   // → Use serde_json::Value for union types
   ```

   This leads to inconsistent handling where the same schema might be treated differently by each generator.

3. **Multiple Spec Formats Require Different Handling**

   | Format | Paths/Resources | Schemas Location | Base URL |
   |--------|-----------------|-----------------|----------|
   | OpenAPI 3.x | `paths` | `components/schemas` | `servers[0].url` |
   | GCP Discovery | `resources[].methods[]` | top-level `schemas` | `baseUrl` or `rootUrl` + `servicePath` |
   | Consolidated | varies per API | varies per API | varies per API |

   Each generator must handle all three formats, leading to complex conditional logic.

4. **Fragile Endpoint-to-Type Mapping**

   The relationship between:
   - Operation ID → function name
   - Response type → `ResourceIdentifier` implementation
   - Path parameters → `parameterOrder` (GCP) vs `parameters[].in: "path"` (OpenAPI)

   ...is computed separately in each generator with subtle differences.

5. **No Normalized Representation for Introspection**

   Each consumer re-parses the raw OpenAPI JSON. There is no:
   - Single pass to understand the entire API surface
   - Quick lookup format for "what are all endpoints and their types?"
   - Easy way to diff between spec versions
   - Debugging-friendly representation

### Key Lessons Learned from Existing Implementation

1. **Composition Types Should Use `serde_json::Value`**
   
   Complex union types (`oneOf`/`anyOf` without properties) are not worth generating structs for. Use `serde_json::Value` and document this clearly.

2. **`allOf` with Single `$ref` is a Wrapper**

   When `allOf` contains exactly one `$ref`, treat it as the referenced type, not a composition:
   ```rust
   // allOf with a single $ref: use the referenced type
   if all_of.len() == 1 {
       if let Some(ref_path) = &all_of[0].ref_path {
           return self.resolve_ref(ref_path, object_schemas);
       }
   }
   "serde_json::Value".to_string()
   ```

3. **GCP Discovery Specifics**

   - `flatPath` is the actual URL pattern; `path` is a template like `"v1/{+name}"`
   - `parameterOrder` gives path parameters in the correct order
   - Parameters are in a map, not an array
   - Nested resources require recursive traversal
   - Response extraction uses direct `$ref` from method response object

4. **Type Name Normalization is Critical**

   Input types come in various naming conventions:
   - `treasury.transaction` (dot-separated)
   - `Custom-pages` (hyphenated)
   - `iam_response_collection` (snake_case)
   - `GoogleCloudRunV2Service` (already PascalCase)
   - `@cf_ai4bharat.translation` (prefixed with `@`)

   All must normalize to valid Rust PascalCase identifiers.

5. **Some "Empty" Types Have Semantic Meaning**

   Types like `GoogleCloudAiplatformV1ContentMap` with a single `additionalProperties` field are semantically meaningful and should NOT be filtered out.

6. **Response/Request Type Detection**

   Types containing "response" or "request" in their names should always be included:
   ```rust
   let is_response_wrapper = schema_name.to_lowercase().contains("api_response") 
       || schema_name.to_lowercase().contains("response");
   let is_request_type = schema_name.to_lowercase().contains("request");
   ```

7. **Rust Keyword Conflicts**

   Types that conflict with Rust built-ins must be renamed:
   - `Option` → `ApiOption`
   - `Value` → `ApiValue`
   - `Result` → `ApiResult`
   - `Ok` → `ApiOk`
   - `Err` → `ApiErr`
   - `Some` → `ApiSome`
   - `None` → `ApiNone`
   - `Box` → `ApiBox`
   - `Vec` → `ApiVec`
   - `String` → `ApiString`

8. **Duplicate Field Handling**

   Some specs have duplicate fields with different casing. Use snake_case deduplication:
   ```rust
   let mut seen_field_names: HashSet<String> = HashSet::new();
   for (field_name, field_schema) in properties {
       let snake_case_name = self.to_snake_case(&field_name);
       if seen_field_names.contains(&snake_case_name) {
           continue; // Skip duplicate
       }
       seen_field_names.insert(snake_case_name.clone());
   }
   ```

9. **allOf Property Merging**

   When a schema has `allOf`, merge properties from all members:
   ```rust
   let mut properties = schema.properties.unwrap_or_default();
   if let Some(allof) = &schema.all_of {
       for member in allof {
           if let Some(member_props) = &member.properties {
               for (key, value) in member_props {
                   properties.entry(key.clone()).or_insert_with(|| value.clone());
               }
           }
       }
   }
   ```

10. **Object Schema Tracking**

    Track which schemas are known object types for `$ref` validation:
    ```rust
    let object_schemas = self.collect_object_schema_names(&spec);
    // Use during type resolution to validate $ref targets
    ```

---

## Design Principles

1. **Single Source of Truth**
   
   All OpenAPI processing flows through this crate. Consumers import `foundation_openapi` and use its unified API.

2. **Minimal Deserialization**
   
   Only deserialize what's needed for endpoint extraction and type resolution. Use `serde_json::Value` for complex/unknown structures.

3. **Format Agnostic**
   
   Unified API regardless of input format. The crate detects and handles OpenAPI 3.x, GCP Discovery, and consolidated formats transparently.

4. **Explicit Type Resolution**
   
   Clear, documented rules for what constitutes a "generatable" type vs what should be `serde_json::Value`.

5. **Normalized Output**
   
   Produce a simplified JSON representation for quick introspection, debugging, and code generation.

---

## API Design

### Module Structure

```
backends/foundation_openapi/
├── Cargo.toml
└── src/
    ├── lib.rs           # Re-exports and high-level API
    ├── spec.rs          # Minimal OpenAPI structures for deserialization
    ├── endpoint.rs      # EndpointInfo and related types
    ├── normalizer.rs    # SpecProcessor and normalized output
    ├── type_resolver.rs # Type resolution logic and composition handling
    └── extractor.rs     # Endpoint extraction from paths and resources
```

---

### `spec.rs` - Minimal OpenAPI Structures

**WHY:** We only need a subset of OpenAPI for endpoint extraction. Minimal structures reduce memory and parsing overhead.

**WHAT:** Core OpenAPI 3.x and GCP Discovery structures needed for processing.

```rust
use serde::Deserialize;
use std::collections::BTreeMap;

/// Root OpenAPI specification (handles multiple formats)
#[derive(Debug, Deserialize, Clone)]
pub struct OpenApiSpec {
    // Standard OpenAPI 3.x fields
    pub openapi: String,
    pub info: Info,
    #[serde(default)]
    pub servers: Option<Vec<Server>>,
    #[serde(default)]
    pub paths: BTreeMap<String, PathItem>,
    #[serde(default)]
    pub components: Option<Components>,
    
    // GCP Discovery fields
    #[serde(default, rename = "baseUrl")]
    pub base_url: Option<String>,
    #[serde(default, rename = "rootUrl")]
    pub root_url: Option<String>,
    #[serde(default, rename = "servicePath")]
    pub service_path: Option<String>,
    #[serde(default)]
    pub schemas: Option<BTreeMap<String, Schema>>,
    #[serde(default)]
    pub resources: Option<BTreeMap<String, Resource>>,
}

/// Minimal schema structure for type resolution
#[derive(Debug, Deserialize, Clone, Default)]
pub struct Schema {
    #[serde(default, deserialize_with = "deserialize_type_field")]
    #[serde(rename = "type")]
    pub schema_type: Option<String>,
    
    #[serde(default)]
    pub format: Option<String>,
    
    #[serde(default)]
    pub description: Option<String>,
    
    #[serde(default)]
    pub required: Vec<String>,
    
    #[serde(default)]
    pub properties: Option<BTreeMap<String, Schema>>,
    
    #[serde(default)]
    pub items: Option<Box<Schema>>,
    
    #[serde(default, rename = "$ref")]
    pub ref_path: Option<String>,
    
    #[serde(default, rename = "allOf")]
    pub all_of: Option<Vec<Schema>>,
    
    #[serde(default, rename = "oneOf")]
    pub one_of: Option<Vec<Schema>>,
    
    #[serde(default, rename = "anyOf")]
    pub any_of: Option<Vec<Schema>>,
    
    #[serde(default, rename = "enum")]
    pub enum_values: Option<Vec<serde_json::Value>>,
}

/// GCP Discovery resource with nested methods
#[derive(Debug, Deserialize, Clone, Default)]
pub struct Resource {
    pub methods: Option<BTreeMap<String, GcpMethod>>,
    pub resources: Option<BTreeMap<String, Resource>>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct GcpMethod {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default, rename = "flatPath")]
    pub flat_path: Option<String>,
    #[serde(default)]
    pub http_method: Option<String>,
    #[serde(default)]
    pub parameters: Option<BTreeMap<String, GcpParameter>>,
    #[serde(default, rename = "parameterOrder")]
    pub parameter_order: Option<Vec<String>>,
    #[serde(default, rename = "requestBody")]
    pub request_body: Option<GcpRequestBodyRef>,
    #[serde(default)]
    pub response: Option<GcpResponseRef>,
}
```

**Key Design Notes:**

1. Use `Option` for all fields to handle missing data gracefully
2. Support both standard OpenAPI and GCP Discovery fields in the same struct
3. Custom deserializer for `type` field handles both string and array formats:
   ```json
   "type": "string"          // OpenAPI 3.0
   "type": ["string", "null"] // OpenAPI 3.1 nullable
   ```

---

### `endpoint.rs` - Endpoint Information

**WHY:** We need a unified representation of endpoints with their request/response types for code generation and runtime introspection.

**WHAT:** `EndpointInfo` struct with all metadata needed for client generation.

```rust
use std::collections::BTreeMap;

/// Unified endpoint representation
#[derive(Debug, Clone)]
pub struct EndpointInfo {
    /// Operation ID (e.g., "getV1ComputeServices")
    pub operation_id: String,
    
    /// HTTP method (GET, POST, PUT, PATCH, DELETE)
    pub method: String,
    
    /// Path template (e.g., "/v1/compute-services/{id}")
    pub path: String,
    
    /// Path parameter names in order
    /// e.g., ["projectId", "databaseId"]
    pub path_params: Vec<String>,
    
    /// Query parameter names
    pub query_params: Vec<String>,
    
    /// Request body type name (if any)
    pub request_type: Option<String>,
    
    /// Response type discriminator
    pub response_type: Option<ResponseType>,
    
    /// Error response types by status code
    /// e.g., {"401": "ApiError", "404": "NotFoundError"}
    pub error_types: BTreeMap<String, String>,
    
    /// Success status codes (e.g., ["200", "201"])
    pub success_codes: Vec<String>,
    
    /// Base URL for this endpoint
    pub base_url: Option<String>,
    
    /// Summary/description
    pub summary: Option<String>,
}

/// Response type discriminator
#[derive(Debug, Clone)]
pub enum ResponseType {
    /// Named type to generate (e.g., "GetProjectResponse")
    Generated(String),
    
    /// Composition type - use serde_json::Value
    JsonValue,
    
    /// 204 No Content - use ()
    NoContent,
}

impl ResponseType {
    /// Get the Rust type string for this response type
    pub fn as_rust_type(&self) -> &str {
        match self {
            ResponseType::Generated(name) => name,
            ResponseType::JsonValue => "serde_json::Value",
            ResponseType::NoContent => "()",
        }
    }
    
    /// Check if this is a generatable type (not JsonValue or NoContent)
    pub fn is_generated(&self) -> bool {
        matches!(self, ResponseType::Generated(_))
    }
}
```

**Key Methods:**

```rust
impl EndpointInfo {
    /// Generate Args struct name from operation_id
    /// "getV1Projects" → "GetV1ProjectsArgs"
    pub fn args_struct_name(&self) -> String;
    
    /// Generate function name from operation_id  
    /// "getV1Projects" → "get_v1_projects"
    pub fn fn_name(&self) -> String;
    
    /// Extract path parameters from path template
    /// "/v1/projects/{projectId}" → ["projectId"]
    pub fn extract_path_params(path: &str) -> Vec<String>;
    
    /// Extract parameters by location from OpenAPI operation parameters
    /// Returns (path_params, query_params)
    pub fn extract_parameters(parameters: &[Parameter]) -> (Vec<String>, Vec<String>);
}
```

---

### `type_resolver.rs` - Type Resolution Logic

**WHY:** Clear rules for what constitutes a "generatable" type vs what should be `serde_json::Value`.

**WHAT:** `TypeResolver` struct with schema lookup and resolution logic.

```rust
use std::collections::BTreeMap;

/// Resolver for OpenAPI schema types
pub struct TypeResolver<'a> {
    schemas: &'a BTreeMap<String, Schema>,
}

impl<'a> TypeResolver<'a> {
    /// Create resolver with schema map
    pub fn new(schemas: &'a BTreeMap<String, Schema>) -> Self;
    
    /// Resolve a $ref to a type name
    /// "#/components/schemas/Project" → Some("Project")
    /// "#/schemas/GoogleCloudRunV2Service" → Some("GoogleCloudRunV2Service")
    pub fn resolve_ref(&self, ref_path: &str) -> Option<String>;
    
    /// Check if a schema is "generatable" (has properties or simple structure)
    /// Returns false for composition types (oneOf/anyOf) and empty objects
    pub fn is_generatable(&self, schema: &Schema) -> bool;
    
    /// Get the response type for a response object
    /// Handles composition types, $refs, and inline schemas
    pub fn get_response_type(&self, response: &Response) -> Option<ResponseType>;
    
    /// Normalize type name to Rust PascalCase
    /// "treasury.transaction" → "TreasuryTransaction"
    /// "Custom-pages" → "CustomPages"
    /// "@cf_ai4bharat.translation" → "CfAi4bharatTranslation"
    pub fn normalize_type_name(name: &str) -> String;
    
    /// Check if a type name is a Rust keyword/builtin that needs renaming
    pub fn rename_if_keyword(name: &str) -> String;
}
```

**Type Resolution Rules:**

| Schema Structure | Resolution | Rationale |
|-----------------|------------|-----------|
| Has `properties` | `Generated(TypeName)` | Struct with fields is generatable |
| `allOf` with single `$ref` | Resolve to referenced type | Wrapper pattern, not composition |
| `allOf` with multiple refs | `JsonValue` | Complex composition |
| `oneOf`/`anyOf` | `JsonValue` | Union types not generatable |
| `type: "object"` with no properties | `JsonValue` | Empty object or `additionalProperties` only |
| Inline schema (no `$ref`) | `None` | No named type to generate |
| `$ref` to generatable schema | `Generated(ResolvedName)` | Follow reference |
| `$ref` to non-generatable schema | `JsonValue` | Referenced type is not generatable |
| Response type name contains "Response" | Always `Generated` | Response wrappers are always needed |
| Request type name contains "Request" | Always `Generated` | Request wrappers are always needed |

**Implementation Notes:**

1. **`allOf` Single-Ref Optimization:**
   ```rust
   if let Some(all_of) = &schema.all_of {
       if all_of.len() == 1 {
           if let Some(ref_path) = &all_of[0].ref_path {
               return self.resolve_ref(ref_path);
           }
       }
       // Multiple members → JsonValue
   }
   ```

2. **Response Type Priority:**
   When multiple success status codes exist:
   - `200` - OK (preferred)
   - `201` - Created
   - `202` - Accepted  
   - `204` - No Content → `ResponseType::NoContent`

3. **Name Normalization Algorithm:**
   ```rust
   // Split on: dots, hyphens, underscores, camelCase boundaries
   // - Remove leading @ or other prefixes
   // - PascalCase each part
   // - Join together
   ```

4. **Rust Keyword Renaming:**
   ```rust
   match name.as_str() {
       "Option" => "ApiOption",
       "Value" => "ApiValue",
       "Result" => "ApiResult",
       "Ok" => "ApiOk",
       "Err" => "ApiErr",
       "Some" => "ApiSome",
       "None" => "ApiNone",
       "Box" => "ApiBox",
       "Vec" => "ApiVec",
       "String" => "ApiString",
       _ => name,
   }
   ```

---

### `extractor.rs` - Endpoint Extraction

**WHY:** Extract endpoint metadata from both standard OpenAPI paths and GCP Discovery resources.

**WHAT:** `EndpointExtractor` with format-aware extraction logic.

```rust
pub struct EndpointExtractor<'a> {
    spec: &'a OpenApiSpec,
    resolver: TypeResolver<'a>,
}

impl<'a> EndpointExtractor<'a> {
    pub fn new(spec: &'a OpenApiSpec) -> Self;
    
    /// Extract all endpoints from the spec
    /// Handles both OpenAPI 3.x and GCP Discovery formats
    pub fn extract_all(&self) -> Vec<EndpointInfo>;
    
    /// Extract endpoints from standard OpenAPI paths
    pub fn extract_from_paths(&self) -> Vec<EndpointInfo>;
    
    /// Extract endpoints from GCP Discovery resources (recursive)
    pub fn extract_from_resources(&self) -> Vec<EndpointInfo>;
}
```

**Extraction Logic - Standard OpenAPI 3.x:**

1. Iterate `paths` map
2. For each path, check GET/POST/PUT/PATCH/DELETE operations
3. Extract `parameters` array, categorize by `in` field (`"path"` vs `"query"`)
4. Extract `requestBody.content.application/json.schema.$ref`
5. Extract `responses["200/201/202"].content.application/json.schema.$ref`
6. Extract error responses from 4xx/5xx status codes
7. Generate operation_id if missing from path + method

**Extraction Logic - GCP Discovery:**

1. Recursively traverse `resources` map
2. For each method in `resources[].methods`:
   - Use `flatPath` for actual URL pattern (NOT `path` which is a template)
   - Use `parameterOrder` to get path params in correct order
   - Parameters are in a map keyed by parameter name
   - Extract `requestBody.$ref`
   - Extract `response.$ref`
3. Handle nested resources (`resources[].resources[]`)

**GCP-Specific Handling:**

```rust
// GCP path is a template: "v1/{+name}"
// flatPath is the actual pattern: "v1/projects/{projectsId}"
let path = method.flat_path
    .or(method.path)
    .unwrap_or("");

// parameterOrder gives path params in order
let path_params = method.parameter_order
    .unwrap_or_default();

// Parameters are in a map, extract location from parameter object
for (param_name, param) in &method.parameters {
    match param.location.as_deref() {
        Some("path") => path_params.push(param_name.clone()),
        Some("query") => query_params.push(param_name.clone()),
        _ => {}
    }
}
```

---

### `normalizer.rs` - Spec Normalization

**WHY:** Produce a simplified JSON representation for quick introspection and code generation.

**WHAT:** `NormalizedSpec` struct with serializable endpoint and type definitions.

```rust
use serde::Serialize;
use std::collections::BTreeMap;

/// Normalized OpenAPI spec representation
#[derive(Debug, Serialize)]
pub struct NormalizedSpec {
    /// Map of endpoint path -> method -> details
    pub endpoints: BTreeMap<String, BTreeMap<String, NormalizedEndpoint>>,
    
    /// All discovered type definitions
    pub types: BTreeMap<String, TypeDefinition>,
    
    /// Metadata about the spec
    pub metadata: SpecMetadata,
}

#[derive(Debug, Serialize)]
pub struct NormalizedEndpoint {
    pub operation_id: String,
    pub request_type: Option<String>,
    pub response_type: Option<String>,
    pub error_types: BTreeMap<String, String>,
    pub path_params: Vec<String>,
    pub query_params: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct TypeDefinition {
    pub name: String,
    pub kind: TypeKind,
    pub properties: Vec<PropertyDefinition>,
}

#[derive(Debug, Serialize)]
pub enum TypeKind {
    Object,
    Composition(CompositionType),
    Primitive,
    Array,
}

#[derive(Debug, Serialize)]
pub enum CompositionType {
    AllOf(Vec<String>),
    OneOf(Vec<String>),
    AnyOf(Vec<String>),
}

#[derive(Debug, Serialize)]
pub struct PropertyDefinition {
    pub name: String,
    pub ty: String,
    pub required: bool,
}

#[derive(Debug, Serialize)]
pub struct SpecMetadata {
    pub spec_format: SpecFormat,
    pub base_url: Option<String>,
    pub total_endpoints: usize,
    pub total_types: usize,
}

#[derive(Debug, Serialize)]
pub enum SpecFormat {
    OpenApi3x,
    GcpDiscovery,
    Consolidated,
}
```

**Normalized JSON Output Format:**

```json
{
  "endpoints": {
    "/v1/projects/{projectId}": {
      "GET": {
        "operation_id": "getV1ProjectsByProjectId",
        "request_type": null,
        "response_type": "ProjectsGetResponse",
        "error_types": {
          "401": "ApiError",
          "403": "ApiError", 
          "404": "NotFoundError"
        },
        "path_params": ["projectId"],
        "query_params": []
      }
    }
  },
  "types": {
    "ProjectsGetResponse": {
      "kind": "object",
      "properties": [
        {"name": "data", "ty": "Project", "required": true}
      ]
    },
    "Project": {
      "kind": "object",
      "properties": [
        {"name": "id", "ty": "string", "required": true},
        {"name": "name", "ty": "string", "required": true}
      ]
    },
    "UnionType": {
      "kind": "composition",
      "CompositionType": {
        "oneOf": ["TypeA", "TypeB"]
      }
    }
  },
  "metadata": {
    "spec_format": "openapi_3x",
    "base_url": "https://api.example.com",
    "total_endpoints": 42,
    "total_types": 156
  }
}
```

**Benefits of Normalized Format:**

- Single pass to understand entire API surface
- Easy to diff between spec versions
- Quick lookup for code generation
- Debugging and visualization friendly
- Can be cached for faster subsequent processing

---

### `lib.rs` - High-Level API

**WHY:** Provide a clean, simple API for consumers.

**WHAT:** `SpecProcessor` as the main entry point.

```rust
pub mod spec;
pub mod endpoint;
pub mod type_resolver;
pub mod extractor;
pub mod normalizer;

pub use endpoint::*;
pub use normalizer::*;
pub use spec::*;
pub use type_resolver::*;

use serde_json::Value;

/// Main entry point for processing OpenAPI specs
pub struct SpecProcessor<'a> {
    spec: &'a OpenApiSpec,
    resolver: TypeResolver<'a>,
    extractor: EndpointExtractor<'a>,
}

impl<'a> SpecProcessor<'a> {
    /// Create processor from parsed spec
    pub fn new(spec: &'a OpenApiSpec) -> Self;
    
    /// Get all endpoints with full type information
    pub fn endpoints(&self) -> Vec<EndpointInfo>;
    
    /// Get all type definitions
    pub fn types(&self) -> Vec<TypeDefinition>;
    
    /// Get normalized representation
    pub fn normalize(&self) -> NormalizedSpec;
    
    /// Export normalized spec as JSON string
    pub fn to_normalized_json(&self) -> Result<String, serde_json::Error>;
    
    /// Get base URL for the API
    pub fn base_url(&self) -> Option<String>;
    
    /// Get API version from info
    pub fn version(&self) -> &str;
    
    /// Get API title from info
    pub fn title(&self) -> &str;
}

/// Convenience function to process a spec JSON string
pub fn process_spec(json: &str) -> Result<SpecProcessor, ProcessError> {
    let spec: OpenApiSpec = serde_json::from_str(json)?;
    Ok(SpecProcessor::new(&spec))
}

/// Convenience function to process and normalize a spec
pub fn normalize_spec(json: &str) -> Result<NormalizedSpec, ProcessError> {
    let processor = process_spec(json)?;
    Ok(processor.normalize())
}
```

---

## Error Handling

```rust
use derive_more::{Display, From};

#[derive(Debug, Display, From)]
pub enum ProcessError {
    /// JSON parse error
    #[display("JSON parse error: {_0}")]
    Json(serde_json::Error),
    
    /// Invalid OpenAPI spec structure
    #[display("Invalid OpenAPI spec: {0}")]
    InvalidSpec(String),
    
    /// Unresolved $ref
    #[display("Unresolved $ref: {0}")]
    UnresolvedRef(String),
    
    /// No base URL found in spec
    #[display("No base URL found in spec")]
    NoBaseUrl,
    
    /// No endpoints found
    #[display("No endpoints found in spec")]
    NoEndpoints,
}

impl std::error::Error for ProcessError {}
```

---

## Implementation Plan

### Phase 1: Core Structures (MVP)

1. **`spec.rs`** - Minimal OpenAPI structures
   - Define `OpenApiSpec`, `Schema`, `PathItem`, `Operation`, `Response`, `MediaType`
   - Support both OpenAPI 3.x and GCP Discovery fields
   - Custom deserializer for `type` field (string or array)
   - Implement `base_url()` helper that handles all formats

2. **`endpoint.rs`** - Endpoint information
   - Define `EndpointInfo`, `ResponseType`
   - Helper methods for name generation (`args_struct_name`, `fn_name`)
   - `extract_path_params()` utility
   - `to_pascal_case()` and `to_snake_case()` utilities

3. **`type_resolver.rs`** - Type resolution
   - `$ref` resolution (both `#/components/schemas/` and `#/schemas/` formats)
   - Composition type detection
   - `is_generatable()` logic with clear rules
   - `normalize_type_name()` for cross-format name handling
   - `rename_if_keyword()` for Rust keyword conflicts

4. **`extractor.rs`** - Endpoint extraction
   - Standard OpenAPI paths extraction
   - GCP Discovery resources extraction (recursive)
   - Consolidated format detection and handling

### Phase 2: Normalization

5. **`normalizer.rs`** - Spec normalization
   - Define `NormalizedSpec`, `NormalizedEndpoint`, `TypeDefinition`
   - Implement `normalize()` method
   - JSON export functionality
   - `SpecMetadata` with format detection

6. **`lib.rs`** - High-level API
   - `SpecProcessor` struct
   - `process_spec()` convenience function
   - Error types

### Phase 3: Integration

7. **Update `bin/platform/src/gen_resources/types.rs`**
   - Replace inline extraction with `foundation_openapi::SpecProcessor`
   - Use `endpoints()` for ResourceIdentifier generation
   - Use `types()` for type definitions
   - Remove duplicated spec structures

8. **Update `bin/platform/src/gen_resources/clients.rs`**
   - Replace inline extraction with `foundation_openapi::SpecProcessor`
   - Use shared type resolution logic
   - Remove duplicated spec structures

9. **Update `foundation_deployment/src/providers/openapi.rs`**
   - Deprecate old extraction functions
   - Re-export from `foundation_openapi`

---

## Edge Cases & Considerations

### Composition Types

```rust
// allOf with single $ref - treat as the referenced type
// Example: ErrorResponse { allOf: [{ $ref: "#/components/schemas/BaseError" }] }
allOf: [{ $ref: "#/components/schemas/BaseType" }]
→ Generated("BaseType")

// allOf with multiple refs or inline schemas - not generatable
// Example: CombinedType { allOf: [{ $ref: "#/components/schemas/A" }, { type: "object", ... }] }
allOf: [{ $ref: "#/components/schemas/A" }, { $ref: "#/components/schemas/B" }]
→ JsonValue

// oneOf/anyOf - always JsonValue (union types)
// Example: SearchResult { oneOf: [{ $ref: "#/components/schemas/User" }, { $ref: "#/components/schemas/Group" }] }
oneOf: [{ $ref: "#/components/schemas/A" }, { $ref: "#/components/schemas/B" }]
→ JsonValue
```

### GCP Discovery Specifics

| Issue | Solution |
|-------|----------|
| `path` is a template like `"v1/{+name}"` | Use `flatPath` for actual URL pattern |
| `parameterOrder` gives path params in order | Use for correct parameter ordering |
| `$ref` format is `"GoogleCloudRunV2Service"` (no prefix) | Handle both `#/schemas/` and bare names |
| Nested resources require recursive traversal | Implement recursive `extract_from_resources()` |
| Parameters in map, not array | Iterate map, extract `location` field |

### Response Type Priority

When multiple success status codes exist, use this priority:

1. `200` - OK (preferred)
2. `201` - Created
3. `202` - Accepted
4. `204` - No Content (special case → `ResponseType::NoContent`)

### Error Response Handling

Common patterns:

- **Standard OpenAPI:** `responses["400"].content.application/json.schema.$ref`
- **GCP Discovery:** Usually a shared `Error` or `Status` type
- **Some specs:** Use `default` response for errors

### Type Name Normalization

| Input | Output | Notes |
|-------|--------|-------|
| `treasury.transaction` | `TreasuryTransaction` | Dot-separated |
| `Custom-pages` | `CustomPages` | Hyphenated |
| `iam_response_collection` | `IamResponseCollection` | Snake case |
| `GoogleCloudRunV2Service` | `GoogleCloudRunV2Service` | Already Pascal |
| `@cf_ai4bharat.translation` | `CfAi4bharatTranslation` | Strip @ prefix |
| `Custom-pages.v2` | `CustomPagesV2` | Multiple separators |

### Rust Keyword Conflicts

Types that conflict with Rust built-ins must be renamed:

| Original | Renamed |
|----------|---------|
| `Option` | `ApiOption` |
| `Value` | `ApiValue` |
| `Result` | `ApiResult` |
| `Ok` | `ApiOk` |
| `Err` | `ApiErr` |
| `Some` | `ApiSome` |
| `None` | `ApiNone` |
| `Box` | `ApiBox` |
| `Vec` | `ApiVec` |
| `String` | `ApiString` |

---

## Testing Strategy

### Unit Tests

1. **Type Resolution:**
   - Test each composition type scenario
   - Test `$ref` resolution for both formats
   - Test primitive type handling
   - Test `is_generatable()` edge cases

2. **Endpoint Extraction:**
   - Test standard OpenAPI path extraction
   - Test GCP Discovery resource extraction
   - Test parameter categorization (path vs query)
   - Test `flatPath` vs `path` handling

3. **Name Normalization:**
   - Test all separator types (dots, hyphens, underscores)
   - Test consecutive separators
   - Test already-pascal inputs
   - Test Rust keyword conflicts

4. **Format Detection:**
   - Test OpenAPI 3.x detection
   - Test GCP Discovery detection
   - Test consolidated format detection

### Integration Tests

1. **Real Spec Processing:**
   - Process Prisma PostgreSQL spec (clean OpenAPI 3.x)
   - Process GCP CloudKMS spec (GCP Discovery format)
   - Process Cloudflare spec (mixed format)
   - Process consolidated GCP multi-API spec

2. **Round-Trip Verification:**
   - Generate normalized JSON
   - Verify all endpoints have correct type mappings
   - Compare against known-good manual extractions

3. **Code Generator Verification:**
   - Run generators with old and new implementations
   - Verify output is identical (or improved)

---

## Dependencies

```toml
[package]
name = "foundation_openapi"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
derive_more = { workspace = true }
tracing = { workspace = true }
regex = "1.10"
indexmap = { version = "2.0", features = ["serde"] }
thiserror = "1.0"
```

---

## Migration Path

1. **Create `foundation_openapi` crate** with MVP (Phase 1)
2. **Add tests** against real specs (Prisma, GCP, Cloudflare)
3. **Update `gen_resources/types.rs`** to use new crate
   - Keep old code until new code produces identical output
   - Gradually migrate functionality
4. **Update `gen_resources/clients.rs`** to use new crate
   - Same gradual approach
5. **Remove duplicated logic** from both generators
6. **Update `foundation_deployment`** to re-export from new crate
7. **Add documentation** and examples

---

## Success Criteria

- [ ] All three spec formats (OpenAPI 3.x, GCP Discovery, consolidated) parse correctly
- [ ] Endpoint extraction produces correct `EndpointInfo` for all tested specs
- [ ] Type resolution handles all composition scenarios correctly
- [ ] Normalized JSON output is valid and human-readable
- [ ] Code generators produce identical output after migration
- [ ] No duplicated extraction logic in consumers
- [ ] All unit tests pass
- [ ] `cargo clippy -- -D warnings` passes
- [ ] Documentation is complete with examples

---

## Verification Commands

```bash
# Build and check
cd backends/foundation_openapi
cargo check
cargo clippy -- -D warnings
cargo fmt -- --check

# Run tests
cargo test

# Integration test with real specs
cd bin/platform
cargo run -- gen_provider_types --use-foundation-openapi
cargo run -- gen_provider_clients --use-foundation-openapi
```

---

_Created: 2026-04-09_

---

## Appendix: Key Code Patterns from Existing Implementation

### A. Type Field Deserializer (from types.rs)

```rust
fn deserialize_type_field<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum TypeField {
        String(String),
        Array(Vec<String>),
    }

    let opt = Option::<TypeField>::deserialize(deserializer)?;
    Ok(match opt {
        Some(TypeField::String(s)) => Some(s),
        Some(TypeField::Array(arr)) => {
            let non_null: Vec<&String> = arr.iter().filter(|s| s != &"null").collect();
            if non_null.len() == 1 {
                Some(non_null[0].clone())
            } else if non_null.is_empty() {
                None
            } else {
                Some(non_null[0].clone())
            }
        }
        None => None,
    })
}
```

### B. Object Schema Collection (from types.rs)

```rust
fn collect_object_schema_names(&self, spec: &Value) -> BTreeSet<String> {
    let mut names = BTreeSet::new();

    let schemas_maps: Vec<&serde_json::Map<String, Value>> = {
        let mut maps = Vec::new();
        if let Some(s) = spec
            .get("components")
            .and_then(|c| c.get("schemas"))
            .and_then(|s| s.as_object())
        {
            maps.push(s);
        }
        if let Some(s) = spec.get("schemas").and_then(|s| s.as_object()) {
            maps.push(s);
        }
        // Consolidated format
        if maps.is_empty() {
            if let Some(obj) = spec.as_object() {
                for (_api_name, api_spec) in obj {
                    if let Some(s) = api_spec
                        .get("components")
                        .and_then(|c| c.get("schemas"))
                        .and_then(|s| s.as_object())
                    {
                        maps.push(s);
                    } else if let Some(s) =
                        api_spec.get("schemas").and_then(|s| s.as_object())
                    {
                        maps.push(s);
                    }
                }
            }
        }
        maps
    };

    for schemas in schemas_maps {
        for (name, value) in schemas {
            let has_composition = value.get("allOf").is_some()
                || value.get("oneOf").is_some()
                || value.get("anyOf").is_some();
            let is_object = value.get("type").and_then(|t| t.as_str()) == Some("object");
            let is_response = name.to_lowercase().contains("response");

            if is_object || has_composition || is_response {
                names.insert(name.clone());
            }
        }
    }

    names
}
```

### C. Snake Case Conversion (from types.rs)

```rust
fn to_snake_case(&self, name: &str) -> String {
    let mut parts = Vec::new();
    let mut current = String::new();

    let chars: Vec<char> = name.chars().collect();
    for i in 0..chars.len() {
        let c = chars[i];
        if !c.is_alphanumeric() {
            if !current.is_empty() {
                parts.push(current.clone());
                current.clear();
            }
        } else if c.is_uppercase() {
            if !current.is_empty() {
                let next_is_lower = i + 1 < chars.len() && chars[i + 1].is_lowercase();
                if next_is_lower || current.chars().last().map_or(false, |p| p.is_lowercase()) {
                    parts.push(current.clone());
                    current.clear();
                }
            }
            current.push(c.to_ascii_lowercase());
        } else {
            current.push(c);
        }
    }

    if !current.is_empty() {
        parts.push(current);
    }

    parts.join("_")
}
```

### D. Pascal Case Conversion (from types.rs)

```rust
fn to_pascal_case(&self, name: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = true;

    for c in name.chars() {
        if c.is_alphanumeric() {
            if capitalize_next {
                result.push(c.to_ascii_uppercase());
                capitalize_next = false;
            } else {
                result.push(c.to_ascii_lowercase());
            }
        } else {
            capitalize_next = true;
        }
    }

    result
}
```

### E. GCP Endpoint Extraction (from clients.rs)

```rust
fn extract_gcp_endpoints(
    &self,
    resources: &BTreeMap<String, Resource>,
    parent_path: &str,
    endpoints: &mut Vec<ApiEndpoint>,
) {
    let placeholder_re = Regex::new(r"\{(\+)?([^}]+)\}").unwrap();

    for (_resource_name, resource) in resources {
        if let Some(methods) = &resource.methods {
            for (_method_name, method) in methods {
                let path = method.flat_path.as_deref().unwrap_or_else(|| method.path.as_deref().unwrap_or(""));

                let path_placeholders: Vec<String> = placeholder_re
                    .captures_iter(path)
                    .map(|cap| cap[2].to_string())
                    .collect();

                let mut path_params = Vec::new();
                let mut query_params = Vec::new();

                if let Some(param_order) = &method.parameter_order {
                    for param_name in param_order {
                        if let Some(param_info) = all_params.get(param_name) {
                            path_params.push(param_info.clone());
                        }
                    }
                }

                let response_type = method
                    .response
                    .as_ref()
                    .and_then(|resp| resp.ref_path.as_ref())
                    .and_then(|ref_path| self.extract_type_name_from_gcp_ref(ref_path));

                endpoints.push(ApiEndpoint {
                    path: path.to_string(),
                    method: method.http_method.as_deref().unwrap_or("GET").to_uppercase(),
                    operation_id: method.id.clone(),
                    summary: method.description.clone(),
                    path_params,
                    query_params,
                    request_body_type,
                    response_type,
                    base_url: base_url.map(String::from),
                    path_placeholders,
                });
            }
        }

        if let Some(nested) = &resource.resources {
            self.extract_gcp_endpoints(nested, parent_path, endpoints);
        }
    }
}
```

### F. Response Type Extraction with Composition Handling (from clients.rs)

```rust
fn extract_response_type(&self, responses: &BTreeMap<String, Response>, spec: &OpenApiSpec) -> Option<String> {
    for status in &["200", "201", "202", "204"] {
        if let Some(response) = responses.get(*status) {
            if let Some(content) = &response.content {
                if let Some(media) = content.get("application/json") {
                    if let Some(schema) = &media.schema {
                        let schema_to_check = if schema.ref_path.is_some()
                            && schema.properties.is_none()
                            && schema.any_of.is_none()
                            && schema.one_of.is_none()
                        {
                            schema.ref_path.as_ref()
                                .and_then(|ref_path| {
                                    let type_name = ref_path.trim_start_matches("#/components/schemas/");
                                    spec.components.as_ref()
                                        .and_then(|c| c.schemas.as_ref())
                                        .and_then(|schemas| schemas.get(type_name))
                                })
                        } else {
                            Some(schema)
                        };

                        let is_generatable = schema_to_check.map_or(true, |s| {
                            s.properties.is_some()
                            || (s.all_of.is_none() && s.any_of.is_none() && s.one_of.is_none())
                        });

                        if is_generatable {
                            return self.extract_type_name_from_ref(schema);
                        } else {
                            return Some("serde_json::Value".to_string());
                        }
                    }
                }
            }
            if *status == "204" {
                return Some("()".to_string());
            }
        }
    }
    None
}
```
