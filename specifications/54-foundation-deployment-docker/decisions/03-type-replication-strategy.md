# 03 — Type Replication Strategy

**Date:** 2026-07-10
**Status:** Resolved

## Decision

Generate the initial API surface using `gen_api` (the `foundation_codegentools` binary), then
hand-refine the generated output where the spec → code mapping is lossy or produces awkward types.

This is a hybrid approach: **generate first, refine second** — not pure hand-writing, and not
raw codegen without intervention.

Breaking changes are acceptable when they improve the codebase. The Feature 00 migration
(SimpleHttpClient → DynNetClient, ClientRequestBuilder → PreparedRequestBuilder) is exactly
this kind of structural improvement — we break compatibility with the old generated surface
because the new surface is strictly better.

## How gen_api / UnifiedGenerator works

The binary at `foundation_codegentools/src/cli/gen_api.rs` wraps
`foundation_openapi::UnifiedGenerator` and handles:

1. **Spec discovery** from `artefacts/cloud_providers/<provider>/` (single or multiple specs)
2. **Analysis** → `analyze_spec()` groups endpoints by path prefix, identifies shared resource types
3. **Code generation** via `UnifiedGenerator::generate()`:
   - **`shared/mod.rs`** — Re-exports `ApiError`, `ApiResponse`, `ApiPending`, `Empty`, `Operation`
     from `crate::providers::common`. Generates stub structs for shared resource types.
   - **Per-group `mod.rs`** — For each API group (containers, images, networks, volumes, etc.):
     - **Type declarations** from `$ref` schemas — proper structs with properties, `Box<>` for recursive types
     - **Args types** — `{OperationId}Args` with path params, query params, and body
     - **Client functions** — `{operation_id}_request(client, args, builder_mod)` returning
       `impl TaskIterator<Ready = Result<ApiResponse<T>, ApiError>, Pending = ApiPending, ...>`
   - **Provider `mod.rs`** — Feature-guarded module tree with all groups.
   - **Cargo.toml updates** — Auto-adds feature flags for provider and groups (hierarchical + flat).
4. **Feature flag fix-up** — `fix_hierarchical_features()` / `fix_flat_features()` rebuilds
   `provider_all`, `provider_group` features from the directory structure.
5. **`cargo fix`** — Auto-cleans unused imports, style issues.

### CLI invocation

```bash
# Analyze (dry run — shows groups, types, endpoints)
cargo run --bin ewe_platform gen_api analyze --provider docker \
  --spec artefacts/cloud_providers/docker/docker-engine-v1.53.json

# Generate (writes files)
cargo run --bin ewe_platform gen_api generate --provider docker \
  --output-dir backends/foundation_deployment_docker/src
```

### Generated client function pattern (post-Feature-00)

```rust
/// GET /containers/json.
pub async fn list_containers<F>(
    client: DynNetClient,
    args: &ListContainersArgs,
    builder_mod: Option<F>,
) -> Result<ApiResponse<Vec<ContainerSummary>>, ApiError>
where
    F: FnOnce(&mut PreparedRequestBuilder),
{
    let endpoint_url = format!("http://localhost/v1.53/containers/json");
    let mut builder = PreparedRequestBuilder::get(&endpoint_url)?
        .query("all", args.all.as_deref())
        .query("limit", args.limit.as_deref());

    if let Some(f) = builder_mod {
        f(&mut builder);
    }

    let req = builder.build();
    let response = client.send_async(req).await
        .map_err(|e| ApiError::RequestSendFailed(e.to_string()))?;

    let body: Vec<ContainerSummary> = response.json().await
        .map_err(|e| ApiError::ParseFailed(e.to_string()))?;

    Ok(ApiResponse {
        status: response.status().into(),
        headers: response.headers().clone(),
        body,
    })
}
```

No `R` generic, no `DnsResolver` bound, no `RequestIntro`, no `build_send_request()`.
Just `async fn` + `DynNetClient` + `PreparedRequestBuilder`.

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

Automate as much as possible via the generator; hand-refine only what the
spec→code mapping can't express (streaming decoders, Unix socket setup, auth).
The generator handles the 90% CRUD surface; hand-writing handles the
Docker-specific 10%.

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

Docker is a **split-out crate** (like cloudflare), so `gen_api` generates directly into
`foundation_deployment_docker/src/`:

```
foundation_deployment_docker/src/
  shared/mod.rs         # Generated: ApiError, ApiResponse, shared types
  containers/mod.rs     # Generated: Container types + {op}_request() TaskIterators
  images/mod.rs         # Generated: Image types + {op}_request() TaskIterators
  networks/mod.rs       # Generated: Network types + {op}_request() TaskIterators
  volumes/mod.rs        # Generated: Volume types + {op}_request() TaskIterators
  system/mod.rs         # Generated: System types + {op}_request() TaskIterators
  exec/mod.rs           # Generated: Exec types + {op}_request() TaskIterators
  docker_client.rs      # Hand-written: DockerClient struct, Unix socket setup, auth
  deployable.rs         # Hand-written: Deployable impl for containers/networks/volumes
  streaming/
    mod.rs              # Hand-written: SendSafeBodyBytesIterator chains, LogFrameDecoder
```

The generator also updates `Cargo.toml` with feature flags:
```toml
docker = []
docker_containers = []
docker_images = []
docker_all = ["docker_containers", "docker_images", "docker_networks", "docker_volumes", "docker_system", "docker_exec"]
```

## Priority order

| Priority | Generation target | Refinement |
|----------|------------------|------------|
| **P0** | containers, images, exec | Stream decoders, request body composition, Unix socket URL |
| **P1** | networks, volumes, system | Minor type fixes, query param builders |
| **P2** | BuildKit (separate via connectrpc) | Proto codegen, gRPC client |
| **P3** | swarm, services, plugins, secrets | Not needed for testbed |
