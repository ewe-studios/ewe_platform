# 03 — Type Replication Strategy

**Date:** 2026-07-10
**Status:** Resolved

## Decision

Use `foundation_openapi::UnifiedGenerator` to generate the initial API surface from the Docker
Engine OpenAPI spec, then hand-refine the generated output where the spec → code mapping is
lossy or produces awkward types.

This is a hybrid approach: **generate first, refine second** — not pure hand-writing, and not
raw codegen without intervention.

## How UnifiedGenerator works

The generator at `foundation_openapi/src/unified/generator.rs` takes an OpenAPI spec and produces:

1. **`shared/mod.rs`** — Re-exports `ApiError`, `ApiResponse`, `ApiPending`, `Empty`, `Operation`
   from `crate::providers::common`. Generates stub structs for shared resource types.

2. **Per-group `mod.rs`** — For each API group (containers, images, networks, volumes, etc.):
   - **Type declarations** from `$ref` schemas — proper structs with properties, `Box<>` for recursive types
   - **Args types** — `{OperationId}Args` with path params, query params, and body
   - **Client functions** — `{operation_id}_request(client, args, builder_mod)` returning
     `impl TaskIterator<Ready = Result<ApiResponse<T>, ApiError>, Pending = ApiPending, ...>`

3. **Provider `mod.rs`** — Feature-guarded module tree with all groups.

4. **Cargo.toml updates** — Auto-adds feature flags for provider and groups.

### Generated client function pattern

```rust
/// GET /containers/json.
pub fn list_containers_request<R, F>(
    client: &SimpleHttpClient<R>,
    args: &ListContainersArgs,
    builder_mod: Option<F>,
) -> Result<impl TaskIterator<Ready = Result<ApiResponse<Vec<ContainerSummary>>, ApiError>, Pending = ApiPending, Spawner = BoxedSendExecutionAction> + Send + 'static, ApiError>
where
    R: DnsResolver + Clone + Default + 'static,
    F: FnOnce(&mut ClientRequestBuilder<R>),
{
    let endpoint_url = format!("http://localhost/v1.53/containers/json");
    // ... append query params ...
    client.get(&endpoint_url)
        .build_send_request()?
        .map_ready(|intro| match intro {
            RequestIntro::Success { stream, .. } => {
                // parse JSON → ApiResponse<T>
            }
            RequestIntro::Failed(e) => Err(ApiError::RequestSendFailed(...)),
        })
        .map_pending(|_| ApiPending::Sending)
}
```

## Generation workflow

```
1. Vendor Docker Engine OpenAPI spec → specs/docker-engine-v1.53.yaml
2. Run: foundation_openapi::UnifiedGenerator::new("backends/foundation_deployment_docker/src/providers/")
3. Generator produces: src/providers/shared/, src/providers/containers/, src/providers/images/, etc.
4. Run: cargo fmt (generator already calls rustfmt)
5. Refine generated types where the spec was lossy (see refinement section below)
```

## Why not pure hand-writing

### Spec 53 lesson: hand-writing 47+ types is tedious and error-prone

The Docker Engine API v1.53 has ~200 types in its schemas. Hand-writing all of them:
- Takes significant time
- Is error-prone (missing fields, wrong types, wrong serde attributes)
- Duplicates work the spec already encoded

### Why not raw codegen without refinement

OpenAPI specs have limitations that make generated code awkward:
- `$ref` chains produce deeply nested types
- `allOf` compositions merge into single structs but lose semantic grouping
- Optional vs required fields depend on spec annotations (often inaccurate)
- Docker-specific patterns (like `ContainerCreateBody` combining Config + HostConfig) aren't
  visible from the spec alone

## Refinement areas

After generation, we refine:

### 1. Serde attributes

Generated types use `#[serde(flatten)] pub data: HashMap<String, Value>` as fallback when
the schema has no properties. We replace these with proper fields based on bollard's generated
types as reference.

### 2. Request body composition

Docker's `/containers/create` body combines `ContainerConfig` + `HostConfig`. The spec may
generate separate types. We ensure the generated `ContainerCreateBody` matches what the API
actually expects.

### 3. Query parameter builders

Generated `{Operation}Args` types are flat structs. We may want builder patterns:
```rust
// Generated
pub struct CreateContainerArgs { pub name: Option<String>, pub platform: Option<String>, pub body: ContainerCreateBody }

// Refined: add builder
impl CreateContainerArgs {
    pub fn builder() -> CreateContainerArgsBuilder { ... }
}
```

### 4. Streaming endpoints

The generator produces standard JSON response handling. For streaming endpoints
(logs, events, stats), we replace the generated code with custom stream decoders
(see **[08 — Streaming responses](08-streaming-responses.md)**).

### 5. Unix socket URL handling

Generated code uses `https://api.example.com` as the base URL. We need to parameterize
this so the client can use `http://localhost` with Unix socket transport.

## Type organization (post-generation)

```
foundation_deployment_docker/src/
  providers/
    shared/mod.rs         # Generated: ApiError, ApiResponse, shared types
    containers/mod.rs     # Generated: Container types + client functions
    images/mod.rs         # Generated: Image types + client functions
    networks/mod.rs       # Generated: Network types + client functions
    volumes/mod.rs        # Generated: Volume types + client functions
    system/mod.rs         # Generated: System types + client functions
    exec/mod.rs           # Generated: Exec types + client functions
  docker_client.rs        # Hand-written: DockerClient struct, Unix socket setup, auth
  deployable.rs           # Hand-written: Deployable impl for containers/networks/volumes
  streaming/
    mod.rs                # Hand-written: LogFrameDecoder, JsonLineDecoder
```

## Priority order

| Priority | Generation target | Refinement |
|----------|------------------|------------|
| **P0** | containers, images, exec | Stream decoders, request body composition, Unix socket URL |
| **P1** | networks, volumes, system | Minor type fixes, query param builders |
| **P2** | BuildKit (separate via connectrpc) | Proto codegen, gRPC client |
| **P3** | swarm, services, plugins, secrets | Not needed for testbed |
