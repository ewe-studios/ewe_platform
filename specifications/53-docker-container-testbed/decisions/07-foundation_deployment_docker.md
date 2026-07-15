# 07 — Docker Client Stack (foundation_deployment_docker + valtron)

**Date:** 2026-07-09 (original), updated 2026-07-15 (migration)
**Status:** Resolved — migrated from bollard to foundation_deployment_docker

## Decision (updated 2026-07-15)

Use **`foundation_deployment_docker`** (spec-54) — our own bollard-free Docker
client over `DynNetClient` + `PreparedRequestBuilder` + valtron. The Docker API
surface is generated from the official `docker-engine-v1.53.yaml` spec. Types
are proper structs via `gen_api`, not `serde_json::Value` shims.

The old bollard 0.21 + tokio stack is **removed**.

### Principle: async core, sync wrappers at the boundary

Every type is **async-first**: `ContainerHandle`, `ContainerGroup`,
`DockerClient`, `NetworkHandle` — all methods that touch the Docker API are
`async fn`. Sync callers (the `Provider` trait, `Drop`, CLI) call
`futures_lite::block_on` at the boundary.

```
Caller                    Bridge                     Core
──────────────────────────────────────────────────────────
#[valtron_test] async fn  → .await                   async fn
Provider trait (sync)     → futures_lite::block_on   async fn
ContainerHandle::Drop     → futures_lite::block_on   async fn
```

### Why we migrated

Bollard's 55+ transitive dependencies (hyper, tokio, tower, etc.) created
runtime conflicts with valtron. Our own client uses the same `DynNetClient` +
`PreparedRequestBuilder` stack as every other provider in the workspace — same
HTTP layer, same error types, same async model.

## Original decision (2026-07-09 — superseded)

The original decision used bollard 0.21 as the Docker API client. Bollard wraps
hyper (tokio), which required a tokio reactor alongside valtron. This created
Decision 29 (Docker test tokio fix).

The migration to `foundation_deployment_docker` eliminates the tokio reactor
entirely. No runtime conflict, no Decision 29.

## What changed

| Area | Before (bollard) | After (deployment_docker) |
|------|-----------------|--------------------------|
| HTTP client | hyper (tokio) | DynNetClient (valtron) |
| Docker types | bollard::models::* | generated::json::* (spec-54 gen_api) |
| Container create body | bollard::models::ContainerCreateBody | ContainerCreateBody + ContainerHostConfig |
| Error type | bollard::errors::Error | foundation_deployment_docker::DockerError |
| SSH transport | bollard connect_ssh | foundation_deployment_docker::connect_ssh (via sshkit) |
| TLS transport | bollard (via hyper) | foundation_deployment_docker::connect_tls (via netio SSLConnector) |
| Image pull | streaming via bollard | buffered via image_pull |
| Log streaming | bollard LogOutput stream | LogFrameDecoder + polling |

## Sync bridges at the boundary

| Caller | Bridge | What happens |
|--------|--------|-------------|
| `#[valtron_test] async fn` | `.await` | valtron drives the future; DockerClient is valtron-native |
| `Provider` trait (sync) | `futures_lite::block_on(...)` | `DockerProvider::launch()` → `block_on(start_async(cfg))` |
| `ContainerHandle::Drop` | `futures_lite::block_on(...)` | Drop is sync; stop/remove are async — bridged inline |

## Macro generates async code

```rust
// User writes:
#[valtron_test]
#[docker_container(image = "redis:7", port = 6379)]
async fn test_redis() {
    let mut conn = redis::connect("...").await?;
    // ...
}

// Expands to (simplified):
#[test]
fn test_redis() {
    foundation_core::valtron::block_on_future(async move {
        let __guard = {
            let __cfg = ContainerConfig::new("redis:7")
                .port(6379)
                .wait(WaitFor::port(6379));
            match ContainerHandle::start_async(__cfg).await {
                Ok(h) => h,
                Err(e) if e.current_context().is_connection_error() => {
                    tracing::warn!("SKIP: Docker not available ({e})");
                    return;
                }
                Err(e) => panic!("Failed to start Docker container: {e}"),
            }
        };
        __docker_body().await;
        drop(__guard);
    });
}
```

## Drop behavior

`ContainerHandle::Drop` calls `stop_container` + `remove_container` via
`futures_lite::block_on` — same bridge as the `Provider` trait methods.
Works regardless of whether the caller is valtron or bare sync.

## Dependency budget

`foundation_deployment_docker` adds minimal transitive deps (it reuses
`foundation_netio`, `foundation_core`, `foundation_sshkit` — all workspace
crates). No tokio, no hyper, no tower.
