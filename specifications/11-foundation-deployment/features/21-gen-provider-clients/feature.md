---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/21-gen-provider-clients"
this_file: "specifications/11-foundation-deployment/features/21-gen-provider-clients/feature.md"

status: pending
priority: high
created: 2026-04-05
updated: 2026-04-05

depends_on: ["20-gen-resource-types", "10-provider-spec-fetcher-core", "02-build-http-client"]

tasks:
  completed: 0
  uncompleted: 6
  total: 6
  completion_percentage: 0%
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

The `gen_provider_clients` tool generates **type-safe API endpoint functions** from OpenAPI specifications. 

**Design Philosophy:**
- **No client structs** - Just plain functions
- **No hidden state** - Pass `SimpleHttpClient` explicitly
- **Two-tier API** - Builder function + execute function for each endpoint

**Input:** 
- OpenAPI specs in `artefacts/cloud_providers/{provider}/{api}/openapi.json`
- Generated resource types in `backends/foundation_deployment/src/providers/{provider}/resources/`

**Output:** 
- Rust client modules in `backends/foundation_deployment/src/providers/{provider}/clients/`

## Two-Tier Function Design

For each OpenAPI endpoint, generate **two functions**:

### Tier 1: Request Builder Function

Returns a `ClientRequestBuilder` that users can customize before sending:

```rust
// providers/gcp/clients/run.rs - Generated

use foundation_core::wire::simple_http::client::{ClientRequestBuilder, SimpleHttpClient};
use crate::providers::gcp::resources::run::*;

/// GET /apis/serving.k8s.io/v1/namespaces/{namespace}/services
/// List all Cloud Run services in a namespace.
///
/// Returns a `ClientRequestBuilder` for customization (auth, headers, etc.).
/// Use `list_services_execute()` for a ready-to-send version.
pub fn list_services_builder(
    client: &SimpleHttpClient,
    namespace: &str,
) -> Result<ClientRequestBuilder<SystemDnsResolver>, Error> {
    let url = format!(
        "https://api.gcp.io/run/v1/apis/serving.k8s.io/v1/namespaces/{}/services",
        namespace
    );
    
    client.get(&url)
        .map_err(|e| Error::RequestBuildFailed(e.to_string()))
}
```

**Use cases for builder functions:**
- Add custom authentication headers
- Add tracing/correlation IDs
- Modify timeouts per-request
- Add custom middleware

### Tier 2: Execute Function

Builds the request, sends it, and returns a `StreamIterator` with the transformed response:

```rust
// providers/gcp/clients/run.rs - Generated

use foundation_core::valtron::{execute, from_future, StreamIterator, StreamIteratorExt};
use foundation_core::wire::simple_http::client::{
    SendRequestTask, RequestIntro, body_reader, PreparedRequest,
};

/// GET /apis/serving.k8s.io/v1/namespaces/{namespace}/services
/// List all Cloud Run services in a namespace.
///
/// Builds the request, executes it, and returns the parsed response.
/// For request customization, use `list_services_builder()` instead.
pub fn list_services_execute(
    client: &SimpleHttpClient,
    namespace: &str,
) -> impl StreamIterator<
    D = Result<ApiResponse<ListServicesResponse>, Error>,
    P = ClientPending
> + Send + 'static {
    // Step 1: Build request using the builder function
    let builder_result = list_services_builder(client, namespace);
    
    // Step 2: Wrap in future for valtron execution
    let future = async move {
        let builder = builder_result?;
        let request = builder
            .send()
            .await
            .map_err(|e| Error::RequestSendFailed(e.to_string()))?;
        Ok(request)
    };
    
    // Step 3: Execute and transform response
    let task = from_future(future);
    execute(task, None)
        .map_iter_done(|request_result| {
            let prepared = match request_result {
                Ok(p) => p,
                Err(e) => return Some(Err(e)),
            };
            
            // Create SendRequestTask for HTTP execution
            let send_task = SendRequestTask::new(
                prepared,
                3, // retries
                pool,
                config
            )
            .map_ready(|intro| match intro {
                RequestIntro::Success { stream, status } => {
                    // Extract headers
                    let headers = status.headers().clone();
                    
                    // Check status
                    if !status.is_success() {
                        return Err(Error::HttpStatus {
                            code: status.as_u16(),
                            headers,
                        });
                    }
                    
                    // Parse body
                    let body = body_reader::collect_string(stream);
                    let parsed: ListServicesResponse = serde_json::from_str(&body)
                        .map_err(|e| Error::ParseFailed(e.to_string()))?;
                    
                    // Return full response with headers, status, body
                    Ok(ApiResponse {
                        status: status.as_u16(),
                        headers,
                        body: parsed,
                    })
                }
                RequestIntro::Failed(e) => Err(Error::RequestFailed(e.to_string())),
            })
            .map_pending(|_| ClientPending::Sending);
            
            Some(Ok(send_task))
        })
        .map_pending(|_| ClientPending::Building)
}
```

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

| OpenAPI | Builder Function | Execute Function |
|---------|-----------------|------------------|
| `GET /services` | `list_services_builder()` | `list_services_execute()` |
| `POST /services` | `create_service_builder()` | `create_service_execute()` |
| `GET /services/{name}` | `get_service_builder()` | `get_service_execute()` |
| `PUT /services/{name}` | `update_service_builder()` | `update_service_execute()` |
| `DELETE /services/{name}` | `delete_service_builder()` | `delete_service_execute()` |

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
└───────────┬────────────┘
            │
            ▼
┌────────────────────────┐
│ 2. Wrap in future      │
│    (async block)       │
└───────────┬────────────┘
            │
            ▼
┌────────────────────────┐
│ 3. from_future()       │
│    wraps in Task       │
└───────────┬────────────┘
            │
            ▼
┌────────────────────────┐
│ 4. execute()           │
│    schedules task      │
└───────────┬────────────┘
            │
            ▼
┌────────────────────────┐
│ 5. map_iter_done()     │
│    transforms to       │
│    SendRequestTask     │
└───────────┬────────────┘
            │
            ▼
┌────────────────────────┐
│ 6. map_ready()         │
│    handles response:   │
│    - Extract headers   │
│    - Check status      │
│    - Parse body        │
│    - Wrap in ApiResponse│
└───────────┬────────────┘
            │
            ▼
┌────────────────────────┐
│ 7. map_pending()       │
│    adds progress state │
└───────────┬────────────┘
            │
            ▼
┌────────────────────────┐
│ 8. collect_next_from_  │
│    streams()           │
│    flattens nested     │
│    StreamIterator      │
└───────────┬────────────┘
            │
            ▼
    StreamIterator yielded
    to user
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
    writeln!(out, "/// Use `{}_execute()` for ready-to-send version.", fn_name).unwrap();
    
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
    writeln!(out, "\n) -> Result<ClientRequestBuilder<SystemDnsResolver>, Error> {{").unwrap();
    
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
    writeln!(out, "/// Builds, sends, and parses response.").unwrap();
    writeln!(out, "/// For customization, use `{}_builder()` instead.", fn_name).unwrap();
    
    // Signature - same parameters as builder
    write!(out, "pub fn {}_execute(\n    client: &SimpleHttpClient,", fn_name).unwrap();
    // ... (same params as builder)
    
    // Return StreamIterator
    writeln!(out)
    writeln!(out, ") -> impl StreamIterator<").unwrap();
    writeln!(out, "    D = Result<ApiResponse<{}>, Error>,", return_type).unwrap();
    writeln!(out, "    P = ApiPending").unwrap();
    writeln!(out, "> + Send + 'static {{").unwrap();
    
    // Call builder
    writeln!(out, "    let builder = {}_builder(client, /* params */)?;", fn_name).unwrap();
    
    // Generate valtron combinator chain
    generate_valtron_chain(out, endpoint, return_type);
    
    writeln!(out, "}}\n").unwrap();
}
```

## Tasks

1. **Create generator infrastructure**
   - [ ] Add `gen_provider_clients` subcommand (or extend `gen_resource_types`)
   - [ ] Create endpoint extraction logic
   - [ ] Create parameter type mapping
   - [ ] Create response type mapping

2. **Implement builder function generation**
   - [ ] Generate URL building code
   - [ ] Generate query parameter handling
   - [ ] Generate request body serialization
   - [ ] Generate `ClientRequestBuilder` return

3. **Implement execute function generation**
   - [ ] Generate valtron task wrapping
   - [ ] Generate `SendRequestTask` integration
   - [ ] Generate response parsing
   - [ ] Generate `ApiResponse` wrapping
   - [ ] Generate error mapping

4. **Generate shared types**
   - [ ] Generate `ApiResponse<T>` wrapper
   - [ ] Generate `ApiError` enum
   - [ ] Generate `ApiPending` states
   - [ ] Generate module exports

5. **Generate for all providers**
   - [ ] GCP (all APIs)
   - [ ] Cloudflare
   - [ ] Stripe
   - [ ] Supabase
   - [ ] Neon
   - [ ] Fly.io
   - [ ] PlanetScale

6. **Verification**
   - [ ] All generated code compiles
   - [ ] Zero clippy warnings
   - [ ] Zero rustdoc warnings
   - [ ] Type-safe parameter passing
   - [ ] Response parsing works

## Success Criteria

- [ ] All 6 tasks completed
- [ ] Two functions per endpoint (builder + execute)
- [ ] Builder returns `ClientRequestBuilder`
- [ ] Execute returns `StreamIterator<ApiResponse<T>>`
- [ ] Generated code compiles with zero warnings
- [ ] All providers have generated clients

## Verification

```bash
cd /home/darkvoid/Boxxed/@dev/ewe_platform

# Generate all provider clients
cargo run --bin ewe_platform gen_provider_clients

# Verify compilation
cargo check -p foundation_deployment --features gcp,cloudflare,stripe

# Verify rustdoc
cargo doc -p foundation_deployment --no-deps

# Verify clippy
cargo clippy -p foundation_deployment -- -D warnings -W clippy::pedantic
```

## Example Usage

```rust
use crate::providers::gcp::clients::run::*;
use crate::providers::gcp::resources::run::*;
use foundation_core::valtron::Stream;

// Option 1: Use execute function for simple calls
let stream = list_services_execute(&client, "my-project");
for item in stream {
    match item {
        Stream::Pending(ApiPending::Building) => println!("Building request..."),
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

// Option 2: Use builder for customization
let builder = list_services_builder(&client, "my-project")?
    .header("X-Custom-Header", "value");

// Add auth, modify, then send manually
let request = builder.send().await?;
```

---

_Created: 2026-04-05_
_Updated: 2026-04-05 - Two-tier design (builder + execute functions)_
