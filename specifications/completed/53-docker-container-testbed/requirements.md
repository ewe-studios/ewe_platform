# Specification 53: Docker Container Testbed

## Overview

This specification covers two related Docker initiatives in one spec:

**Part A — `foundation_deployment_platform` crate + Docker runtime.** The
existing platform abstraction (Provider trait, QEMU/UTM backends, SSH/WinRM,
bootstrap, images, state, CLI) moves from `foundation_testbed` into a new
`foundation_deployment_platform` crate alongside `foundation_deployment`. A
new `docker/` module in this crate uses `foundation_deployment_docker`
(spec-54, our own bollard-free Docker client) to provide a testcontainers-like
experience — `ContainerHandle` (RAII), `ContainerConfig` (builder), `WaitFor`
strategies, `NetworkHandle` — plus the `#[docker_container(...)]` proc macro
(in `foundation_macros`, referencing `foundation_deployment_platform::docker`).

**Part B — Testbed migration.** Replace QEMU/KVM-based VMs in
`foundation_testbed` with Docker containers as the primary test isolation
mechanism for Linux. The testbed becomes a thin consumer: depends on
`foundation_deployment_platform` for all VM/container lifecycle, adds its
wasm test harness, and provides thin CLI wrappers. Part B adds a
`DockerProvider` (implementing the `Provider` trait using
`foundation_deployment_platform::docker` primitives) alongside the existing
`QemuProvider` and `UtmProvider`.

> **2026-07-15 — Bollard migration complete.** The crate originally wrapped
> bollard 0.21 + tokio. It now uses `foundation_deployment_docker` (spec-54),
> our own Docker client over `DynNetClient` + valtron. The tokio reactor
> dependency is gone. All types use proper generated structs, not
> `serde_json::Value` shims. See [progress.md](progress.md) and
> [decisions/07](decisions/07-foundation_deployment_docker.md).

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
- **Remote Docker support** — SSH transport to remote daemons via `foundation_deployment_docker::connect_ssh`.
- **Graceful skip without Docker** — On machines without Docker, tests skip and
  pass rather than fail.
- **Works with valtron** — Docker lifecycle uses `futures_lite::block_on` for sync
  callers (Provider trait, Drop), `.await` for async callers.
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
| 07 | Docker Client Stack | Migrated from bollard to `foundation_deployment_docker` + valtron (2026-07-15); no tokio reactor |
| 08 | Proc Macro Location | Macro in `foundation_macros`, runtime types in `foundation_deployment_platform::docker` |
| 09 | Container Lifecycle | Pull → Create → Start → Inspect → Wait → Use → Stop → Remove |
| 10 | Networking Model | User-defined bridge networks via `NetworkHandle`; DNS-based service discovery |
| 11 | Wait Strategies | Port, HTTP, Stdout, Composite, None; extensible enum |
| 12 | Error Handling | Typed `DockerError` enum; `is_connection_error()` enables graceful Docker-absent skip |

### Part B — Testbed integration

| # | Decision | Summary |
|---|----------|---------|
| 01 | Docker API Strategy | `ContainerConfig` + `foundation_deployment_docker`; Compose YAML as optional serialization |
| 02 | Container Strategy per Platform | Native Docker (Linux), dockurr (Windows + macOS), QEMU/UTM fallback |
| 04 | Networking & Volume Mounts | User-defined bridge networks, bind mounts for project source, named volumes for state |
| 05 | Image Management | Multi-stage Dockerfiles + `DockerfileConfig` + `build_once()` + pre-built registry images |
| 06 | Cloud Deployment | cloud-init for Hetzner; colima on macOS; remote Docker via `connect_ssh` |
| 13 | foundation_sshkit | Dedicated SSH crate: connection pooling, key management, host abstraction, runners |
| 14 | foundation_proxy | Reverse proxy: SSL termination, zero-downtime deploys, VFS cert storage |
| 15 | Cloudflare Crate Transition | Convert `foundation_deployment_cloudflare` from auto-generated to hand-maintained |
| 16 | foundation_deployment Split | Split `foundation_deployment` into shared library + `foundation_deployment_platform` |

## Features

| # | Feature | Description | Priority | Status |
|---|---------|-------------|----------|--------|
| 01 | Runtime Library | `ContainerHandle`, `ContainerConfig`, `DockerError`, `NetworkHandle` | High | ✅ Complete |
| 02 | Proc Macro | `#[docker_container(...)]` attribute macro | High | ✅ Complete |
| 03 | Networking & Volumes | `NetworkHandle`, network lifecycle, bind mounts, named volumes | Medium | ✅ Complete |
| 04 | Image Management | Pull from registry, local cache | Medium | ✅ Complete |
| 05 | Wait Strategies | Port, HTTP, Stdout, Composite readiness checks | Medium | ✅ Complete |

## Remaining work

> **⚠️ Reopened by the 2026-07-16 completeness audit.** Several features marked
> complete were not actually integrated (their modules exist and unit-test in
> isolation but are never wired into the running system). See
> [progress.md](progress.md) → "Audit findings" for evidence.

| # | Item | Decision | Status |
|---|------|----------|--------|
| — | Make russh the default OpenSSL-free SSH backend; route all usage through traits | 13 | 🔴 In progress |
| — | Replace `ssh2-config` host-config parsing (drops `git2/libssh2` OpenSSL pull) | 13 | 🔴 In progress |
| F14 | Wire state persistence into `ProxyServer` | 21 | ✅ Done |
| F15 | Wire ACME provisioning into TLS path (`AcmeCertManager`) | 18 | 🔴 Not integrated |
| F16 | Wire unix-socket control RPC into `ProxyServer` | 20 | ✅ Done |
| F19 | Integrate HTTP/3 (define `quic` feature + deps, fix handler, wire, test) | 27 | 🔴 Not compiled |
| — | `foundation_deployment_docker` CI tests | — | 📋 Needs Docker-in-CI |

## Plan

1. **Spec & design** — ✅ This document and decision files.
2. **Feature 01 — Runtime library** — ✅ `ContainerHandle`, `ContainerConfig`,
   `DockerError`, `NetworkHandle`. Integration tests.
3. **Feature 02 — Proc macro** — ✅ `#[docker_container]` in `foundation_macros`.
4. **Feature 03 — Networking & volumes** — ✅ `NetworkHandle`, bind mounts.
5. **Feature 04 — Image management** — ✅ Pull with cache, `always_pull`.
6. **Feature 05 — Wait strategies** — ✅ Port, HTTP, Stdout, Composite.
7. **Bollard → deployment_docker migration** — ✅ Complete (2026-07-15).
8. **Testbed integration** — 📋 `DockerProvider` consuming `foundation_deployment_platform`.
