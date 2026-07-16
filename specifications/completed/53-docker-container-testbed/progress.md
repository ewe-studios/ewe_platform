# Progress — Spec 53: Docker Container Testbed

**Last updated:** 2026-07-16

> ## ⚠️ Completeness audit (2026-07-16) — several "complete" features reopened
>
> A completeness review found that the assembled proxy and the platform test
> harness were never actually exercised, which masked a set of integration
> defects. `ProxyServer::start` panicked on **every** call (a `ContextBag`
> type-key bug), so no server-level proxy feature was runnable; and
> `foundation_testbed` did not compile under default features, so the platform
> test suites never ran in CI. Unit tests passed in isolation and hid all of it.
>
> Corrected status is below. Items marked 🔴 are **reopened / in progress**.

## Audit findings

### Fixed during the audit
- **`ProxyServer::start` panicked always** — `ContextBag::store` keys by
  `TypeId::of::<T>()` and wraps in its own `Arc`; the code stored a pre-`Arc`'d
  `ProxyState` under the wrong key, so `ProxyHandler::create` never found it.
  Fixed (store value, retrieve the shared `Arc`).
- **F17 HTTP/2 proxy was broken** — hand-rolled upstream HTTP/1.1 that only
  decoded chunked responses (empty body for any `Content-Length` backend), and
  every `tx.send()` dropped the un-awaited future so no frame was sent. Rewrote
  to forward through the shared `NativeHttpClient`; H2 client now assembles the
  response body. Verified end-to-end (`h2_integration_tests`).
- **`splice_bidirectional` broke on completion sockets** — treated the async
  `flush` barrier (`WouldBlock` = "send in flight") as fatal. Rewrote with a
  backpressure state machine. Verified (`completion_splice_tests`).
- **`foundation_testbed` didn't build with default features** — the
  `foundation_deployment_platform` dep was misplaced under the wasm32 target
  table and the `pub use … as platform` was unconditional. Fixed; this unblocks
  every crate that dev-depends on testbed.
- **Platform SSH tests didn't compile under default features** — three files
  import `foundation_sshkit` (vms-only) with no feature guard. Guarded.

### Reopened — not actually integrated
| # | Feature | Reality |
|---|---------|---------|
| 🔴 F14 | State persistence | `ProxyStateStore` is never called by `ProxyServer` — no load on start, no save on change |
| 🔴 F15 | ACME cert provisioning | `AcmeClient` is unused outside `acme.rs`; there is no `AcmeCertManager`, so ACME never provisions the proxy cert |
| 🔴 F16 | Unix-socket control RPC | `ControlSocket::start()` is never called; the running proxy exposes no admin socket |
| 🔴 F19 | HTTP/3 proxy | Entire `h3_proxy.rs` is gated on a **non-existent** `quic` feature (Cargo has only `default = []`); no QUIC deps; never wired (`ServerApp::both`, not `Any{…http3}`); 0 tests; same chunked-only bug |

### Reopened — dependency / backend defect
| Item | Reality |
|------|---------|
| 🔴 SSH backend grounding | `russh` implements the `Backend` trait but is not "as grounded as ssh2": the Docker-over-SSH transport (`Dialer`/`ChannelStream`), the connection pool (`ssh2::Session`), `powershell` (`&ssh2::Session`), the platform `ssh` module, and host-config parsing (`ssh2-config` → `git2/libssh2`) all bind to ssh2 concretely. `ssh2` + `ssh2-config` are hard deps → OpenSSL is always linked. **Decision: make russh the default OpenSSL-free backend; abstract all ssh2 sites behind traits.** |
| 🔴 Platform default build link failure | `docker`+`wireguard` (default) fails to **link**: `ssh2`/libssh2 needs OpenSSL 3 symbols; `foundation_wireguard`→`boring` provides BoringSSL, which lacks them. Resolved by the russh-default work above. |

## Docker runtime (Part A) — implemented

| # | Feature | Status |
|---|---------|--------|
| 01 | Runtime Library (`ContainerHandle`/`Config`/`DockerError`/`NetworkHandle`) | ✅ code present |
| 02 | Proc Macro `#[docker_container]` | ✅ code present |
| 03 | Networking & Volumes | ✅ code present |
| 04 | Image Management | ✅ code present |
| 05 | Wait Strategies | ✅ code present |
| — | Bollard → `foundation_deployment_docker` migration | ✅ (no bollard dep) |

> Part A Docker-backed tests are `#[ignore]` (need a Docker daemon) and were not
> executed in this audit; "code present" means the types and non-Docker paths
> exist and compile, not that the container round-trips were re-verified here.

## Platform consolidation (Part B) — implemented, with the testbed defect fixed

| # | Feature | Status |
|---|---------|--------|
| 09 | sshkit (powershell, shell, command, runner, pool) | ✅ code present (ssh2-coupled — see SSH grounding) |
| 10 | `DockerProvider` (`impl Provider`) | ✅ code present |
| 11 | vms/ file migration | ✅ code present (testbed re-export bug fixed) |

## Proxy capabilities (Part B continued)

| # | Feature | Decision | Status |
|---|---------|----------|--------|
| 12 | SSL redirect | 25 | ✅ wired |
| 13 | Writer affinity (sticky cookie) | 23 | ✅ wired |
| 14 | State persistence | 21 | 🔴 not integrated |
| 15 | ACME cert provisioning | 18 | 🔴 not integrated |
| 16 | Unix-socket control RPC | 20 | 🔴 not integrated |
| 17 | HTTP/2 proxy | 26 | ✅ fixed + verified |
| 18 | Zero-downtime deploy (drain) | 22 | ✅ wired |
| 19 | HTTP/3 proxy | 27 | 🔴 not integrated (not compiled) |

## Proxy → kamal-proxy parity (corrected)

| Capability | Status |
|---|---|
| HTTP/1.1 reverse proxy | ✅ (server startable after ContextBag fix) |
| HTTP/2 termination | ✅ verified end-to-end |
| HTTP/3 (QUIC) termination | 🔴 not integrated |
| Let's Encrypt ACME (DNS-01) | 🔴 protocol code only, not wired |
| Zero-downtime deploys (drain) | ✅ wired |
| TCP/UDP health checks | ✅ |
| Weighted round-robin (smooth WRR) | ✅ |
| Sticky sessions (cookie) | ✅ wired |
| Unix-socket admin interface | 🔴 not wired |
| State persistence (cross-restart) | 🔴 not wired |
| TCP/UDP passthrough | ✅ (standalone listeners) |
| WebSocket upgrade relay | ✅ wired |
| SSL redirect (80→443) | ✅ wired |
| Structured access logging | ✅ |

## Test results (corrected)

The previously reported "379 passed, 0 failed" is not reproducible: the platform
suites did not compile under default features (fixed now), and the platform
default build (`docker`+`wireguard`) still fails to **link** pending the
russh-default work. Verified this session:

| Suite | Result |
|-------|--------|
| foundation_proxy (all binaries) | ✅ green after the fixes above |
| foundation_deployment_platform (`--features docker`, non-Docker) | compiles + links; Docker-gated tests `#[ignore]` |
| foundation_deployment_platform (default `docker`+`wireguard`) | 🔴 link failure (BoringSSL/OpenSSL) |

## Remaining work (tracked)

1. 🔴 Make russh the default OpenSSL-free SSH backend; route **all** usage through
   traits (no direct `ssh2::` calls outside the gated ssh2 backend); resolve the
   platform link failure.
2. 🔴 Replace `ssh2-config` host-config parsing (pure-Rust or our own) to drop the
   `git2/libssh2` OpenSSL pull.
3. 🔴 Wire F14 state persistence into `ProxyServer`.
4. 🔴 Wire F16 unix-socket control RPC into `ProxyServer`.
5. 🔴 Wire F15 ACME cert provisioning into the TLS path (`AcmeCertManager`).
6. 🔴 Integrate F19 HTTP/3 (define the `quic` feature + deps, fix the handler,
   wire via `ServerApp::Any`, add a test).

## Related specs

- **[Spec 54](../54-foundation-deployment-docker/)** — Our own Docker client
- **[Spec 41](../41-connectrpc/)** — ConnectRPC foundation
- **[Spec 55](../55-foundation-wireguard/)** — WireGuard mesh (source of the BoringSSL dep)
