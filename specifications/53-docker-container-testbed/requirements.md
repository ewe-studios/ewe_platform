# Specification 53: Docker Container Testbed

## Overview

This specification covers two related Docker initiatives in one spec:

**Part A — `foundation_deployment_platform` crate + Docker runtime.** The
existing platform abstraction (Provider trait, QEMU/UTM backends, SSH/WinRM,
bootstrap, images, state, CLI) moves from `foundation_testbed` into a new
`foundation_deployment_platform` crate alongside `foundation_deployment`. A
new `docker/` module in this crate wraps `bollard` to provide a
testcontainers-like experience — `ContainerHandle` (RAII), `ContainerConfig`
(builder), `WaitFor` strategies, `NetworkHandle` — plus the
`#[docker_container(...)]` proc macro (in `foundation_macros`, referencing
`foundation_deployment_platform::docker`). No separate `foundation_deployment_platform`
crate — the Docker runtime lives as a module within the platform crate.

**Part B — Testbed migration.** Replace QEMU/KVM-based VMs in
`foundation_testbed` with Docker containers as the primary test isolation
mechanism for Linux. The testbed becomes a thin consumer: depends on
`foundation_deployment_platform` for all VM/container lifecycle, adds its
wasm test harness, and provides thin CLI wrappers. Part B adds a
`DockerProvider` (implementing the `Provider` trait using
`foundation_deployment_platform::docker` primitives) alongside the existing
`QemuProvider` and `UtmProvider`.

## Goals

### Part A — Platform crate + Docker runtime

- **Proc-macro ergonomics** — `#[docker_container(image = "redis:7", port = 6379)]`
  on any function (test, main, regular) starts a container for its duration.
  Macro in `foundation_macros`, runtime in `foundation_deployment_platform::docker`.
- **RAII lifecycle** — `ContainerHandle` stops and removes containers on Drop,
  even on panic. No manual cleanup code.
- **Programmatic API** — `ContainerConfig` builder, `ContainerHandle::start()`,
  `ContainerGroup` for multi-container, `WaitFor` strategies, `NetworkHandle`.
- **Zero-config local dev** — Connects to the local Docker socket. Colima on macOS.
- **Remote Docker support** — SSH transport to remote daemons via bollard.
- **Graceful skip without Docker** — On machines without Docker, tests skip and
  pass rather than fail.
- **Works with valtron** — Docker lifecycle ops use an internal tokio runtime
  (bollard requires tokio for hyper). Proc macro composes with `#[valtron_test]`.
- **Platform consolidation** — Provider trait, all backends, SSH/WinRM, bootstrap,
  images, state, and CLI live in one crate. No circular deps.

### Part B — Testbed migration

- **Replace QEMU with Docker** for Linux test environments, retaining QEMU
  only where Docker cannot reach (macOS guests without KVM).
- **`DockerProvider`** as a new `Provider` impl using `foundation_deployment_platform::docker`.
- **Docker network model** for multi-container test scenarios.
- **Adapt cloud-init patterns** from `vm-uncloud` for cloud deployment.
- **Preserve existing functionality**: SSH exec, file push/pull, build-in-guest,
  binary validation — all must work through the Docker provider.
- **Thin testbed** — `foundation_testbed` becomes a consumer, not a platform owner.

## Non-Goals

- Replacing QEMU for macOS on Apple Silicon (UTM with native HVF remains
  the best option for Mac hosts; dockurr/macos requires KVM).
- A Compose replacement — this crate provides imperative container management;
  Compose files are a complementary serialization format.
- A production deployment tool — this is for testing and development.

## Decisions

All decisions documented in `decisions/`.

### Part A — Platform crate + Docker runtime

| # | Decision | Summary |
|---|----------|---------|
| 03 | Crate Architecture | Extract platform from testbed into `foundation_deployment_platform`; docker module lives there |
| 07 | Bollard + Internal Tokio Runtime | Bollard for Docker API; core API is async; sync at boundaries via block_on |
| 08 | Proc Macro Location | Macro in `foundation_macros`, runtime types in `foundation_deployment_platform::docker` |
| 09 | Container Lifecycle | Pull → Create → Start → Inspect → Wait → Use → Stop → Remove |
| 10 | Networking Model | User-defined bridge networks via `NetworkHandle`; DNS-based service discovery |
| 11 | Wait Strategies | Port, HTTP, Stdout, Composite, None; extensible enum |
| 12 | Error Handling | Typed `DockerError` enum; `is_connection_error()` enables graceful Docker-absent skip |

### Part B — Testbed integration

| # | Decision | Summary |
|---|----------|---------|
| 01 | Docker API Strategy | `ContainerServiceDefinition` + bollard; Compose YAML as optional serialization |
| 02 | Container Strategy per Platform | Native Docker (Linux), dockurr (Windows + macOS), QEMU/UTM fallback |
| 04 | Networking & Volume Mounts | User-defined bridge networks, bind mounts for project source, named volumes for state |
| 05 | Image Management | Multi-stage Dockerfiles + `DockerfileConfig` + `build_once()` + pre-built registry images |
| 06 | Cloud Deployment | cloud-init for Hetzner; colima on macOS; remote Docker via bollard SSH |
| 13 | foundation_sshkit | Dedicated SSH crate: connection pooling, key management, host abstraction, runners |
| 14 | foundation_proxy | Reverse proxy: SSL termination, zero-downtime deploys, VFS cert storage |
| 15 | Cloudflare Crate Transition | Convert `foundation_deployment_cloudflare` from auto-generated to hand-maintained; add `CloudflareClient`, `DnsRecord`, auth management |
| 16 | foundation_deployment Split | Split `foundation_deployment` into shared library (OpenAPI utils) + `foundation_deployment_platform` (orchestration); archive 8 broken sibling shells |

## Features

| # | Feature | Description | Priority |
|---|---------|-------------|----------|
| 01 | Runtime Library | `DockerClient`, `ContainerHandle`, `ContainerConfig`, `DockerError` | High |
| 02 | Proc Macro | `#[docker_container(...)]` attribute macro | High |
| 03 | Networking & Volumes | `NetworkHandle`, network lifecycle, bind mounts, named volumes | Medium |
| 04 | Image Management | Pull from registry, local cache, progress streaming | Medium |
| 05 | Wait Strategies | Port, HTTP, Stdout, Composite readiness checks | Medium |

## Plan

1. **Spec & design** — This document and decision files.
2. **Feature 01 — Runtime library** — `DockerClient`, `ContainerHandle`,
   `ContainerConfig`, `DockerError`. Manual integration tests.
3. **Feature 02 — Proc macro** — `#[docker_container]` in `foundation_macros`,
   re-exported from `foundation_deployment_platform`. Macro integration tests.
4. **Feature 03 — Networking & volumes** — `NetworkHandle`, bind mounts,
   named volumes, multi-container scenarios.
5. **Feature 04 — Image management** — `ImageHandle`, pull with progress,
   local cache, `always_pull` flag.
6. **Feature 05 — Wait strategies** — Port polling, HTTP health checks,
   stdout log scanning, composite strategies.
7. **Testbed integration** — `DockerProvider` consuming `foundation_deployment_platform`,
   testbed profiles as Docker containers, cloud deployment with cloud-init.
