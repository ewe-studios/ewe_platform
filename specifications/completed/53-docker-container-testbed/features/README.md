# Spec-53 Feature Set (2026-07-16)

> **⚠️ Completeness audit 2026-07-16, extended 2026-07-17.** The earlier "all 19
> implemented, no stubs" claim did not hold. The assembled proxy never ran (a
> `ContextBag` bug panicked `ProxyServer::start`) and the platform test harness
> didn't compile under default features, hiding that F14, F15, F16 and F19 were
> not integrated.
>
> **All four are now fixed and verified** (see the Phase 4 table below), the
> BoringSSL/OpenSSL link conflict is resolved (`boring` gated behind
> `foundation_wireguard`'s `native-mesh`; russh removed, ssh2 the sole backend),
> and the platform suites build and run.
>
> **Part A did not survive the same scrutiny.** F02, F03 and F04 were all marked
> "code present" while having no consumer and no test anywhere in the workspace —
> so none of them worked: the `#[docker_container]` macro did not compile,
> `NetworkHandle` was dead code with five silently-dropped options, and
> `build_once` shelled out to the `docker` CLI over a native `image_build` that
> sent no request body. All three are now repaired and proven end-to-end against
> a real daemon. See [progress.md](../progress.md) for the full corrected status.

## Phase 1: Docker Runtime (Part A)

| # | Feature | Crate | Status | Key files |
|---|---------|-------|--------|-----------|
| 01 | Runtime Library | foundation_deployment_platform | ✅ e2e (`container_integration`) | container.rs, config.rs, error.rs |
| 02 | Proc Macro | foundation_macros | ✅ **fixed + e2e** (`docker_macro_tests`) — did not compile | docker_container.rs |
| 03 | Networking & Volumes | foundation_deployment_platform | ✅ **fixed + e2e** (`network_volume_integration`) — was dead code | network.rs |
| 04 | Image Management | foundation_deployment_platform | ✅ **ported off the CLI + e2e** (`image_build_integration`) | image.rs |
| 05 | Wait Strategies | foundation_deployment_platform | ✅ e2e (`container_integration`) | wait_for.rs |

## Phase 2: Client Migration

| # | Feature | What |
|---|---------|------|
| — | Bollard → deployment_docker | Cargo.toml, client.rs deleted, all bollard types → generated |
| — | ContainerCreateBody | Typed POST body with ContainerHostConfig (HostConfig + Resources) |
| — | sleep_async in valtron | SleepingTask → sleep()/sleep_async() → DurationWaker |

## Phase 3: Platform Consolidation (Part B)

| # | Feature | Crate | Key files |
|---|---------|-------|-----------|
| 09 | sshkit enhancements | foundation_sshkit | powershell.rs, shell.rs, command.rs |
| 10 | Provider trait migration | foundation_deployment_platform | providers/{mod,qemu,utm,docker}/ |
| 11 | vms/ file migration | foundation_deployment_platform | 58 files moved + cfg-gated |

## Phase 4: Proxy Capabilities

| # | Feature | Decision | Status | Key files |
|---|---------|----------|--------|-----------|
| 12 | SSL redirect | 25 | ✅ wired | server.rs (ssl_redirect_loop) |
| 13 | Writer affinity | 23 | ✅ wired | runtime.rs (pick_sticky), handler.rs (cookie) |
| 14 | State persistence | 21 | ✅ wired + verified | persistence.rs + `persist_to`; restore on start, write-through on admin change (`persistence_restart_tests`) |
| 15 | ACME cert provisioning | 18 | ✅ wired + verified | `AcmeCertManager` drives `AcmeClient`; `build_cert_manager` selects it (`acme_provisioning_tests` vs a mock CA) |
| 16 | Unix-socket RPC | 20 | ✅ wired + verified | control.rs + `control_socket`; started in `ProxyServer` (`control_socket_tests`) |
| 17 | HTTP/2 proxy | 26 | ✅ fixed + verified | h2_proxy.rs (now forwards via shared client) |
| 18 | Zero-downtime deploy | 22 | ✅ wired | state.rs (draining), server.rs (drain_complete) |
| 19 | HTTP/3 proxy | 27 | ✅ built + verified | built the foundation_http H3 server layer (`serve_h3`), fixed `h3_proxy.rs`, wired `h3_bind`; e2e `http3_server_tests` + `h3_integration_tests` |

## Foundation reused (not rebuilt)

| What we needed | Already exists in |
|----------------|------------------|
| TLS acceptor, client certs | foundation_netio::netcap::ssl (RustlsAcceptor, SSLConnector) |
| HTTP/2 frame layer | foundation_netio::http2 (H2Channel, H2Client, HPACK, flow control) |
| HTTP/3 + QUIC | foundation_netio::http3 + quic (H3Connection, QuicDriver, QuinnBidiStream) |
| HTTP server frontend | foundation_http (HttpServer, H2Serve, H3Serve, ProtocolDetectHandler) |
| Unix socket primitives | foundation_nativeapis (UnixListener, UnixStream) |
| State storage | foundation_db (FileStateStore, StateStore trait) |
| ECDSA key crypto | sha2, base64 (workspace deps) |

## Test results

> The earlier "379 passed, 0 failed" was **not reproducible** — the platform
> suites did not compile under default features. Since fixed: the default build
> links (BoringSSL/OpenSSL resolved by gating `boring` behind
> `foundation_wireguard`'s `native-mesh`; russh removed), and platform,
> `foundation_proxy`, `foundation_deployment_docker` and `foundation_netio` are
> all green, including the Docker-gated suites. Note `buildkit` and `ssh` are
> mutually exclusive features for the same libcrypto reason — see decision 05.
> See [progress.md](../progress.md) → "Test results (corrected)".
