# Progress — Spec 53: Docker Container Testbed

**Last updated:** 2026-07-10

## Phase 1: foundation_deployment_platform skeleton ✅

| File | Status |
|------|--------|
| Cargo.toml | ✅ bollard 0.21, tokio, futures-lite, foundation_errstacks |
| lib.rs | ✅ Re-exports docker module + block_on |
| docker/error.rs | ✅ DockerError enum, std::error::Error impl, docker_err() helper |
| docker/config.rs | ✅ ContainerConfig builder with full chainable API |
| docker/container.rs | ✅ ContainerHandle — start_async/start, shutdown, is_running, Drop, port wait |
| docker/group.rs | ✅ ContainerGroup for multi-container |
| docker/client.rs | ✅ DockerClient wrapping bollard (local + SSH stub) |
| docker/network.rs | ✅ NetworkHandle — create_or_find, find, connect, remove |
| docker/image.rs | 🔄 Stub — build_once() pending |
| docker/wait_for.rs | ✅ Port (TCP connect loop), Stdout (logs stream), Http (stub), Composite (iterative) |
| tests/container_integration.rs | ✅ Redis start/stop/PING/SET/GET/Drop |

## Phase 2: Wait strategies + Network ✅

- ✅ Port wait — TCP connect loop with exponential backoff (100ms→1s cap)
- ✅ Stdout wait — bollard logs() streaming API with tokio timeout
- 🔄 Http wait — stub (deferred until reqwest/foundation_http available)
- ✅ Composite wait — iterative stack-based flattening
- ✅ NetworkHandle — create_or_find, find, connect, remove

## Next: Phase 3 — Proc macro

- [ ] `#[docker_container]` in `foundation_macros/src/docker_container.rs`
- [ ] Re-export from `foundation_deployment_platform`
- [ ] Macro integration tests
