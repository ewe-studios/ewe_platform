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
  uncompleted: 7
  total: 7
  completion_percentage: 0%
---


# Gen Provider Clients - API Client Code Generation from OpenAPI Specs

## Iron Law: Zero Warnings

> **All generated code must compile with zero warnings and pass all lints.**
>
> - Generated files must not require `#![allow(clippy::too_many_lines)]` or similar suppressions
> - All doc comments must be valid rustdoc
> - `cargo doc -p foundation_deployment --no-deps` — zero warnings from generated files
> - `cargo clippy -p foundation_deployment -- -D warnings -W clippy::pedantic` — zero warnings

## Overview

The `gen_provider_clients` tool generates **type-safe API client functions** from OpenAPI specifications. These clients use the generated resource types (Feature 20) and `SimpleHttpClient` to provide a fluent, ergonomic API for interacting with cloud provider APIs.

**Input:** 
- OpenAPI specs in `artefacts/cloud_providers/{provider}/{api}/openapi.json`
- Generated resource types in `backends/foundation_deployment/src/providers/{provider}/resources/`

**Output:** 
- Rust client modules in `backends/foundation_deployment/src/providers/{provider}/clients/`

The tool generates:
1. **Client structs** - One per API service (e.g., `CloudRunClient`, `ComputeClient`)
2. **Request builders** - Fluent builders for each endpoint with type-safe parameters
3. **Response handlers** - Transform HTTP responses using generated resource types
4. **StreamIterators** - Use valtron combinators for async streaming operations

## Directory Structure

```
backends/foundation_deployment/src/providers/{provider}/
├── mod.rs
├── provider.rs
├── fetch.rs
├── resources/
│   └── mod.rs              # Generated resource types
└── clients/
    ├── mod.rs              # Generated client module declarations
    ├── {api}.rs            # Generated client for each API
    └── shared.rs           # Shared client utilities
```

### Example: GCP Cloud Run Client

```rust
// providers/gcp/clients/run.rs - Generated

use crate::providers::gcp::resources::run::*;
use foundation_core::valtron::{execute, from_future, StreamIterator, StreamIteratorExt};
use foundation_core::wire::simple_http::client::{
    SimpleHttpClient, ClientConfig, HttpConnectionPool, SystemDnsResolver, body_reader,
};
use std::sync::Arc;

/// Client for Google Cloud Run API.
pub struct CloudRunClient {
    client: SimpleHttpClient,
    base_url: String,
}

impl CloudRunClient {
    /// Create a new Cloud Run client.
    pub fn new(client: SimpleHttpClient) -> Self {
        Self {
            client,
            base_url: "https://api.gcp.io/run/v1".to_string(),
        }
    }

    /// GET /apis/serving.k8s.io/v1/namespaces/{namespace}/services
    /// List all Cloud Run services in a namespace.
    pub fn list_services(
        &self,
        namespace: String,
    ) -> impl StreamIterator<D = Result<ListServicesResponse, CloudRunError>, P = CloudRunPending> + Send + 'static {
        let url = format!("{}/apis/serving.k8s.io/v1/namespaces/{}/services", 
                          self.base_url, namespace);
        
        let future = async move {
            // Build request
            let request = self.client
                .get(&url)
                .map_err(|e| CloudRunError::RequestBuildFailed(e.to_string()))?;
            
            // Execute and transform response
            // Uses valtron SendRequestTask combinator internally
            // ...
        };
        
        let task = from_future(future);
        execute(task, None)
            .map_pending(|_| CloudRunPending::FetchingServices)
    }

    /// POST /apis/serving.k8s.io/v1/namespaces/{namespace}/services
    /// Create a new Cloud Run service.
    pub fn create_service(
        &self,
        namespace: String,
        service: Service,
    ) -> impl StreamIterator<D = Result<Service, CloudRunError>, P = CloudRunPending> + Send + 'static {
        // Implementation using valtron combinators
    }

    /// PUT /apis/serving.k8s.io/v1/namespaces/{namespace}/services/{name}
    /// Update an existing Cloud Run service.
    pub fn update_service(
        &self,
        namespace: String,
        name: String,
        service: Service,
    ) -> impl StreamIterator<D = Result<Service, CloudRunError>, P = CloudRunPending> + Send + 'static {
        // Implementation
    }

    /// DELETE /apis/serving.k8s.io/v1/namespaces/{namespace}/services/{name}
    /// Delete a Cloud Run service.
    pub fn delete_service(
        &self,
        namespace: String,
        name: String,
    ) -> impl StreamIterator<D = Result<DeleteResponse, CloudRunError>, P = CloudRunPending> + Send + 'static {
        // Implementation
    }
}

/// Progress states for Cloud Run operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloudRunPending {
    FetchingServices,
    CreatingService,
    UpdatingService,
    DeletingService,
}

/// Cloud Run API errors.
#[derive(Debug)]
pub enum CloudRunError {
    RequestBuildFailed(String),
    HttpStatus { code: u16, message: String },
    ParseFailed(String),
}
```

## Client Generation Rules

### 1. One Client Per API Service

For multi-API providers (GCP, AWS), generate one client module per API:

| Provider | API | Client Module |
|----------|-----|---------------|
| GCP | Cloud Run | `clients/run.rs` |
| GCP | Compute | `clients/compute.rs` |
| GCP | Cloud Storage | `clients/storage.rs` |
| AWS | Lambda | `clients/lambda.rs` |
| Cloudflare | Workers | `clients/workers.rs` |

### 2. One Method Per Endpoint

Each OpenAPI path + HTTP method combination becomes a client method:

| OpenAPI Path | Method | Generated Method |
|--------------|--------|------------------|
| `/services` | GET | `list_services()` |
| `/services` | POST | `create_service()` |
| `/services/{name}` | GET | `get_service(name)` |
| `/services/{name}` | PUT | `update_service(name)` |
| `/services/{name}` | DELETE | `delete_service(name)` |

### 3. Type-Safe Parameters

Path parameters and query parameters become method arguments:

```rust
// OpenAPI: GET /projects/{project}/locations/{location}/services/{name}
pub fn get_service(
    &self,
    project: String,
    location: String,
    name: String,
) -> impl StreamIterator<D = Result<Service, Error>, P = Pending>
```

### 4. Request Bodies Use Resource Types

Request body schemas reference generated resource types:

```rust
// OpenAPI: POST /services with body: Service
pub fn create_service(
    &self,
    namespace: String,
    service: Service,  // From resources::run::Service
) -> impl StreamIterator<D = Result<Service, Error>, P = Pending>
```

### 5. Response Types Use Resource Types

Response schemas reference generated resource types:

```rust
// OpenAPI: 200 response with schema: Service
// Returns: Result<Service, Error>

// OpenAPI: 200 response with schema: ListServicesResponse  
// Returns: Result<ListServicesResponse, Error>
```

## Valtron Combinator Usage

All client methods return `StreamIterator` using valtron combinators:

```rust
use foundation_core::valtron::{
    execute, from_future, StreamIterator, StreamIteratorExt,
    TaskIteratorExt, TaskShortCircuit, TaskStatus,
};
use foundation_core::wire::simple_http::client::{
    SendRequestTask, RequestIntro, body_reader,
};

pub fn get_service(
    &self,
    name: String,
) -> impl StreamIterator<D = Result<Service, Error>, P = Pending> {
    let url = format!("{}/services/{}", self.base_url, name);
    
    // Build request in future
    let future = async move {
        let request = self.client.get(&url)?;
        Ok(request)
    };
    
    // Wrap in valtron task
    let task = from_future(future);
    
    // Execute and chain combinators
    let stream = execute(task, None)
        .map_iter_done(move |request_result| {
            let request = match request_result {
                Ok(r) => r,
                Err(e) => return Some(Err(e)),
            };
            
            // Create SendRequestTask for HTTP execution
            let send_task = SendRequestTask::new(
                request, 
                3, // retries
                self.pool.clone(),
                self.config.clone()
            )
            .map_ready(|intro| match intro {
                RequestIntro::Success { stream, status } => {
                    if !status.is_success() {
                        return Err(Error::HttpStatus { 
                            code: status.as_u16(),
                            message: "Request failed".to_string()
                        });
                    }
                    
                    // Parse response body
                    let body = body_reader::collect_string(stream);
                    serde_json::from_str::<Service>(&body)
                        .map_err(|e| Error::ParseFailed(e.to_string()))
                }
                RequestIntro::Failed(e) => Err(Error::RequestFailed(e.to_string())),
            })
            .map_pending(|_| Pending::Fetching);
            
            // Return single-item iterator
            Some(Ok(send_task))
        })
        .map_pending(|_| Pending::BuildingRequest);
    
    // Flatten nested StreamIterator
    collect_next_from_streams(stream)
}
```

## Provider-Specific Generators

Some providers have unique requirements. Use provider-specific generator modules when needed:

### Generic Generator (Default)

For most providers, the generic generator in `gen_resource_types/mod.rs` handles everything:

```rust
// bin/platform/src/gen_resource_types/mod.rs

impl ResourceGenerator {
    pub fn generate_clients(&self, provider: &str) -> Result<(), GenResourceError> {
        // Generic implementation
    }
}
```

### Provider-Specific Generator

For providers with unique auth or request patterns:

```rust
// providers/gcp/clients/generator.rs

pub struct GcpClientGenerator {
    // GCP-specific configuration
}

impl GcpClientGenerator {
    pub fn generate(&self, spec: &Value) -> Result<String, GenResourceError> {
        // GCP-specific client generation
        // - Handles OAuth2 token injection
        // - Generates methods for long-running operations
        // - Handles GCP-specific response patterns
    }
}
```

### When to Use Provider-Specific Generators

| Provider | Use Custom Generator? | Reason |
|----------|----------------------|--------|
| GCP | Yes | OAuth2, long-running operations, API versioning |
| AWS | Yes | SigV4 signing, request format |
| Cloudflare | No | Standard REST API |
| Stripe | No | Standard REST API |
| Supabase | No | Standard REST API |

## Implementation

### Step 1: Parse OpenAPI Endpoints

```rust
fn extract_endpoints(spec: &Value) -> Vec<ApiEndpoint> {
    let mut endpoints = Vec::new();
    
    // Handle OpenAPI 3.x: paths object
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
                        summary: operation.get("summary")
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

### Step 2: Generate Client Struct

```rust
fn generate_client_struct(out: &mut String, client_name: &str) {
    writeln!(out, "/// Client for {} API.", client_name).unwrap();
    writeln!(out, "#[derive(Clone)]").unwrap();
    writeln!(out, "pub struct {}Client {{", client_name).unwrap();
    writeln!(out, "    client: SimpleHttpClient,").unwrap();
    writeln!(out, "    base_url: String,").unwrap();
    writeln!(out, "}}").unwrap();
}
```

### Step 3: Generate Methods

```rust
fn generate_method(
    out: &mut String,
    endpoint: &ApiEndpoint,
    resource_types: &HashMap<String, String>,
) {
    let method_name = to_snake_case(&endpoint.operation_id.clone()
        .unwrap_or_else(|| format!("{}_{}", endpoint.method, endpoint.path)));
    
    // Generate doc comment
    writeln!(out, "/// {} {}", endpoint.method, endpoint.path).unwrap();
    if let Some(summary) = &endpoint.summary {
        writeln!(out, "/// {}", sanitize_doc_comment(summary)).unwrap();
    }
    
    // Generate signature
    write!(out, "pub fn {}(&self", method_name).unwrap();
    
    // Add path/query parameters
    for param in &endpoint.parameters {
        write!(out, ", {}: {}", to_snake_case(&param.name), param.rust_type).unwrap();
    }
    
    // Add request body if present
    if let Some(body_type) = &endpoint.request_body {
        write!(out, ", body: {}", body_type).unwrap();
    }
    
    // Return type
    let return_type = get_return_type(&endpoint.responses, resource_types);
    writeln!(out, ") -> impl StreamIterator<D = {}, P = {}Pending>", 
             return_type, client_name).unwrap();
    
    // Generate implementation using valtron combinators
    // ...
}
```

## Tasks

1. **Create generator infrastructure**
   - [ ] Add `gen_provider_clients` subcommand to platform CLI
   - [ ] Create generator module structure
   - [ ] Implement OpenAPI endpoint extraction
   - [ ] Implement parameter type mapping

2. **Implement generic client generator**
   - [ ] Generate client structs
   - [ ] Generate methods for each endpoint
   - [ ] Generate response type mappings
   - [ ] Generate progress state enums
   - [ ] Generate error types

3. **Implement valtron integration**
   - [ ] Use `from_future` for async operations
   - [ ] Use `SendRequestTask` for HTTP execution
   - [ ] Use `map_ready` for response transformation
   - [ ] Use `collect_next_from_streams` for flattening

4. **Implement provider-specific generators**
   - [ ] Create GCP generator with OAuth2 support
   - [ ] Create AWS generator with SigV4 signing
   - [ ] Keep generic generator for simple providers

5. **Generate clients for all providers**
   - [ ] GCP (all APIs)
   - [ ] Cloudflare
   - [ ] Stripe
   - [ ] Supabase
   - [ ] Neon
   - [ ] Fly.io
   - [ ] PlanetScale

6. **Integration tests**
   - [ ] Test client compilation with all features
   - [ ] Test type-safe parameter passing
   - [ ] Test response parsing
   - [ ] Test error handling

7. **Documentation**
   - [ ] Document generator architecture
   - [ ] Add usage examples
   - [ ] Document valtron combinator patterns

## Success Criteria

- [ ] All 7 tasks completed
- [ ] Generated clients compile with zero warnings
- [ ] All client methods return proper StreamIterator
- [ ] Type-safe parameter and response handling
- [ ] Provider-specific generators for GCP and AWS
- [ ] Documentation complete

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
use crate::providers::gcp::clients::run::CloudRunClient;
use crate::providers::gcp::resources::run::Service;
use foundation_core::valtron::StreamIteratorExt;

// Create client
let client = CloudRunClient::new(http_client);

// List services - returns StreamIterator
let stream = client.list_services("my-project".to_string());

// Consume stream (synchronous iteration)
for item in stream {
    match item {
        Stream::Pending(CloudRunPending::FetchingServices) => {
            println!("Fetching services...");
        }
        Stream::Next(Ok(response)) => {
            for service in response.items {
                println!("Service: {}", service.metadata.name);
            }
        }
        Stream::Next(Err(e)) => {
            eprintln!("Error: {:?}", e);
        }
    }
}
```

---

_Created: 2026-04-05_
