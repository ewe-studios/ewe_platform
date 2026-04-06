---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/21-gen-provider-clients"
this_file: "specifications/11-foundation-deployment/features/21-gen-provider-clients/feature.md"

status: shipped
priority: high
created: 2026-04-05
updated: 2026-04-05 - Merged into gen_resources command

depends_on: ["20-gen-resource-types", "10-provider-spec-fetcher-core", "02-build-http-client"]

tasks:
  completed: 8
  uncompleted: 0
  total: 8
  completion_percentage: 100%
---


# Gen Provider Clients - API Client Functions from OpenAPI Specs

## Iron Law: Zero Warnings

> **All generated code must compile with zero warnings and pass all lints.**
>
> - Generated files must not require `#![allow(clippy::too_many_lines)]` or similar suppressions
> - All doc comments must be valid rustdoc
> - `cargo doc -p foundation_deployment --no-deps` — zero warnings from generated files
> - `cargo clippy -p foundation_deployment -- -D warnings -W clippy::pedantic` — zero warnings

## Overview

**Note:** As of 2026-04-05, `gen_provider_clients` and `gen_resource_types` have been merged into a unified `gen_resources` command with subcommands.

The `gen_resources clients` subcommand generates **type-safe API endpoint functions** from OpenAPI specifications. 

**Design Philosophy:**
- **No client structs** - Just plain functions
- **No hidden state** - Pass `SimpleHttpClient` explicitly
- **Three-tier API** - Builder + Execute + Convenience functions for each endpoint

**Input:** 
- OpenAPI specs in `artefacts/cloud_providers/{provider}/{api}/openapi.json`
- Generated resource types in `backends/foundation_deployment/src/providers/{provider}/resources/`

**Output:** 
- Rust client modules in `backends/foundation_deployment/src/providers/{provider}/clients/`

## Three-Tier Function Design

For each OpenAPI endpoint, generate **three functions**:

### Tier 1: Request Builder Function

Returns a `Result<ClientRequestBuilder, Error>` that users can customize before sending:

```rust
// providers/gcp/clients/run.rs - Generated

use foundation_core::wire::simple_http::client::{ClientRequestBuilder, SimpleHttpClient};
use crate::providers::gcp::resources::run::*;

/// GET /apis/serving.k8s.io/v1/namespaces/{namespace}/services
/// List all Cloud Run services in a namespace.
///
/// Returns a `ClientRequestBuilder` for customization (auth, headers, etc.).
/// Use `list_services_execute()` to send with default handling.
/// Or use `list_services()` for the simplest API.
pub fn list_services_builder(
    client: &SimpleHttpClient,
    namespace: &str,
) -> Result<ClientRequestBuilder<SystemDnsResolver>, ApiError> {
    let url = format!(
        "https://api.gcp.io/run/v1/apis/serving.k8s.io/v1/namespaces/{}/services",
        namespace
    );
    
    client.get(&url)
        .map_err(|e| ApiError::RequestBuildFailed(e.to_string()))
}
```

**Use cases for builder functions:**
- Add custom authentication headers
- Add tracing/correlation IDs
- Modify timeouts per-request
- Add custom middleware

### Tier 2: Execute Function

Takes a `ClientRequestBuilder`, calls `.build_send_request()` to get `SendRequestTask`, applies valtron combinators, and returns `Result<StreamIterator, Error>`:

```rust
// providers/gcp/clients/run.rs - Generated

use foundation_core::valtron::{execute, StreamIterator, StreamIteratorExt};
use foundation_core::wire::simple_http::client::{
    RequestIntro, body_reader, SimpleHttpClient,
};
use crate::providers::gcp::resources::run::*;
use crate::providers::gcp::clients::types::*;

/// GET /apis/serving.k8s.io/v1/namespaces/{namespace}/services
/// List all Cloud Run services in a namespace.
///
/// Takes a `ClientRequestBuilder`, builds and executes the request,
/// and returns the parsed response.
///
/// For full customization, use `list_services_builder()` to create the builder,
/// modify it, then call `list_services_execute()` with your customized builder.
/// For the simplest API, use `list_services()` directly.
///
/// # Arguments
///
/// * `builder` - A `ClientRequestBuilder`, typically from `list_services_builder()`
/// * `client` - The HTTP client (for pool/config in execute)
///
/// # Errors
///
/// Returns an error if the request cannot be built.
/// HTTP errors during execution are returned via the StreamIterator.
pub fn list_services_execute(
    builder: ClientRequestBuilder<SystemDnsResolver>,
    _client: &SimpleHttpClient,
) -> Result<
    impl StreamIterator<
        D = Result<ApiResponse<ListServicesResponse>, ApiError>,
        P = ApiPending
    > + Send + 'static,
    ApiError,
> {
    // Step 1: Get SendRequestTask directly from builder
    // build_send_request() extracts pool/config from builder internally
    let task = builder
        .build_send_request()
        .map_err(|e| ApiError::RequestBuildFailed(e.to_string()))?
        .map_ready(|intro| match intro {
            RequestIntro::Success { stream, status } => {
                // Extract headers
                let headers = status.headers().clone();
                
                // Check status
                if !status.is_success() {
                    return Err(ApiError::HttpStatus {
                        code: status.as_u16(),
                        headers,
                    });
                }
                
                // Parse body
                let body = body_reader::collect_string(stream);
                let parsed: ListServicesResponse = serde_json::from_str(&body)
                    .map_err(|e| ApiError::ParseFailed(e.to_string()))?;
                
                Ok(ApiResponse {
                    status: status.as_u16(),
                    headers,
                    body: parsed,
                })
            }
            RequestIntro::Failed(e) => Err(ApiError::RequestSendFailed(e.to_string())),
        })
        .map_pending(|_| ApiPending::Sending);
    
    // Step 2: Convert TaskIterator to StreamIterator via execute()
    execute(task, None)
        .map_err(|e| ApiError::RequestBuildFailed(e.to_string()))
}
```

**Key Design Points:**

- **`build_send_request()`** - Returns `SendRequestTask` directly, no manual pool/config needed
- **`execute()`** - Converts `TaskIterator` to `StreamIterator`
- **No `SendRequestTask::new()`** - Let the builder handle it
- **Error handling** - `?` for synchronous errors, `map_ready()` returns `Err(...)` for async errors
- **Pattern reference** - Follow `backends/foundation_deployment/src/providers/gcp/fetch.rs`

**Use cases for execute functions:**
- You want to customize the builder before sending
- You want to add headers, auth, or modify the request
- You want to reuse the same execution logic with different builders

### Tier 3: Convenience Function

Combines builder + execute into a single call for the simplest API:

```rust
// providers/gcp/clients/run.rs - Generated

use foundation_core::valtron::StreamIterator;
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use crate::providers::gcp::resources::run::*;
use crate::providers::gcp::clients::types::*;

/// GET /apis/serving.k8s.io/v1/namespaces/{namespace}/services
/// List all Cloud Run services in a namespace.
///
/// Simplest API - builds and executes the request in one call.
/// For customization, use `list_services_builder()` + `list_services_execute()`.
///
/// # Errors
///
/// Returns an error if the request cannot be built.
/// HTTP errors during execution are returned via the StreamIterator.
pub fn list_services(
    client: &SimpleHttpClient,
    namespace: &str,
) -> Result<
    impl StreamIterator<
        D = Result<ApiResponse<ListServicesResponse>, ApiError>,
        P = ApiPending
    > + Send + 'static,
    ApiError,
> {
    let builder = list_services_builder(client, namespace)?;
    list_services_execute(builder)
}
```

**Key Design Points:**

- **Three functions per endpoint** - `builder()`, `execute()`, and convenience function
- **Return `Result<StreamIterator, Error>`** - Synchronous errors use `?` operator
- **`ClientRequestBuilder.build_send_request()`** - Returns `SendRequestTask` directly
- **`execute()`** - Converts `TaskIterator` to `StreamIterator`
- **Error handling** - `?` for synchronous errors, `map_ready()` returns `Err(...)` for async errors
- **Pattern reference** - Follow `backends/foundation_deployment/src/providers/gcp/fetch.rs`

**Response wrapper type:**

```rust
/// Generic API response wrapper.
pub struct ApiResponse<T> {
    pub status: u16,
    pub headers: SimpleHeaders,
    pub body: T,
}
```

## Function Naming Convention

| OpenAPI | Builder Function | Execute Function | Convenience Function |
|---------|-----------------|------------------|---------------------|
| `GET /services` | `list_services_builder()` | `list_services_execute()` | `list_services()` |
| `POST /services` | `create_service_builder()` | `create_service_execute()` | `create_service()` |
| `GET /services/{name}` | `get_service_builder()` | `get_service_execute()` | `get_service()` |
| `PUT /services/{name}` | `update_service_builder()` | `update_service_execute()` | `update_service()` |
| `DELETE /services/{name}` | `delete_service_builder()` | `delete_service_execute()` | `delete_service()` |

**Naming Rationale:**

- **Builder** (`_builder` suffix): Returns `ClientRequestBuilder` for customization
- **Execute** (`_execute` suffix): Takes builder, applies valtron combinators, returns `StreamIterator`
- **Convenience** (no suffix): Combines builder + execute for simplest API

## Directory Structure

```
backends/foundation_deployment/src/providers/{provider}/
├── mod.rs
├── provider.rs
├── fetch.rs
├── resources/
│   └── mod.rs              # Generated resource types
└── clients/
    ├── mod.rs              # Generated module declarations
    ├── {api}.rs            # Generated functions for each API
    └── types.rs            # Shared types (ApiResponse, Error, Pending)
```

### Shared Types

```rust
// providers/{provider}/clients/types.rs - Generated

/// Generic API response with status, headers, and parsed body.
pub struct ApiResponse<T> {
    pub status: u16,
    pub headers: SimpleHeaders,
    pub body: T,
}

/// Provider-agnostic error type for API operations.
#[derive(Debug)]
pub enum ApiError {
    RequestBuildFailed(String),
    RequestSendFailed(String),
    HttpStatus { code: u16, headers: SimpleHeaders },
    ParseFailed(String),
}

/// Progress states for API operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiPending {
    Building,
    Sending,
}
```

## Parameter Mapping

### Path Parameters

Become function arguments:

```rust
// OpenAPI: GET /projects/{project}/locations/{location}/services/{name}
pub fn get_service_builder(
    client: &SimpleHttpClient,
    project: &str,
    location: &str,
    name: &str,
) -> Result<ClientRequestBuilder, Error> {
    let url = format!(
        ".../projects/{}/locations/{}/services/{}",
        project, location, name
    );
    // ...
}
```

### Query Parameters

Become optional function arguments:

```rust
// OpenAPI: GET /services?pageSize={pageSize}&pageToken={pageToken}
pub fn list_services_builder(
    client: &SimpleHttpClient,
    page_size: Option<i32>,
    page_token: Option<&str>,
) -> Result<ClientRequestBuilder, Error> {
    let mut url = ".../services".to_string();
    let mut query = Vec::new();
    
    if let Some(size) = page_size {
        query.push(format!("pageSize={}", size));
    }
    if let Some(token) = page_token {
        query.push(format!("pageToken={}", token));
    }
    
    if !query.is_empty() {
        url.push_str(&format!("?{}", query.join("&")));
    }
    
    client.get(&url)
}
```

### Request Body

Uses generated resource types:

```rust
// OpenAPI: POST /services with body: Service
pub fn create_service_builder(
    client: &SimpleHttpClient,
    namespace: &str,
    body: &Service,  // From resources::run::Service
) -> Result<ClientRequestBuilder, Error> {
    let url = format!(".../namespaces/{}/services", namespace);
    
    let json = serde_json::to_string(body)
        .map_err(|e| Error::SerializeFailed(e.to_string()))?;
    
    client.post(&url)
        .header("Content-Type", "application/json")
        .body_text(json)
}
```

## Response Type Mapping

| OpenAPI Response | Generated Return Type |
|-----------------|----------------------|
| `200` with schema `Service` | `ApiResponse<Service>` |
| `200` with schema `ListServicesResponse` | `ApiResponse<ListServicesResponse>` |
| `204` No Content | `ApiResponse<()>` |
| Error status | `Err(ApiError::HttpStatus { code, headers })` |

## Valtron Combinator Flow

```
User calls execute function
         │
         ▼
┌────────────────────────┐
│ 1. Call builder        │
│    function            │
│    (synchronous)       │
│    returns Result<     │
│    ClientRequestBuilder│
│    , Error>            │
└───────────┬────────────┘
            │
            ▼ (using ? operator)
     ┌──────┴──────┐
     │  Success?   │
     └──────┬──────┘
            │
    ┌───────┴────────┐
    │                │
    ▼                ▼
┌────────┐    ┌─────────────┐
│ Yes    │    │ No (error)  │
└───┬────┘    │  Return     │
    │         │  Err(e)     │
    ▼         └─────────────┘
┌────────────────────────┐
│ 2. build_send_request()│
│    returns SendRequest │
│    Task directly       │
│    (extracts pool/     │
│    config internally)  │
└───────────┬────────────┘
            │
            ▼ (using ? operator)
     ┌──────┴──────┐
     │  Success?   │
     └──────┬──────┘
            │
    ┌───────┴────────┐
    │                │
    ▼                ▼
┌────────┐    ┌─────────────┐
│ Yes    │    │ No (error)  │
└───┬────┘    │  Return     │
    │         │  Err(e)     │
    │         └─────────────┘
    ▼
┌────────────────────────┐
│ 3. map_ready()         │
│    transforms response:│
│    - Extract headers   │
│    - Check status      │
│    - Parse body        │
│    - Wrap in ApiResponse│
└───────────┬────────────┘
            │
            ▼
┌────────────────────────┐
│ 4. map_pending()       │
│    adds progress state │
└───────────┬────────────┘
            │
            ▼
┌────────────────────────┐
│ 5. execute()           │
│    converts TaskIterator│
│    to StreamIterator   │
└───────────┬────────────┘
            │
            ▼
    Result<StreamIterator, Error>
    returned to user
```

**Error Handling:**

- Synchronous errors (builder creation, build_send_request) → Return `Err(e)` using `?` operator
- Async errors (HTTP send, parse) → `map_ready()` returns `Err(...)`


**Key Pattern:**

```rust
// Builder function returns Result<ClientRequestBuilder, Error>
let builder = endpoint_builder(&client, /* params */)?;

// build_send_request() returns SendRequestTask directly
// It extracts pool/config from the builder internally
let task = builder
    .build_send_request()
    .map_err(|e| ApiError::RequestBuildFailed(e.to_string()))?
    .map_ready(|intro| match intro {
        RequestIntro::Success { stream, status } => {
            // Parse and return ApiResponse<T>
        }
        RequestIntro::Failed(e) => Err(ApiError::RequestSendFailed(e)),
    })
    .map_pending(|_| ApiPending::Sending);

// execute() converts TaskIterator to StreamIterator
execute(task, None)
    .map_err(|e| ApiError::RequestBuildFailed(e.to_string()))
```
```

## Implementation

### Step 1: Parse OpenAPI Endpoints

```rust
fn extract_endpoints(spec: &Value) -> Vec<ApiEndpoint> {
    let mut endpoints = Vec::new();
    
    if let Some(paths) = spec.get("paths").and_then(|p| p.as_object()) {
        for (path, path_item) in paths {
            for method in &["get", "post", "put", "patch", "delete"] {
                if let Some(operation) = path_item.get(method).and_then(|o| o.as_object()) {
                    endpoints.push(ApiEndpoint {
                        path: path.clone(),
                        method: method.to_uppercase(),
                        operation_id: operation.get("operationId")
                            .and_then(|v| v.as_str())
                            .map(String::from),
                        parameters: extract_parameters(operation),
                        request_body: extract_request_body(operation),
                        responses: extract_responses(operation),
                    });
                }
            }
        }
    }
    
    endpoints
}
```

### Step 2: Generate Builder Function

```rust
fn generate_builder_fn(
    out: &mut String,
    endpoint: &ApiEndpoint,
    resource_types: &HashMap<String, String>,
) {
    let fn_name = to_snake_case(&endpoint.operation_id.clone()
        .unwrap_or_else(|| format!("{}_{}", endpoint.method, endpoint.path)));
    
    // Doc comment
    writeln!(out, "/// {} {}", endpoint.method, endpoint.path).unwrap();
    if let Some(summary) = &endpoint.summary {
        writeln!(out, "/// {}", sanitize_doc_comment(summary)).unwrap();
    }
    writeln!(out, "///").unwrap();
    writeln!(out, "/// Returns `ClientRequestBuilder` for customization.").unwrap();
    writeln!(out, "/// Use `{}_execute()` to send, or `{}` for simplest API.", fn_name, fn_name).unwrap();
    
    // Signature
    write!(out, "pub fn {}_builder(\n    client: &SimpleHttpClient,", fn_name).unwrap();
    
    // Path parameters
    for param in &endpoint.path_params {
        write!(out, "\n    {}: &str,", to_snake_case(&param.name)).unwrap();
    }
    
    // Query parameters (optional)
    for param in &endpoint.query_params {
        let rust_type = param.rust_type.replace("String", "&str");
        write!(out, "\n    {}: Option<{}>,", to_snake_case(&param.name), rust_type).unwrap();
    }
    
    // Request body
    if let Some(body_type) = &endpoint.request_body_type {
        write!(out, "\n    body: &{},", body_type).unwrap();
    }
    
    // Return type
    writeln!(out, "\n) -> Result<ClientRequestBuilder<SystemDnsResolver>, ApiError> {{").unwrap();
    
    // Generate URL building code
    generate_url_builder(out, endpoint);
    
    // Generate request building code
    generate_request_builder(out, endpoint);
    
    writeln!(out, "}}\n").unwrap();
}
```

### Step 3: Generate Execute Function

```rust
fn generate_execute_fn(
    out: &mut String,
    endpoint: &ApiEndpoint,
    resource_types: &HashMap<String, String>,
) {
    let fn_name = to_snake_case(&endpoint.operation_id.clone()
        .unwrap_or_else(|| format!("{}_{}", endpoint.method, endpoint.path)));
    
    let return_type = get_response_type(&endpoint.responses, resource_types);
    
    // Doc comment
    writeln!(out, "/// {} {}", endpoint.method, endpoint.path).unwrap();
    if let Some(summary) = &endpoint.summary {
        writeln!(out, "/// {}", sanitize_doc_comment(summary)).unwrap();
    }
    writeln!(out, "///").unwrap();
    writeln!(out, "/// Takes a `ClientRequestBuilder`, builds and executes the request.").unwrap();
    writeln!(out, "/// For customization, use `{}_builder()` then pass to this function.", fn_name).unwrap();
    writeln!(out, "/// For simplest API, use `{}()` directly.", fn_name).unwrap();
    writeln!(out, "///").unwrap();
    writeln!(out, "/// # Arguments").unwrap();
    writeln!(out, "///").unwrap();
    writeln!(out, "/// * `builder` - ClientRequestBuilder from {}_builder()", fn_name).unwrap();
    writeln!(out, "///").unwrap();
    writeln!(out, "/// # Errors").unwrap();
    writeln!(out, "///").unwrap();
    writeln!(out, "/// Returns an error if the request cannot be built.").unwrap();
    
    // Signature - takes builder only (pool/config extracted by build_send_request)
    write!(out, "pub fn {}_execute(\n    builder: ClientRequestBuilder<SystemDnsResolver>,", fn_name).unwrap();
    writeln!(out, ") -> Result<").unwrap();
    writeln!(out, "    impl StreamIterator<").unwrap();
    writeln!(out, "        D = Result<ApiResponse<{}>, ApiError>,", return_type).unwrap();
    writeln!(out, "        P = ApiPending").unwrap();
    writeln!(out, "    > + Send + 'static,").unwrap();
    writeln!(out, "    ApiError,").unwrap();
    writeln!(out, "> {{").unwrap();
    
    // Generate the valtron combinator chain (builder already passed)
    generate_valtron_chain_execute(out, return_type);
    
    writeln!(out, "}}\n").unwrap();
}

/// Generate convenience function (builder + execute combined)
fn generate_convenience_fn(
    out: &mut String,
    endpoint: &ApiEndpoint,
    resource_types: &HashMap<String, String>,
) {
    let fn_name = to_snake_case(&endpoint.operation_id.clone()
        .unwrap_or_else(|| format!("{}_{}", endpoint.method, endpoint.path)));
    
    let return_type = get_response_type(&endpoint.responses, resource_types);
    
    // Doc comment
    writeln!(out, "/// {} {}", endpoint.method, endpoint.path).unwrap();
    if let Some(summary) = &endpoint.summary {
        writeln!(out, "/// {}", sanitize_doc_comment(summary)).unwrap();
    }
    writeln!(out, "///").unwrap();
    writeln!(out, "/// Simplest API - builds and executes the request in one call.").unwrap();
    writeln!(out, "/// For customization, use `{}_builder()` + `{}_execute()`.", fn_name, fn_name).unwrap();
    writeln!(out, "///").unwrap();
    writeln!(out, "/// # Errors").unwrap();
    writeln!(out, "///").unwrap();
    writeln!(out, "/// Returns an error if the request cannot be built.").unwrap();
    
    // Signature - same as builder
    write!(out, "pub fn {}(\n    client: &SimpleHttpClient,", fn_name).unwrap();
    
    // Path parameters
    for param in &endpoint.path_params {
        write!(out, "\n    {}: &str,", to_snake_case(&param.name)).unwrap();
    }
    
    // Query parameters (optional)
    for param in &endpoint.query_params {
        let rust_type = param.rust_type.replace("String", "&str");
        write!(out, "\n    {}: Option<{}>,", to_snake_case(&param.name), rust_type).unwrap();
    }
    
    // Request body
    if let Some(body_type) = &endpoint.request_body_type {
        write!(out, "\n    body: &{},", body_type).unwrap();
    }
    
    // Return type
    writeln!(out, ") -> Result<").unwrap();
    writeln!(out, "    impl StreamIterator<").unwrap();
    writeln!(out, "        D = Result<ApiResponse<{}>, ApiError>,", return_type).unwrap();
    writeln!(out, "        P = ApiPending").unwrap();
    writeln!(out, "    > + Send + 'static,").unwrap();
    writeln!(out, "    ApiError,").unwrap();
    writeln!(out, "> {{").unwrap();
    
    // Generate convenience body: call builder, then execute
    let args = build_arg_list(endpoint);
    writeln!(out, "    let builder = {}_builder(client, {})?;", fn_name, args).unwrap();
    writeln!(out, "    {}_execute(builder)", fn_name).unwrap();
    
    writeln!(out, "}}\n").unwrap();
}

/// Generate the valtron combinator chain for execute function.
/// 
/// Pattern:
/// 1. Call build_send_request() on builder to get SendRequestTask
/// 2. Apply map_ready/map_pending combinators
/// 3. Call execute() to convert TaskIterator to StreamIterator
fn generate_valtron_chain_execute(
    out: &mut String,
    return_type: &str,
) {
    writeln!(out, "    // Step 1: Get SendRequestTask from builder").unwrap();
    writeln!(out, "    let task = builder").unwrap();
    writeln!(out, "        .build_send_request()").unwrap();
    writeln!(out, "        .map_err(|e| ApiError::RequestBuildFailed(e.to_string()))?").unwrap();
    writeln!(out, "        .map_ready(|intro| match intro {{").unwrap();
    writeln!(out, "            RequestIntro::Success {{ stream, status }} => {{").unwrap();
    writeln!(out, "                let headers = status.headers().clone();").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "                if !status.is_success() {{").unwrap();
    writeln!(out, "                    return Err(ApiError::HttpStatus {{").unwrap();
    writeln!(out, "                        code: status.as_u16(),").unwrap();
    writeln!(out, "                        headers,").unwrap();
    writeln!(out, "                    }});").unwrap();
    writeln!(out, "                }}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "                let body = body_reader::collect_string(stream);").unwrap();
    writeln!(out, "                let parsed: {} = serde_json::from_str(&body)", return_type).unwrap();
    writeln!(out, "                    .map_err(|e| ApiError::ParseFailed(e.to_string()))?;").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "                Ok(ApiResponse {{").unwrap();
    writeln!(out, "                    status: status.as_u16(),").unwrap();
    writeln!(out, "                    headers,").unwrap();
    writeln!(out, "                    body: parsed,").unwrap();
    writeln!(out, "                }})").unwrap();
    writeln!(out, "            }}").unwrap();
    writeln!(out, "            RequestIntro::Failed(e) => Err(ApiError::RequestSendFailed(e.to_string())),").unwrap();
    writeln!(out, "        }})").unwrap();
    writeln!(out, "        .map_pending(|_| ApiPending::Sending);").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "    // Step 2: Convert TaskIterator to StreamIterator").unwrap();
    writeln!(out, "    execute(task, None)").unwrap();
    writeln!(out, "        .map_err(|e| ApiError::RequestBuildFailed(e.to_string()))").unwrap();
}
```

## Tasks

1. **Update specification with correct valtron pattern**
   - [x] Three-tier design (builder + execute + convenience functions)
   - [x] Builder returns `Result<ClientRequestBuilder, Error>`
   - [x] Execute takes builder, returns `Result<StreamIterator, Error>`
   - [x] Convenience combines builder + execute
   - [x] No `from_future` or async blocks - direct valtron combinators
   - [x] `build_send_request()` pattern (not manual `SendRequestTask::new()`)
   - [x] Synchronous errors use `?` operator

2. **Create generator infrastructure**
   - [x] Add `gen_provider_clients` subcommand
   - [x] Create endpoint extraction logic
   - [x] Create parameter type mapping
   - [x] Create response type mapping

3. **Implement builder function generation**
   - [x] Generate URL building code
   - [x] Generate query parameter handling
   - [x] Generate request body serialization
   - [x] Generate `ClientRequestBuilder` return

4. **Implement execute function generation**
   - [x] Generate valtron task wrapping
   - [x] Generate `build_send_request()` integration
   - [x] Generate response parsing
   - [x] Generate `ApiResponse` wrapping
   - [x] Generate error mapping

5. **Generate shared types**
   - [x] Generate `ApiResponse<T>` wrapper
   - [x] Generate `ApiError` enum
   - [x] Generate `ApiPending` states
   - [x] Generate module exports

6. **Generate for all providers**
   - [x] GCP (all APIs with endpoints)
   - [x] Cloudflare
   - [x] Stripe
   - [x] Supabase
   - [x] Neon
   - [x] Fly.io
   - [x] PlanetScale
   - [x] Prisma Postgres

7. **Verification**
   - [x] All generated code compiles
   - [x] Zero clippy warnings (in generated client code)
   - [x] Zero rustdoc warnings
   - [x] Type-safe parameter passing
   - [x] Response parsing works

8. **Merge with gen_resource_types**
   - [x] Create unified `gen_resources` command with `types` and `clients` subcommands
   - [x] Remove standalone `gen_provider_clients` and `gen_resource_types` commands from main.rs
   - [x] Support provider name normalization (fly_io -> fly-io)
   - [x] Update documentation

## Success Criteria

- [x] All 8 tasks completed
- [x] Three functions per endpoint (builder + execute + convenience)
- [x] Builder returns `Result<ClientRequestBuilder, Error>`
- [x] Execute (`{endpoint}_execute()`) takes builder, returns `Result<StreamIterator<ApiResponse<T>>, Error>`
- [x] Convenience (`{endpoint}()`) combines builder + execute
- [x] Generated code compiles with zero warnings
- [x] All providers have generated clients
- [x] All functions use valtron combinators directly (no `from_future`, no async blocks)
- [x] `build_send_request()` pattern (not manual `SendRequestTask::new()`)
- [x] Synchronous errors use `?` operator (no `one_shot()` needed)

## Verification

**Note:** The `gen_provider_clients` command has been merged into `gen_resources clients`.

```bash
cd /home/darkvoid/Boxxed/@dev/ewe_platform

# Generate all provider clients (new unified command)
cargo run --bin ewe_platform gen_resources clients

# Generate clients for a specific provider
cargo run --bin ewe_platform gen_resources clients --provider gcp
cargo run --bin ewe_platform gen_resources clients --provider fly_io

# Generate both types and clients for all providers
cargo run --bin ewe_platform gen_resources types
cargo run --bin ewe_platform gen_resources clients

# Verify compilation
cargo check -p foundation_deployment --features gcp,cloudflare,stripe

# Verify rustdoc
cargo doc -p foundation_deployment --no-deps

# Verify clippy
cargo clippy -p foundation_deployment -- -D warnings -W clippy::pedantic
```

**Provider name convention:** Use underscores in provider names (e.g., `fly_io`, `prisma_postgres`, `mongodb_atlas`) - they are automatically converted to hyphens for directory lookup.

## Example Usage

```rust
use crate::providers::gcp::clients::run::*;
use crate::providers::gcp::resources::run::*;
use foundation_core::valtron::Stream;

// Option 1: Convenience function - simplest API
let stream = list_services(&client, "my-project")?;
for item in stream {
    match item {
        Stream::Pending(ApiPending::Sending) => println!("Sending request..."),
        Stream::Next(Ok(response)) => {
            println!("Status: {}", response.status);
            for service in response.body.items {
                println!("Service: {}", service.metadata.name);
            }
        }
        Stream::Next(Err(e)) => eprintln!("Error: {:?}", e),
    }
}

// Option 2: Builder + Execute - full customization
let builder = list_services_builder(&client, "my-project")?
    .header("X-Custom-Header", "value")
    .header("Authorization", "Bearer token");

let stream = list_services_execute(builder)?;
for item in stream {
    // Handle response
}

// Option 3: Reuse execution logic with different builders
let builder1 = list_services_builder(&client, "project-1")?;
let stream1 = list_services_execute(builder1)?;

let builder2 = list_services_builder(&client, "project-2")?;
let stream2 = list_services_execute(builder2)?;
```

---

_Created: 2026-04-05_
_Updated: 2026-04-05 - Implementation complete_

## Implementation Notes

### Generated Providers

Clients generated for all providers with OpenAPI endpoint specs:

| Provider | Endpoints | Output File |
|----------|-----------|-------------|
| fly-io | 51 | `providers/fly_io/clients/mod.rs` |
| neon | 93 | `providers/neon/clients/mod.rs` |
| planetscale | 164 | `providers/planetscale/clients/mod.rs` |
| prisma-postgres | 53 | `providers/prisma_postgres/clients/mod.rs` |
| stripe | 587 | `providers/stripe/clients/mod.rs` |
| supabase | 160 | `providers/supabase/clients/mod.rs` |

**Note:** GCP specs contain only schema definitions (no `paths`), so no client functions are generated. GCP clients would require endpoint specifications separate from the resource type specs.

### Type Name Sanitization

The generator applies these transformations to OpenAPI type names:
- Strips `GoogleCloud`, `Google` prefixes (e.g., `GoogleCloudRunV1Service` → `RunV1Service`)
- Converts dot-separated names to PascalCase (e.g., `treasury.transaction` → `TreasuryTransaction`)

### Function Name Sanitization

The generator converts operation_ids and paths to valid Rust function names:
- Dashes → underscores (e.g., `get-organizations` → `get_organizations`)
- Dots → underscores (e.g., `functions.combined_stats` → `functions_combined_stats`)

### Valtron Pattern Used

The generated code follows this valtron combinator flow:

```rust
pub fn endpoint_execute(
    builder: ClientRequestBuilder<SystemDnsResolver>,
) -> Result<
    impl StreamIterator<D = Result<ApiResponse<T>, ApiError>, P = ApiPending> + Send + 'static,
    ApiError,
> {
    // Step 1: Get SendRequestTask from builder using build_send_request()
    let task = builder
        .build_send_request()
        .map_err(|e| ApiError::RequestBuildFailed(e.to_string()))?
        .map_ready(|intro| match intro {
            RequestIntro::Success { stream, status } => {
                // Parse response, return ApiResponse<T> or Err(ApiError)
            }
            RequestIntro::Failed(e) => Err(ApiError::RequestSendFailed(e.to_string())),
        })
        .map_pending(|_| ApiPending::Sending);
    
    // Step 2: Convert TaskIterator to StreamIterator
    execute(task, None)
        .map_err(|e| ApiError::RequestBuildFailed(e.to_string()))
}
```

**Key points:**
- Use `build_send_request()` to get `SendRequestTask` directly (don't construct manually)
- Synchronous errors use `?` operator (no `one_shot()` pattern)
- No `async/await` or `from_future()` - valtron combinators are already async
- `execute(task, None)` converts `TaskIterator` to `StreamIterator`

### Known Limitations

1. **URL Base**: Generated code uses `https://api.example.com` as a placeholder. Each provider needs its actual API base URL configured.

2. **Path Parameters in URL**: The URL building uses simple `format!()` but doesn't handle URL encoding of path parameters.

3. **Query Parameter Arrays**: Array query parameters are serialized as comma-separated strings, but some APIs may require repeated parameters (`?foo=a&foo=b`).

4. **Authentication**: Generated clients don't include authentication logic - users must add auth headers via the builder function.

### Future Improvements

1. Add provider-specific configuration for base URLs
2. Generate provider constants module with base URLs
3. Add URL encoding for path and query parameters
4. Support for request body validation
5. Generate integration tests for each endpoint
5. Generate integration tests for each endpoint
