# Progress — Spec 53: Docker Container Testbed

**Last updated:** 2026-07-16

## All 29 decisions — ✅ IMPLEMENTED

Every decision has production code and passing tests. No design-only stubs remain.

## Docker runtime (Part A)

| # | Feature | Status |
|---|---------|--------|
| 01 | Runtime Library | ✅ Complete |
| 02 | Proc Macro | ✅ Complete |
| 03 | Networking & Volumes | ✅ Complete |
| 04 | Image Management | ✅ Complete |
| 05 | Wait Strategies | ✅ Complete |
| — | Bollard → deployment_docker migration | ✅ Complete |
| — | valtron sleep_async replacing thread::sleep | ✅ Complete |

## Platform consolidation (Part B)

| # | Feature | Status |
|---|---------|--------|
| 09 | sshkit enhancements (powershell, shell, command wrappers) | ✅ Complete |
| 10 | Provider trait migration (associated types, QEMU/UTM/Docker) | ✅ Complete |
| 11 | vms/ file migration (58 files testbed → platform) | ✅ Complete |

## Proxy capabilities (Part B continued)

| # | Feature | Decision | Status |
|---|---------|----------|--------|
| 12 | SSL redirect (HTTP→HTTPS on port 80) | 25 | ✅ Complete |
| 13 | Writer affinity (sticky sessions via cookie) | 23 | ✅ Complete |
| 14 | State persistence (FileStateStore, TLS certs, config hash) | 21 | ✅ Complete |
| 15 | ACME cert provisioning (full RFC 8555, DNS-01, Let's Encrypt) | 18 | ✅ Complete |
| 16 | Unix-socket control RPC (JSON-RPC, 7 commands) | 20 | ✅ Complete |
| 17 | HTTP/2 proxy (streaming H2 frontend → HTTP/1.1 backend) | 26 | ✅ Complete |
| 18 | Zero-downtime deploy (connection draining, drain_complete) | 22 | ✅ Complete |
| 19 | HTTP/3 proxy (streaming H3 frontend → HTTP/1.1 backend) | 27 | ✅ Complete |

## Spec-53 decisions (all 29)

| # | Decision | Status |
|---|----------|--------|
| 01–06 | Testbed infrastructure, networking, cloud | ✅ |
| 07 | `foundation_deployment_docker` + valtron | ✅ |
| 08–13 | Proc macro, lifecycle, networking, wait, error, sshkit | ✅ |
| 14 | foundation_proxy architecture | ✅ |
| 15 | Cloudflare client transition | ✅ |
| 16 | Deployment crate split | ✅ |
| 17 | UDP proxying | ✅ |
| 18 | TLS termination + auto-cert | ✅ F15 |
| 19 | `proxy!` macro | ✅ |
| 20 | Unix-socket RPC | ✅ F16 |
| 21 | State persistence | ✅ F14 |
| 22 | Zero-downtime deploy + canary | ✅ F18 |
| 23 | Writer affinity | ✅ F13 |
| 24 | Cloudflare typed DNS records | ✅ |
| 25 | SSL redirect (HTTP→HTTPS) | ✅ F12 |
| 26 | HTTP/2 proxy integration | ✅ F17 |
| 27 | HTTP/3 (QUIC) proxy integration | ✅ F19 |
| 28 | Testbed vms/ migration | ✅ F09-F11 |
| 29 | Docker test tokio fix | ✅ Resolved |

## Proxy → kamal-proxy parity

| Capability | Status |
|---|---|
| HTTP/1.1 reverse proxy | ✅ |
| HTTP/2 termination | ✅ |
| HTTP/3 (QUIC) termination | ✅ |
| Let's Encrypt ACME (DNS-01) | ✅ |
| Zero-downtime deploys (drain) | ✅ |
| TCP/UDP health checks | ✅ |
| Weighted round-robin (smooth WRR) | ✅ |
| Sticky sessions (cookie) | ✅ |
| Unix-socket admin interface | ✅ |
| State persistence (cross-restart) | ✅ |
| TCP/UDP passthrough | ✅ |
| WebSocket upgrade relay | ✅ |
| SSL redirect (80→443) | ✅ |
| Structured access logging | ✅ |

## Test results

| Suite | Passed |
|-------|--------|
| foundation_core (valtron) | 313/313 |
| foundation_proxy (lib) | 42/42 |
| foundation_deployment_platform (wait_for) | 10/10 |
| foundation_sshkit (command + powershell) | 14/14 |
| **Total** | **379 passed, 0 failed** |

## Related specs

- **[Spec 54](../completed/54-foundation-deployment-docker/)** — Our own Docker client
- **[Spec 41](../completed/41-connectrpc/)** — ConnectRPC foundation
- **[Spec 55](../completed/55-foundation-wireguard/)** — WireGuard mesh
