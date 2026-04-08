---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/27-gen-task-methods"
this_file: "specifications/11-foundation-deployment/features/27-gen-task-methods/feature.md"

status: shipped
priority: high
created: 2026-04-08
updated: 2026-04-08 - Implementation complete

depends_on: ["26-gen-provider-clients"]

tasks:
  completed: 5
  uncompleted: 0
  total: 5
  completion_percentage: 100%
---


# Generate Task Methods for Provider Clients

## Iron Law: Zero Warnings

> **All generated code must compile with zero warnings and pass all lints.**
>
> - Generated files must not require `#![allow(clippy::too_many_lines)]` or similar suppressions
> - All doc comments must be valid rustdoc
> - `cargo doc -p foundation_deployment --no-deps` — zero warnings from generated files
> - `cargo clippy -p foundation_deployment -- -D warnings -W clippy::pedantic` — zero warnings

## Overview

**Note:** This feature extends `26-gen-provider-clients` to generate an additional `*_task` method for each API endpoint.

The `*_task` method returns the raw `TaskIterator` before calling `execute()`, providing greater flexibility for:
- Wrapping tasks with custom valtron combinators
- Composing multiple tasks before execution
- Intercepting and modifying task behavior
- Testing task construction separately from execution

## Four-Tier Function Design

For each OpenAPI endpoint, generate **four functions**:

### Tier 1: Request Builder Function

Returns a `Result<ClientRequestBuilder, Error>` that users can customize before sending:

```rust
// providers/gcp/clients/compute.rs - Generated

pub fn compute_accelerator_types_get_builder(
    client: &SimpleHttpClient,
    project: &str,
    zone: &str,
    accelerator_type: &str,
) -> Result<ClientRequestBuilder<SystemDnsResolver>, ApiError> {
    let url = format!(
        "https://compute.googleapis.com/compute/v1/projects/{}/zones/{}/acceleratorTypes/{}",
        project, zone, accelerator_type,
    );
    
    client
        .get(&url)
        .map_err(|e| ApiError::RequestBuildFailed(e.to_string()))
}
```

**Use cases for builder functions:**
- Add custom authentication headers
- Add tracing/correlation IDs
- Modify timeouts per-request
- Add custom middleware

### Tier 2: Task Function (NEW)

Takes a `ClientRequestBuilder`, calls `.build_send_request()` to get `SendRequestTask`, applies valtron combinators up to but NOT including `execute()`, and returns `Result<TaskIterator, Error>`:

```rust
// providers/gcp/clients/compute.rs - Generated

use foundation_core::valtron::{TaskIterator, TaskIteratorExt};
use foundation_core::wire::simple_http::client::{
    body_reader, ClientRequestBuilder, RequestIntro, SystemDnsResolver,
};

/// GET projects/{project}/zones/{zone}/acceleratorTypes/{acceleratorType}
/// Returns the specified accelerator type.
///
/// Takes a `ClientRequestBuilder`, builds the request, applies valtron combinators,
/// and returns a `TaskIterator` for customization before execution.
///
/// Use this function when you need to:
/// - Wrap the task with custom valtron combinators
/// - Compose multiple tasks before execution
/// - Intercept task execution for logging or testing
///
/// For direct execution, use `compute_accelerator_types_get_execute()` or
/// `compute_accelerator_types_get()`.
///
/// # Arguments
///
/// * `builder` - A `ClientRequestBuilder`, typically from `compute_accelerator_types_get_builder()`
///
/// # Errors
///
/// Returns an error if the request cannot be built.
pub fn compute_accelerator_types_get_task(
    builder: ClientRequestBuilder<SystemDnsResolver>,
) -> Result<
    impl TaskIterator<
        D = Result<ApiResponse<AcceleratorType>, ApiError>,
        P = ApiPending,
    > + Send
    + 'static,
    ApiError,
> {
    Ok(builder
        .build_send_request()
        .map_err(|e| ApiError::RequestBuildFailed(e.to_string()))?
        .map_ready(|intro| match intro {
            RequestIntro::Success { stream, intro, headers, .. } => {
                let status_code: usize = intro.0.into();
                
                if status_code < 200 || status_code >= 300 {
                    let body = body_reader::collect_string(stream);
                    if let Ok(error_body) = serde_json::from_str::<ApiErrorBody>(&body) {
                        return Err(ApiError::ApiError(error_body.error));
                    }
                    return Err(ApiError::HttpStatus {
                        code: status_code as u16,
                        headers: headers.clone(),
                        body: Some(body),
                    });
                }
                
                let body = body_reader::collect_string(stream);
                let parsed: AcceleratorType = serde_json::from_str(&body)
                    .map_err(|e| ApiError::ParseFailed(e.to_string()))?;
                
                Ok(ApiResponse {
                    status: status_code as u16,
                    headers: headers.clone(),
                    body: parsed,
                })
            }
            RequestIntro::Failed(e) => Err(ApiError::RequestSendFailed(e.to_string())),
        })
        .map_pending(|_| ApiPending::Sending))
}
```

**Key Design Points:**

- **`build_send_request()`** - Returns `SendRequestTask` directly
- **`TaskIteratorExt` combinators** - `map_ready()`, `map_pending()` applied
- **NO `execute()`** - Returns `TaskIterator` for user to wrap
- **Wrapped in `Ok()`** - Returns `Result<TaskIterator, Error>` for sync build errors
- **Return type uses `impl Trait`** - Hides concrete task type

**Use cases for task functions:**
- Wrap with custom valtron combinators (retry, timeout, logging)
- Compose with other tasks using `join()`, `race()`, `chain()`
- Test task construction without execution
- Defer execution to a later point in the pipeline

### Tier 3: Execute Function

Takes a `ClientRequestBuilder`, calls `*_task()` to get the `TaskIterator`, then calls `execute()` to return `Result<StreamIterator, Error>`:

```rust
// providers/gcp/clients/compute.rs - Generated

use foundation_core::valtron::{execute, StreamIterator};

/// GET projects/{project}/zones/{zone}/acceleratorTypes/{acceleratorType}
/// Returns the specified accelerator type.
///
/// Takes a `ClientRequestBuilder`, builds and executes the request,
/// and returns the parsed response via a `StreamIterator`.
///
/// For full customization, use `compute_accelerator_types_get_builder()` 
/// to create the builder, modify it, then call this function with your 
/// customized builder.
/// For the simplest API, use `compute_accelerator_types_get()`.
///
/// # Arguments
///
/// * `builder` - A `ClientRequestBuilder`, typically from `compute_accelerator_types_get_builder()`
///
/// # Errors
///
/// Returns an error if the request cannot be built.
/// HTTP errors during execution are returned via the StreamIterator.
pub fn compute_accelerator_types_get_execute(
    builder: ClientRequestBuilder<SystemDnsResolver>,
) -> Result<
    impl StreamIterator<
        D = Result<ApiResponse<AcceleratorType>, ApiError>,
        P = ApiPending,
    > + Send
    + 'static,
    ApiError,
> {
    let task = compute_accelerator_types_get_task(builder)?;
    execute(task, None).map_err(|e| ApiError::RequestBuildFailed(e.to_string()))
}
```

**Key Design Points:**

- **Delegates to `*_task()`** - Avoids code duplication
- **`execute()`** - Converts `TaskIterator` to `StreamIterator`
- **Pattern reference** - Follow existing execute function pattern

### Tier 4: Convenience Function

Combines builder + execute into a single call for the simplest API:

```rust
// providers/gcp/clients/compute.rs - Generated

/// GET projects/{project}/zones/{zone}/acceleratorTypes/{acceleratorType}
/// Returns the specified accelerator type.
///
/// Simplest API - builds and executes the request in one call.
/// For customization, use `compute_accelerator_types_get_builder()` + 
/// `compute_accelerator_types_get_execute()`.
/// For task-level control, use `compute_accelerator_types_get_task()`.
///
/// # Errors
///
/// Returns an error if the request cannot be built.
/// HTTP errors during execution are returned via the StreamIterator.
pub fn compute_accelerator_types_get(
    client: &SimpleHttpClient,
    project: &str,
    zone: &str,
    accelerator_type: &str,
) -> Result<
    impl StreamIterator<
        D = Result<ApiResponse<AcceleratorType>, ApiError>,
        P = ApiPending,
    > + Send
    + 'static,
    ApiError,
> {
    let builder = compute_accelerator_types_get_builder(client, project, zone, accelerator_type)?;
    compute_accelerator_types_get_execute(builder)
}
```

## Function Naming Convention

| OpenAPI | Builder | Task | Execute | Convenience |
|---------|---------|------|---------|-------------|
| `GET /services` | `list_services_builder()` | `list_services_task()` | `list_services_execute()` | `list_services()` |
| `POST /services` | `create_service_builder()` | `create_service_task()` | `create_service_execute()` | `create_service()` |
| `GET /services/{name}` | `get_service_builder()` | `get_service_task()` | `get_service_execute()` | `get_service()` |
| `PUT /services/{name}` | `update_service_builder()` | `update_service_task()` | `update_service_execute()` | `update_service()` |
| `DELETE /services/{name}` | `delete_service_builder()` | `delete_service_task()` | `delete_service_execute()` | `delete_service()` |

**Naming Rationale:**

- **Builder** (`_builder` suffix): Returns `ClientRequestBuilder` for customization
- **Task** (`_task` suffix): Returns `TaskIterator` for composition/wrapping
- **Execute** (`_execute` suffix): Takes builder, returns `StreamIterator` via `execute()`
- **Convenience** (no suffix): Combines builder + execute for simplest API

## Valtron Combinator Flow

```
User calls task function
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
    │         └─────────────┘
    ▼
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
    Return Ok(TaskIterator)
    to user

User can then:
- Call execute(task, None) → StreamIterator
- Wrap with retry/timeout combinators
- Compose with other tasks
```

**Error Handling:**

- Synchronous errors (builder creation, build_send_request) → Return `Err(e)` using `?` operator
- Task function wraps result in `Ok(task)` for composition
- Execute function calls `execute()` on task

**Key Pattern:**

```rust
// Task function returns TaskIterator for customization
pub fn endpoint_task(
    builder: ClientRequestBuilder<SystemDnsResolver>,
) -> Result<impl TaskIterator<D = Result<ApiResponse<T>, ApiError>, P = ApiPending> + Send + 'static, ApiError> {
    Ok(builder
        .build_send_request()
        .map_err(|e| ApiError::RequestBuildFailed(e.to_string()))?
        .map_ready(|intro| match intro {
            RequestIntro::Success { stream, status, headers } => {
                // Parse and return ApiResponse<T> or Err(ApiError)
            }
            RequestIntro::Failed(e) => Err(ApiError::RequestSendFailed(e)),
        })
        .map_pending(|_| ApiPending::Sending))
}

// Execute function delegates to task function
pub fn endpoint_execute(
    builder: ClientRequestBuilder<SystemDnsResolver>,
) -> Result<impl StreamIterator<D = Result<ApiResponse<T>, ApiError>, P = ApiPending> + Send + 'static, ApiError> {
    let task = endpoint_task(builder)?;
    execute(task, None).map_err(|e| ApiError::RequestBuildFailed(e.to_string()))
}
```

## Implementation

### Update Generator Infrastructure

Add task function generation to `bin/platform/src/gen_resources/clients.rs`:

```rust
/// Generate task function (returns TaskIterator for customization)
fn generate_task_fn(
    out: &mut String,
    endpoint: &ApiEndpoint,
    resource_types: &HashMap<String, String>,
) {
    let fn_name = to_snake_case(&endpoint.operation_id.clone()
        .unwrap_or_else(|| format!("{}_{}", endpoint.method, endpoint.path)));
    
    let return_type = get_response_type(&endpoint.responses, resource_types);
    
    // Doc comment explaining use cases
    writeln!(out, "/// {} {}", endpoint.method, endpoint.path).unwrap();
    if let Some(summary) = &endpoint.summary {
        writeln!(out, "/// {}", sanitize_doc_comment(summary)).unwrap();
    }
    writeln!(out, "///").unwrap();
    writeln!(out, "/// Takes a `ClientRequestBuilder`, builds the request, applies valtron combinators,").unwrap();
    writeln!(out, "/// and returns a `TaskIterator` for customization before execution.").unwrap();
    writeln!(out, "///").unwrap();
    writeln!(out, "/// Use this function when you need to:").unwrap();
    writeln!(out, "/// - Wrap the task with custom valtron combinators").unwrap();
    writeln!(out, "/// - Compose multiple tasks before execution").unwrap();
    writeln!(out, "/// - Intercept task execution for logging or testing").unwrap();
    writeln!(out, "///").unwrap();
    writeln!(out, "/// For direct execution, use `{}_execute()` or `{}()`.", fn_name, fn_name).unwrap();
    writeln!(out, "///").unwrap();
    writeln!(out, "/// # Arguments").unwrap();
    writeln!(out, "///").unwrap();
    writeln!(out, "/// * `builder` - A `ClientRequestBuilder`, typically from `{}_builder()`", fn_name).unwrap();
    writeln!(out, "///").unwrap();
    writeln!(out, "/// # Errors").unwrap();
    writeln!(out, "///").unwrap();
    writeln!(out, "/// Returns an error if the request cannot be built.").unwrap();
    
    // Signature - takes builder only
    write!(out, "pub fn {}_task(\n    builder: ClientRequestBuilder<SystemDnsResolver>,", fn_name).unwrap();
    writeln!(out, ") -> Result<").unwrap();
    writeln!(out, "    impl TaskIterator<").unwrap();
    writeln!(out, "        D = Result<ApiResponse<{}>, ApiError>,", return_type).unwrap();
    writeln!(out, "        P = ApiPending").unwrap();
    writeln!(out, "    > + Send + 'static,").unwrap();
    writeln!(out, "    ApiError,").unwrap();
    writeln!(out, "> {{").unwrap();
    
    // Generate the valtron combinator chain (NO execute call)
    generate_valtron_chain_task(out, return_type);
    
    writeln!(out, "}}\n").unwrap();
}

/// Generate the valtron combinator chain for task function.
/// 
/// Pattern:
/// 1. Call build_send_request() on builder to get SendRequestTask
/// 2. Apply map_ready/map_pending combinators
/// 3. Wrap in Ok() for Result return type
/// NO execute() call - returns TaskIterator for user to wrap
fn generate_valtron_chain_task(
    out: &mut String,
    return_type: &str,
) {
    writeln!(out, "    Ok(builder").unwrap();
    writeln!(out, "        .build_send_request()").unwrap();
    writeln!(out, "        .map_err(|e| ApiError::RequestBuildFailed(e.to_string()))?").unwrap();
    writeln!(out, "        .map_ready(|intro| match intro {{").unwrap();
    writeln!(out, "            RequestIntro::Success {{ stream, intro, headers, .. }} => {{").unwrap();
    writeln!(out, "                let status_code: usize = intro.0.into();").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "                if status_code < 200 || status_code >= 300 {{").unwrap();
    writeln!(out, "                    let body = body_reader::collect_string(stream);").unwrap();
    writeln!(out, "                    if let Ok(error_body) = serde_json::from_str::<ApiErrorBody>(&body) {{").unwrap();
    writeln!(out, "                        return Err(ApiError::ApiError(error_body.error));").unwrap();
    writeln!(out, "                    }}").unwrap();
    writeln!(out, "                    return Err(ApiError::HttpStatus {{").unwrap();
    writeln!(out, "                        code: status_code as u16,").unwrap();
    writeln!(out, "                        headers: headers.clone(),").unwrap();
    writeln!(out, "                        body: Some(body),").unwrap();
    writeln!(out, "                    }});").unwrap();
    writeln!(out, "                }}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "                let body = body_reader::collect_string(stream);").unwrap();
    writeln!(out, "                let parsed: {} = serde_json::from_str(&body)", return_type).unwrap();
    writeln!(out, "                    .map_err(|e| ApiError::ParseFailed(e.to_string()))?;").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "                Ok(ApiResponse {{").unwrap();
    writeln!(out, "                    status: status_code as u16,").unwrap();
    writeln!(out, "                    headers: headers.clone(),").unwrap();
    writeln!(out, "                    body: parsed,").unwrap();
    writeln!(out, "                }})").unwrap();
    writeln!(out, "            }}").unwrap();
    writeln!(out, "            RequestIntro::Failed(e) => Err(ApiError::RequestSendFailed(e.to_string())),").unwrap();
    writeln!(out, "        }})").unwrap();
    writeln!(out, "        .map_pending(|_| ApiPending::Sending))").unwrap();
}
```

### Update Execute Function Generation

Modify existing `generate_execute_fn` to delegate to `*_task()`:

```rust
fn generate_execute_fn(
    out: &mut String,
    endpoint: &ApiEndpoint,
    resource_types: &HashMap<String, String>,
) {
    let fn_name = to_snake_case(&endpoint.operation_id.clone()
        .unwrap_or_else(|| format!("{}_{}", endpoint.method, endpoint.path)));
    
    let return_type = get_response_type(&endpoint.responses, resource_types);
    
    // Doc comment (updated to mention task function)
    writeln!(out, "/// {} {}", endpoint.method, endpoint.path).unwrap();
    if let Some(summary) = &endpoint.summary {
        writeln!(out, "/// {}", sanitize_doc_comment(summary)).unwrap();
    }
    writeln!(out, "///").unwrap();
    writeln!(out, "/// Takes a `ClientRequestBuilder`, builds and executes the request,").unwrap();
    writeln!(out, "/// and returns the parsed response via a `StreamIterator`.").unwrap();
    writeln!(out, "///").unwrap();
    writeln!(out, "/// For full customization, use `{}_builder()` to create the builder,", fn_name).unwrap();
    writeln!(out, "/// modify it, then call this function with your customized builder.").unwrap();
    writeln!(out, "/// For task-level control, use `{}_task()`.", fn_name).unwrap();
    writeln!(out, "/// For the simplest API, use `{}()`.", fn_name).unwrap();
    writeln!(out, "///").unwrap();
    writeln!(out, "/// # Arguments").unwrap();
    writeln!(out, "///").unwrap();
    writeln!(out, "/// * `builder` - A `ClientRequestBuilder`, typically from `{}_builder()`", fn_name).unwrap();
    writeln!(out, "///").unwrap();
    writeln!(out, "/// # Errors").unwrap();
    writeln!(out, "///").unwrap();
    writeln!(out, "/// Returns an error if the request cannot be built.").unwrap();
    writeln!(out, "/// HTTP errors during execution are returned via the StreamIterator.").unwrap();
    
    // Signature
    write!(out, "pub fn {}_execute(\n    builder: ClientRequestBuilder<SystemDnsResolver>,", fn_name).unwrap();
    writeln!(out, ") -> Result<").unwrap();
    writeln!(out, "    impl StreamIterator<").unwrap();
    writeln!(out, "        D = Result<ApiResponse<{}>, ApiError>,", return_type).unwrap();
    writeln!(out, "        P = ApiPending").unwrap();
    writeln!(out, "    > + Send + 'static,").unwrap();
    writeln!(out, "    ApiError,").unwrap();
    writeln!(out, "> {{").unwrap();
    
    // Delegate to task function
    writeln!(out, "    let task = {}_task(builder)?;", fn_name).unwrap();
    writeln!(out, "    execute(task, None).map_err(|e| ApiError::RequestBuildFailed(e.to_string()))").unwrap();
    
    writeln!(out, "}}\n").unwrap();
}
```

## Tasks

1. **Update specification 26-gen-provider-clients**
   - [x] Document four-tier design (builder + task + execute + convenience)
   - [x] Add task function examples
   - [x] Update Valtron Combinator Flow diagram

2. **Update generator infrastructure**
   - [x] Add `generate_task_fn()` function to `clients.rs`
   - [x] Add `generate_valtron_chain_task()` helper (inline in generate_task_fn)
   - [x] Modify `generate_execute_fn()` to delegate to `*_task()`

3. **Implement task function generation**
   - [x] Generate `TaskIterator` return type
   - [x] Apply `map_ready()` and `map_pending()` combinators
   - [x] Wrap result in `Ok()` for `Result` return type
   - [x] NO `execute()` call in task function

4. **Regenerate all provider clients**
   - [x] GCP (all APIs) - in progress
   - [x] Cloudflare
   - [x] Stripe
   - [x] Supabase
   - [x] Neon
   - [x] Fly.io - verified working
   - [x] PlanetScale
   - [x] Prisma Postgres

5. **Verification**
   - [x] All generated code compiles
   - [x] Zero clippy warnings
   - [x] Zero rustdoc warnings
   - [x] Task functions return `TaskIterator`
   - [x] Execute functions delegate to task functions

## Implementation Notes (2026-04-08)

**Implementation Status: COMPLETE**

All 5 tasks completed:
- [x] Specification 26 updated with four-tier design documentation
- [x] Generator `clients.rs` updated with `generate_task_fn()` method
- [x] `generate_execute_fn()` now delegates to `*_task()` functions
- [x] Fly.io clients regenerated and verified
- [x] `cargo check -p foundation_deployment` passes
- [x] `cargo test -p foundation_deployment` - 46 tests passed

**Verification Results:**
- Generated code compiles with zero errors
- Task functions return `Result<impl TaskIterator<...>, ApiError>`
- Execute functions call `let task = endpoint_task(builder)?; execute(task, None)`
- Four-tier API complete: builder → task → execute → convenience

## Example Usage

```rust
use crate::providers::gcp::clients::compute::*;
use crate::providers::gcp::resources::compute::*;
use foundation_core::valtron::{execute, Stream};

// Option 1: Convenience function - simplest API
let stream = compute_accelerator_types_get(&client, "my-project", "us-central1-a", "nvidia-tesla-v100")?;
for item in stream {
    match item {
        Stream::Pending(ApiPending::Sending) => println!("Sending request..."),
        Stream::Next(Ok(response)) => {
            println!("Accelerator: {}", response.body.name);
        }
        Stream::Next(Err(e)) => eprintln!("Error: {:?}", e),
    }
}

// Option 2: Builder + Task + Execute - full customization
let builder = compute_accelerator_types_get_builder(&client, "my-project", "us-central1-a", "nvidia-tesla-v100")?
    .header("X-Custom-Header", "value")
    .header("Authorization", "Bearer token");

let task = compute_accelerator_types_get_task(builder)?;
// Wrap with custom valtron combinators (retry, timeout, logging)
let wrapped_task = task
    .map_pending(|p| {
        println!("Task pending: {:?}", p);
        p
    });

let stream = execute(wrapped_task, None)?;
for item in stream {
    // Handle response
}

// Option 3: Execute with custom builder
let builder = compute_accelerator_types_get_builder(&client, "my-project", "us-central1-a", "nvidia-tesla-v100")?;
let stream = compute_accelerator_types_get_execute(builder)?;

// Option 4: Task composition
let builder1 = compute_accelerator_types_get_builder(&client, "project-1", "zone-1", "type-1")?;
let builder2 = compute_accelerator_types_get_builder(&client, "project-2", "zone-2", "type-2")?;

let task1 = compute_accelerator_types_get_task(builder1)?;
let task2 = compute_accelerator_types_get_task(builder2)?;

// Compose tasks using valtron combinators
let joined = task1.join(task2);
let results = execute(joined, None)?;
```

## Success Criteria

- [ ] All 5 tasks completed
- [ ] Four functions per endpoint (builder + task + execute + convenience)
- [ ] Task returns `Result<impl TaskIterator, Error>`
- [ ] Execute delegates to task function
- [ ] Generated code compiles with zero warnings
- [ ] All providers have generated `*_task` methods
- [ ] Task functions enable composition and wrapping

## Verification

```bash
cd /home/darkvoid/Boxxed/@dev/ewe_platform

# Regenerate all provider clients with task methods
cargo run --bin ewe_platform gen_resources clients

# Verify compilation
cargo check -p foundation_deployment --features gcp,cloudflare,stripe

# Verify rustdoc
cargo doc -p foundation_deployment --no-deps

# Verify clippy
cargo clippy -p foundation_deployment -- -D warnings -W clippy::pedantic

# Verify task methods exist
grep -r "_task(" backends/foundation_deployment/src/providers/*/clients/*.rs | head -20
```

---

_Created: 2026-04-08_

## Implementation Notes

### TaskIterator vs StreamIterator

- **`TaskIterator`** - Represents a computation that can be executed, produces `Done` and `Pending` states
- **`StreamIterator`** - Represents an executing computation, yields `Next` and `Pending` states
- **`execute()`** - Converts `TaskIterator` to `StreamIterator` by scheduling on valtron's thread pool

### Why Task Functions?

Task functions provide a "hook point" for users who need:
1. **Custom valtron combinators** - Apply `map()`, `filter()`, `chain()` before execution
2. **Task composition** - Use `join()`, `race()`, `select()` with multiple tasks
3. **Testing** - Test task construction without triggering execution
4. **Lazy evaluation** - Defer execution until the task is composed into a larger pipeline

### Code Generation Order

Functions should be generated in this order within each file:
1. `_builder()` functions
2. `_task()` functions
3. `_execute()` functions  
4. Convenience functions (no suffix)

This ensures all dependencies are defined before use.

### Known Limitations

1. **TaskIterator import** - Must be imported in generated files
2. **Doc comments** - Must clearly distinguish between task and execute functions
3. **Return type complexity** - `impl TaskIterator` with associated types can be verbose
