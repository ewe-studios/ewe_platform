# Progress — Spec 54: foundation_deployment_docker

## Status: Decisions written, awaiting implementation

## Decision docs

| # | Title | Status |
|---|-------|--------|
| 01 | No bollard dependency | ✅ Written |
| 02 | Docker OpenAPI spec source | ✅ Written |
| 03 | Type replication strategy | ✅ Written |
| 04 | HTTP via SimpleHttpClient | ✅ Written |
| 05 | Valtron TaskIterator format | ✅ Written |
| 06 | BuildKit via foundation_connectrpc | ✅ Written (deferred to P2) |
| 07 | Unix socket transport | ✅ Written |
| 08 | Streaming responses | ✅ Written |

## Implementation phases

### Phase 1: Crate scaffolding + generation
- [ ] Create `backends/foundation_deployment_docker/` with Cargo.toml
- [ ] Download Docker Engine OpenAPI spec (v1.53) → `artefacts/cloud_providers/docker/docker-engine-v1.53.json`
- [ ] Run `gen_api analyze --provider docker` to validate grouping
- [ ] Run `gen_api generate --provider docker` to generate types, Args, {op}_request() functions
- [ ] Register docker as split-out provider in `gen_api.rs` (`SPLIT_OUT_PROVIDERS`)

### Phase 2: Refinement (P0 containers + images)
- [ ] Replace `HashMap<String, Value>` fallback types with proper fields from bollard reference
- [ ] Fix `ContainerCreateBody` to match Docker's actual request body shape
- [ ] Hand-write `DockerClient` (Unix socket setup, auth, version negotiation)
- [ ] Hand-write streaming: `container_logs()`, `container_stats()`, `events()`
  via `SendSafeBodyBytesIterator` + `StreamIteratorExt`
- [ ] Implement `LogFrameDecoder` (~50 lines for 8-byte multiplexed frames)

### Phase 3: Deployable integration (P0)
- [ ] Hand-write `Deployable` for `DockerContainer` (create → start → wait → store)
- [ ] Hand-write `Deployable` for `DockerImage` (pull → store)
- [ ] State store persistence for deploy/destroy via `self.update()`

### Phase 4: Network + Volume API (P1)
- [ ] Refine generated network/volume types where spec is lossy
- [ ] Hand-write `Deployable` for `DockerNetwork`, `DockerVolume`

### Phase 5: System API (P1)
- [ ] Refine generated system types (`SystemVersion`, `SystemInfo`)
- [ ] Implement `events()` streaming endpoint

### Phase 6: BuildKit via connectrpc (P2)
- [ ] Vendored protobuf files to `specs/buildkit/`
- [ ] Proto code generation via `foundation_connectrpc_codegen`
- [ ] `BuildKitClient` with Unix socket transport
- [ ] `Solve`, `Status`, `DiskUsage`, `Prune` operations
- [ ] Integrate with `build_image()` flow

## Related specs

- **[Spec 53](../53-docker-container-testbed/)** — Previous attempt using bollard directly
- **[Spec 41](../41-connectrpc/)** — ConnectRPC foundation (used for BuildKit)
- **[foundation_openapi](../../backends/foundation_openapi/)** — OpenAPI analysis + UnifiedGenerator
- **[foundation_codegentools](../../backends/foundation_codegentools/)** — gen_api binary
