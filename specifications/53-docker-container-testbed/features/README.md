# Spec-53 Feature Set — 2026-07-15

All features are complete. Tests: 11/11 command, 3/3 powershell, 10/10 wait_for,
313/313 valtron, 6/6 sleep.

## Phase 1: Docker Runtime (Part A)

| # | Feature | What | Why |
|---|---------|------|-----|
| 01 | Runtime Library | ContainerHandle (RAII), ContainerConfig (builder), DockerError, NetworkHandle | Testcontainers-style API |
| 02 | Proc Macro | #[docker_container(image="redis:7", port=6379)] | One-annotation container lifecycle |
| 03 | Networking | NetworkHandle + bind mounts + named volumes | Multi-container test scenarios |
| 04 | Images | Pull from registry, local cache, DockerFileConfig | Image management |
| 05 | Wait Strategies | Port, HTTP, Stdout, Composite resilience checks | Container readiness |

## Phase 2: Client Migration

| # | Feature | What | Why |
|---|---------|------|-----|
| 07 (prev) | Bollard → deployment_docker | bollard 0.21 + tokio → foundation_deployment_docker + valtron | No tokio reactor, same HTTP stack as rest of workspace |
| — | ContainerCreateBody | Typed POST /containers/create with HostConfig + Resources merge | No serde_json::Value shims |
| — | sleep_async | std::thread::sleep → valtron sleep_async in all backoff loops | Cooperative, DurationWaker-backed |

## Phase 3: Platform Consolidation (Part B)

| # | Feature | What | Why |
|---|---------|------|-----|
| 09 | sshkit enhancements | powershell.rs, shell.rs, Command::bash()/cmd() | Consolidate SSH tools |
| 10 | Provider trait migration | QEMU/UTM implement associated-types Provider alongside Docker | One trait, three backends |
| 11 | vms/ file migration | 58 files testbed → platform, feature-gated | Clean crate dependency graph |

## Tests Added

| Crate | Test | What it verifies |
|-------|------|-----------------|
| foundation_sshkit | powershell_tests.rs (3) | ps_exec error handling, base64 encoding, UTF-16LE roundtrip |
| foundation_sshkit | command_tests.rs (3 new) | .bash() wrapping, .cmd() wrapping, combined env+cd+bash |
| foundation_deployment_platform | wait_for_tests.rs (2 async) | port_wait_succeeds_on_open_port, port_wait_times_out_on_closed_port |
| foundation_core | sleep_tests.rs (6) | SleepingTask unit tests + sleep_async #[valtron_test] |
