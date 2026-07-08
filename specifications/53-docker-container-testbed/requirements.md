# Specification 53: Docker Container Testbed

## Overview

This specification covers two related Docker initiatives in one spec:

**Part A — `foundation_deployment_docker` crate.** A new workspace crate that
wraps `bollard` (Rust Docker Engine API client) to provide a
testcontainers-like experience. The centerpiece is a `#[docker_container(...)]`
proc macro that starts Docker containers before a function body and
stops/removes them after (RAII on Drop, panic-safe). This crate is a sibling of
`foundation_deployment` — a general-purpose Docker interaction layer for the
entire workspace, not scoped to any single consumer.

**Part B — Testbed migration.** Replace QEMU/KVM-based VMs in
`foundation_testbed` with Docker containers as the primary test isolation
mechanism for Linux. Docker provides simpler networking, reliable host-to-guest
filesystem mounts, faster startup times, and a mature image ecosystem. Part B
consumes the crate built in Part A and adds a `DockerProvider` implementation of
the existing `Provider` trait.

## Goals

### Part A — `foundation_deployment_docker` crate

- **Proc-macro ergonomics** — `#[docker_container(image = "redis:7", port = 6379)]`
  on any function (test, main, regular) starts a container for its duration.
- **RAII lifecycle** — `ContainerHandle` stops and removes containers on Drop,
  even on panic. No manual cleanup code.
- **Programmatic API** — `ContainerConfig` builder, `ContainerHandle::start()`,
  `WaitFor` strategies, `NetworkHandle` for multi-container topologies.
- **Zero-config local dev** — Connects to the local Docker socket.
- **Remote Docker support** — SSH transport to remote Docker daemons via bollard.
- **Graceful skip without Docker** — On machines without Docker, tests skip and
  pass rather than fail.
- **Works with valtron** — Docker lifecycle ops use an internal tokio runtime
  (bollard requires tokio for hyper). Proc macro composes with `#[valtron_test]`.

### Part B — Testbed migration

- **Replace QEMU with Docker** for Linux test environments, retaining QEMU
  only where Docker cannot reach (macOS guests, Windows guests).
- **Keep the `Provider` trait** — add a `DockerProvider` alongside the existing
  `QemuProvider` and `UtmProvider`, so callers don't change.
- **Docker network model** for multi-container test scenarios.
- **Adapt cloud-init patterns** from `vm-uncloud` for deploying the testbed to
  Hetzner Cloud and similar providers.
- **Preserve existing functionality**: SSH exec, file push/pull, build-in-guest,
  binary validation — all must work through the Docker provider.

## Non-Goals

- Replacing QEMU for macOS guests (Docker cannot virtualize macOS).
- A Compose replacement — this crate provides imperative container management;
  Compose files are a complementary approach.
- A production deployment tool — this is for testing and development.

## Decisions

All decisions documented in `decisions/`.

### Part A — Crate internals

| # | Decision | Summary |
|---|----------|---------|
| 07 | Bollard + Internal Tokio Runtime | Bollard for Docker API; internal `LazyLock<Runtime>` singleton for async bridge |
| 08 | Proc Macro Location | Macro in `foundation_macros`, runtime types in `foundation_deployment_docker` |
| 09 | Container Lifecycle | Pull → Create → Start → Inspect → Wait → Use → Stop → Remove |
| 10 | Networking Model | User-defined bridge networks via `NetworkHandle`; DNS-based service discovery |
| 11 | Wait Strategies | Port, HTTP, Stdout, Composite, None; extensible enum |
| 12 | Error Handling | Typed `DockerError` enum; `is_connection_error()` enables graceful Docker-absent skip |

### Part B — Testbed integration

| # | Decision | Summary |
|---|----------|---------|
| 01 | Docker API Strategy | Bollard for programmatic control; Compose files as declarative fallback |
| 02 | Container Strategy per Platform | Native Docker (Linux), dockurr wrapper (Windows), QEMU retained (macOS) |
| 03 | Provider Trait Integration | `DockerProvider` as a new `Provider` impl; consumes `foundation_deployment_docker` |
| 04 | Networking & Volume Mounts | User-defined bridge networks, bind mounts for project source, named volumes for state |
| 05 | Image Management | Multi-stage Dockerfiles per profile + pre-built images on a registry |
| 06 | Cloud Deployment | cloud-init for Hetzner; Docker-in-Docker or sibling-container pattern |

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
   re-exported from `foundation_deployment_docker`. Macro integration tests.
4. **Feature 03 — Networking & volumes** — `NetworkHandle`, bind mounts,
   named volumes, multi-container scenarios.
5. **Feature 04 — Image management** — `ImageHandle`, pull with progress,
   local cache, `always_pull` flag.
6. **Feature 05 — Wait strategies** — Port polling, HTTP health checks,
   stdout log scanning, composite strategies.
7. **Testbed integration** — `DockerProvider` consuming `foundation_deployment_docker`,
   testbed profiles as Docker containers, cloud deployment with cloud-init.
