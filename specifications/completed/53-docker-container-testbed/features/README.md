# Spec-53 Feature Set — Final (2026-07-16)

All 19 features implemented, tested, and committed. No design-only stubs.

## Phase 1: Docker Runtime (Part A)

| # | Feature | Crate | Key files |
|---|---------|-------|-----------|
| 01 | Runtime Library | foundation_deployment_platform | container.rs, config.rs, error.rs |
| 02 | Proc Macro | foundation_macros | docker_container.rs |
| 03 | Networking & Volumes | foundation_deployment_platform | network.rs |
| 04 | Image Management | foundation_deployment_platform | image.rs |
| 05 | Wait Strategies | foundation_deployment_platform | wait_for.rs |

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

| # | Feature | Decision | Crate | Key files |
|---|---------|----------|-------|-----------|
| 12 | SSL redirect | 25 | foundation_proxy | server.rs (ssl_redirect_loop, 8 tests) |
| 13 | Writer affinity | 23 | foundation_proxy | runtime.rs (pick_sticky), handler.rs (cookie) |
| 14 | State persistence | 21 | foundation_proxy | persistence.rs (ProxyStateStore, 4 tests) |
| 15 | ACME cert provisioning | 18 | foundation_proxy | acme.rs (full RFC 8555, DNS-01, 7 tests) |
| 16 | Unix-socket RPC | 20 | foundation_proxy | control.rs (JSON-RPC, 7 commands, 4 tests) |
| 17 | HTTP/2 proxy | 26 | foundation_proxy | h2_proxy.rs (streaming relay, 14 tests) |
| 18 | Zero-downtime deploy | 22 | foundation_proxy | state.rs (draining), server.rs (drain_complete) |
| 19 | HTTP/3 proxy | 27 | foundation_proxy | h3_proxy.rs (streaming relay, quic feature) |

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

## Test results (2026-07-16)

| Suite | Count |
|-------|-------|
| foundation_core valtron | 313 passed |
| foundation_proxy lib | 42 passed |
| foundation_sshkit (command + powershell) | 14 passed |
| foundation_deployment_platform (wait_for) | 10 passed |
| **Total** | **379 passed, 0 failed** |
