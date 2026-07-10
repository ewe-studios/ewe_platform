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

### Phase 1: Crate scaffolding
- [ ] Create `backends/foundation_deployment_docker/` with Cargo.toml
- [ ] Vendor Docker Engine OpenAPI spec (v1.53) to `specs/`
- [ ] Define `DockerError`, `DockerPending` types
- [ ] Define `DockerClient` struct with Unix socket support

### Phase 2: Container API (P0)
- [ ] Hand-write `ContainerCreateBody`, `ContainerInspectResponse`, `ContainerSummary`
- [ ] Hand-write `CreateContainerOptions`, `StartContainerOptions`, `StopContainerOptions`
- [ ] Implement `create_container()` TaskIterator
- [ ] Implement `start_container()` TaskIterator
- [ ] Implement `stop_container()` TaskIterator
- [ ] Implement `remove_container()` TaskIterator
- [ ] Implement `inspect_container()` TaskIterator
- [ ] Implement `list_containers()` TaskIterator

### Phase 3: Image API (P0)
- [ ] Hand-write `ImageSummary`, `ImageInspect`, `PullImageOptions`, `PushImageOptions`
- [ ] Implement `pull_image()` TaskIterator
- [ ] Implement `list_images()` TaskIterator
- [ ] Implement `inspect_image()` TaskIterator
- [ ] Implement `remove_image()` TaskIterator

### Phase 4: Exec API (P0)
- [ ] Hand-write `ExecConfig`, `CreateExecOptions`, `ExecInspectResponse`
- [ ] Implement `create_exec()` TaskIterator
- [ ] Implement `start_exec()` TaskIterator
- [ ] Implement `inspect_exec()` TaskIterator

### Phase 5: Streaming (P0)
- [ ] Implement `LogFrameDecoder` for Docker's 8-byte multiplexed format
- [ ] Implement `JsonLineDecoder<T>` for JSON-line streams
- [ ] Implement `container_logs()` with streaming
- [ ] Implement `container_stats()` with streaming

### Phase 6: Network + Volume API (P1)
- [ ] Hand-write `Network`, `NetworkCreateRequest`, `EndpointSettings`
- [ ] Hand-write `Volume`, `VolumeCreateRequest`
- [ ] Implement network CRUD TaskIterators
- [ ] Implement volume CRUD TaskIterators

### Phase 7: System API (P1)
- [ ] Hand-write `SystemVersion`, `SystemInfo`
- [ ] Implement `version()`, `info()`, `ping()` TaskIterators
- [ ] Implement `events()` streaming
- [ ] Implement `df()` (disk usage)

### Phase 8: Deployable integration
- [ ] Implement `Deployable` for `DockerContainer`
- [ ] Implement `Deployable` for `DockerNetwork`
- [ ] Implement `Deployable` for `DockerVolume`
- [ ] State store persistence for deploy/destroy

### Phase 9: BuildKit via connectrpc (P2)
- [ ] Vendored protobuf files to `specs/buildkit/`
- [ ] Proto code generation via `foundation_connectrpc_codegen`
- [ ] `BuildKitClient` with Unix socket transport
- [ ] `Solve`, `Status`, `DiskUsage`, `Prune` operations
- [ ] Integrate with `build_image()` flow

## Related specs

- **[Spec 53](../53-docker-container-testbed/)** — Previous attempt using bollard directly
- **[Spec 41](../41-connectrpc/)** — ConnectRPC foundation (used for BuildKit)
