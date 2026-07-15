# Progress — Spec 53: Docker Container Testbed

**Last updated:** 2026-07-15

## Spec-53 Docker work — ✅ COMPLETE

All Docker-related decisions (01-13, 28, 29) are implemented. The remaining proxy
stages (18, 20-23, 25-27) belong to `foundation_proxy`, not Docker — they should
be tracked in a separate specification.

## All features — ✅ complete

The platform crate originally wrapped bollard 0.21 + tokio. Migrated to
`foundation_deployment_docker` (spec-54), our own bollard-free Docker client.

**What changed:**
- `Cargo.toml`: bollard/tokio/futures-util → `foundation_deployment_docker`
- `client.rs`: deleted (thin bollard wrapper, no external consumers)
- `container.rs`: `foundation_deployment_docker::DockerClient` + typed
  `ContainerCreateBody` + `ContainerHostConfig` (merges generated `HostConfig`
  + `Resources`) replace `bollard::Docker` + `ContainerCreateBody`
- `network.rs`: `DockerClient` replaces `bollard::Docker`; `NetworkCreateResponse`
  is typed, `network_list` parses typed `serde_json::Value`
- `image.rs`: `DockerClient` + typed 404 handling on `image_inspect`
- `wait_for.rs`: `DockerClient` + `LogFrameDecoder` for stdout polling;
  `std::thread::sleep` backoff (safe in single-threaded `block_on` context)
- `error.rs`: `map_docker_client_err()` maps spec-54 errors to platform errors
- `mod.rs`: re-exports `foundation_deployment_docker::DockerClient` directly

**Decision 07 renamed:** `07-bollard-tokio-runtime.md` →
`07-foundation_deployment_docker.md`.

**Decision 29 resolved:** Docker test tokio fix — no longer needed (no tokio).

### foundation_deployment_docker additions (for this migration)

- `ContainerCreateBody`: typed `POST /containers/create` body — `#[serde(flatten)]`
  `ContainerConfig` + optional `ContainerHostConfig` + `NetworkingConfig`
- `ContainerHostConfig`: merges generated `HostConfig` (networking/runtime) +
  `Resources` (CPU/memory/devices) via `#[serde(flatten)]`
- `NetworkingConfig`: maps network names to `EndpointSettings`

## All features — ✅ complete

| # | Feature | Status |
|---|---------|--------|
| 01 | Runtime Library | ✅ Complete |
| 02 | Proc Macro | ✅ Complete |
| 03 | Networking & Volumes | ✅ Complete |
| 04 | Image Management | ✅ Complete |
| 05 | Wait Strategies | ✅ Complete |
| — | Bollard migration | ✅ Complete (2026-07-15) |
| 09 | sshkit enhancements | ✅ Complete (2026-07-15) |
| 10 | Provider migration (associated types) | ✅ Complete (2026-07-15) |
| 11 | vms/ file migration → platform | ✅ Complete (2026-07-15) |

### Feature 09: sshkit enhancements — ✅ Complete
- `powershell.rs`: `ps_exec()` — UTF-16LE+Base64, CLIXML stripping
- `shell.rs`: `interactive()` + `exec_streaming()` via ssh CLI
- `command.rs`: `Command::bash()` / `cmd()` OS-aware wrappers

### Feature 10: Provider trait migration — ✅ Complete
- `QemuProvider`, `UtmProvider` implement `Provider<Handle=VmHandle, Config=VmProfile>`
- `DockerProvider` implements `Provider<Handle=ContainerHandle, Config=ContainerConfig>`
- `DisplayMode` is a provider field, not a `launch()` parameter
- Backward-compatible: `VmProvider` trait preserved for existing CLI callers

### Feature 11: vms/ file migration — ✅ Complete
- 58 files moved from `foundation_testbed/src/vms/` → `foundation_deployment_platform/src/`
- `crate::vms::` → `crate::` imports fixed
- Scripts copied, `include_str!` paths adjusted
- Feature-gated: default-build has Docker only; `--features vms` enables QEMU/UTM

### foundation_proxy ✅ — Stage 1 data plane COMPLETE
- ProxyServer::start(), ProxyHandler, HTTP forwarding, health probes, TCP passthrough
- Weighted round-robin, BackendLease, wildcard host matching, longest-path-prefix routing
- **26 unit tests, 8 Docker integration tests**

### foundation_sshkit ✅
- Host, Command, CommandResult, Backend trait, Ssh2Backend, RusshBackend
- ConnectionPool, Runner strategies
- **23 unit tests, 6 Docker-backed tests** (moved to `foundation_deployment_platform/tests/`)

### foundation_deployment_cloudflare ✅
- DnsRecord, Zone, CloudflareClient, dns_ops
- `proxy!` macro test suite: 7 compile-fail + 13 round-trip tests

### foundation_macros ✅
- `#[docker_container]` proc macro (~200 lines), sync + async fn support

### Generator improvements ✅
- gen_api binary, split-out provider routing, typed shared resources, `generated/` isolation

## Spec-53 decisions (29 total)

| # | Decision | Status |
|---|----------|--------|
| 01–06 | Testbed infrastructure, networking, cloud | ✅ Implemented |
| 07 | ~~Bollard + Internal Tokio Runtime~~ → `foundation_deployment_docker` + valtron | ✅ Migrated |
| 08–13 | Proc macro, lifecycle, networking, wait, error, sshkit | ✅ Implemented |
| 14 | foundation_proxy architecture | ✅ Stage 1 done |
| 15 | Cloudflare client transition | ✅ Implemented |
| 16 | Deployment crate split | ✅ Implemented |
| 17 | UDP proxying | ✅ Implemented |
| 18 | TLS termination + auto-cert | 📋 Stage 2 |
| 19 | `proxy!` macro | ✅ COMPLETE |
| 20 | Unix-socket RPC | 📋 Stage 3 |
| 21 | State persistence | 📋 Stage 3 |
| 22 | Zero-downtime deploy + canary | 📋 Stage 3 |
| 23 | Writer affinity | 📋 Stage 3 |
| 24 | Cloudflare typed DNS records | ✅ COMPLETE |
| 25 | SSL redirect (HTTP→HTTPS) | 📋 Bundled with 18 |
| 26 | HTTP/2 proxy integration | 📋 Stage 4 |
| 27 | HTTP/3 (QUIC) proxy integration | 📋 Stage 4+ |
| 28 | Testbed vms/ migration | ✅ Complete (Features 09-11) |
| 29 | Docker test tokio fix | ✅ Resolved by migration |

## Stages 2-4 (proxy) — belong to foundation_proxy, not this spec

## Stage 2 (next up — foundation_proxy)

| Feature | Decision | Dependencies |
|---------|----------|-------------|
| TLS + auto-cert | 18 | rustls, acme-micro, VFS, CloudflareClient |
| SSL redirect | 25 | Bundled with 18 |

## Housekeeping

| Task | Decision | Status |
|------|----------|--------|
| Testbed vms/ migration | 28 | 📋 Not started |
| ~~Docker test tokio fix~~ | 29 | ✅ Resolved — no tokio reactor needed |

## Related specs

- **[Spec 54](../completed/54-foundation-deployment-docker/)** — Our own Docker client (now consumed here)
- **[Spec 41](../completed/41-connectrpc/)** — ConnectRPC foundation
- **[Spec 55](../completed/55-foundation-wireguard/)** — WireGuard mesh
