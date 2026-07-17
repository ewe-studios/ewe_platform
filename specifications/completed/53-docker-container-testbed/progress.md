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

### F02 `#[docker_container]` — reopened and repaired (2026-07-16)

Marked "code present", but **nothing in the workspace ever used the macro**, so
it had never been expanded even once. It did not compile, and its grammar was a
fraction of what decision 08 documents. Repaired and now proven end-to-end
against a real daemon (`docker_macro_tests`, 3 tests: sync, stacked, async — each
does a real Redis `PING`/`+PONG` over the mapped port, so "container is usable",
not merely "start returned `Ok`").

Codegen bugs (all fatal, all invisible without a call site):
- Emitted `#vis #fn_name()` with **no `fn` keyword** → "missing `fn`" on any use.
- Sync skip path returned `Default::default()`, requiring `ContainerHandle:
  Default`, which does not exist → skip path could not compile. Now carries the
  skip out of the async block as `Option`.
- Async arm emitted `{ body }.await` — a block is not a future (the decision doc's
  sketch had the same error). In an `async fn` the body needs no wrapping.

Grammar gaps vs decision 08 (`port_mapped` and `env` were dead `let` bindings —
declared non-`mut`, never assigned, so both were rejected as unknown keys):
added `port_mapped`, `env`, `volume`, `wait_http`, `wait_timeout`, `always_pull`,
`required`, `port_udp`, and multi-strategy composite waits.

Runtime bugs found by finally exercising this path:
- **Containers leaked on every failed start.** After `create_container`, no
  `ContainerHandle` owns the container yet, so nothing ran Drop's teardown — a
  failed start, port conflict, or readiness timeout stranded a container that
  kept holding its host ports, making every later run fail with an opaque
  `docker API error (status 500)`. `start_async` now removes it on any failure.
- **`memory = "256m"` was silently ignored** — `parse::<i64>()` fails on the
  suffixed form the spec documents and `.ok()` swallowed it, so the container ran
  unlimited. Added `parse_memory_bytes` (docker `--memory` spellings, binary
  multipliers); an unparseable limit is now an `InvalidConfig` error, never a
  silent default. Verified applied: `HostConfig.Memory=268435456` on the live
  container.

Decision 08 was corrected to match reality: the sync generated-shape sketch
(`block_on_future`, not `.await` in a sync fn), the async arm, `wait_port` being
a container-side port, and the multi-container start order (the doc claimed
"outermost starts first"; the *lowest* attribute is outermost at runtime and
starts first — measured via `docker events`).

#### Handle exposure (design change, decided 2026-07-17)

The repair above left a real gap: the macro kept the `ContainerHandle` in a
hidden local, so a body could only reach its container by pinning a host port
with `port_mapped` — which is what made two of these tests collide in parallel.
Handles are now passed to the body (decision 08, "Reaching the containers"):

- **The annotated fn must take exactly one parameter**, receiving a
  `ContainerGroup` of every started container; a zero-arg fn is a compile error.
  `ContainerGroup` is reused rather than a new type — it already modelled
  "several containers torn down together in reverse start order" and only needed
  `empty()`/`insert()` for the macro to fill it. The group owns the handles, so
  its Drop replaces the per-container guard.
- **All containers are declared in one invocation**, one `{ ... }` block each
  (bare `key = value` remains shorthand for a single container); stacking the
  attribute is now a compile error. Stacking made each attribute an independent
  expansion blind to its siblings, which forced the group to be threaded through
  the nest (each expansion inferring whether it was outermost), inverted the
  start order (the *lowest* attribute started first), and left duplicate lookup
  keys detectable only at runtime. One invocation sees the whole set: containers
  start in the order written, and a duplicate `as` key is a compile error.
- **Lookup is by logical key (`as = "cache"`), never the Docker name.** Docker
  names are global to the daemon, so keying on `name` would force every test that
  wants a handle to pin a globally-unique name and collide with a 409 in parallel
  — the same failure this change removes for ports. A logical key never reaches
  the Docker API, so containers stay auto-named and any number of runs can share
  the key. Duplicate keys panic on insert rather than resolving arbitrarily.
- **Ports are Docker-assigned by default**: `port = 6379` plus the new
  `ContainerHandle::address(6379)` → `127.0.0.1:<assigned>`. No test pins a host
  port any more, so parallel runs cannot conflict.
- `as` is a Rust keyword, so the attribute parser is hand-rolled: `syn::Meta`
  rejects `as = "cache"` ("expected identifier, found keyword `as`"). Peeking
  `Token![as]` accepts it, and spans on unknown keys improved as a side effect.

Coverage: `docker_macro_tests` rewritten onto the group API (the multi-container
test asserts both containers are in one group, get distinct Docker-assigned
ports, are each independently reachable via `address()`, and start in the order
written); plus four `foundation_macros` trybuild compile-fail cases — missing
parameter, unknown attribute key, stacked attributes, and duplicate `as` key.

### Reopened — repair status
| # | Feature | Status |
|---|---------|--------|
| ✅ F14 | State persistence | **Fixed:** `ProxyConfig::persist_to(dir)`; `ProxyServer` restores backend drain/pause on start and writes admin changes through `ProxyState`'s store. E2E test `persistence_restart_tests` (drain → restart → still draining). |
| ✅ F16 | Unix-socket control RPC | **Fixed:** `ProxyConfig::control_socket(path)`; `ControlSocket` started in `ProxyServer::start`, stopped on shutdown. E2E test `control_socket_tests` (status/list/drain over the socket). |
| ✅ F15 | ACME cert provisioning | **Fixed:** `AcmeCertManager` (P-256 account key/JWS + rcgen CSR) drives the full `AcmeClient` flow; `tls::build_cert_manager` selects it for the `LetsEncrypt` provider; `ProxyConfig::acme(dns01_setter, directory_url)`. E2E test `acme_provisioning_tests` against a real mock ACME CA that signs the CSR → provisioned cert+key build a real `SSLAcceptor`. |
| ✅ F19 | HTTP/3 proxy | **Fixed — the H3 serving layer was built from scratch.** Added the QUIC/H3 server to `foundation_http` (`server::serve_h3`: `QuicDriver` endpoint pump + H3 accept/dispatch task, plus `new_h3_serve`/`route_any_h3`). Rewrote `h3_proxy.rs` to forward via the shared client and emit proper QPACK frames. Wired into `ProxyServer` (`ProxyConfig::h3_bind`, `quic` feature). Two e2e tests: `foundation_http::http3_server_tests` (QUIC client ↔ echo handler) and `foundation_proxy::h3_integration_tests` (H3 client → proxy → HTTP/1.1 backend). |

### Resolved — dependency / backend defect
| Item | Resolution |
|------|------------|
| ✅ Platform default build link failure | `docker`+`wireguard` (default) failed to **link**: `ssh2`/libssh2 needs OpenSSL 3 symbols; `foundation_wireguard`→`boring` provides BoringSSL, which lacks them (two incompatible libcryptos in one binary). Investigation showed `boring` is used only by wireguard's `native/{bootstrap,mtls}.rs`, and the platform needs only the pure-Rust key/seed types. **Fixed:** gated `boring` behind a default-on `native-mesh` feature in `foundation_wireguard` and made the platform depend with `default-features = false` → no BoringSSL in the platform binary → ssh2/OpenSSL links. Platform default suite now green. |
| ✅ SSH backend | `russh` was a half-wired alternative `Backend` (its sync trait path panicked — tokio-only, never caught because tests were Docker-`#[ignore]`). With the link conflict resolved by gating boring, russh was **removed entirely**; ssh2 (libssh2) is the sole backend. Also fixed two environment-coupled `host_tests` (default user is `$USER`/`$LOGNAME`, not hardcoded `root`). |

## Docker runtime (Part A) — implemented

| # | Feature | Status |
|---|---------|--------|
| 01 | Runtime Library (`ContainerHandle`/`Config`/`DockerError`/`NetworkHandle`) | ✅ code present |
| 02 | Proc Macro `#[docker_container]` | ✅ **fixed + verified e2e** (was "code present" but did not compile — see below) |
| 03 | Networking & Volumes | ✅ code present |
| 04 | Image Management | ✅ code present |
| 05 | Wait Strategies | ✅ code present |
| — | Bollard → `foundation_deployment_docker` migration | ✅ (no bollard dep) |

> Part A Docker-backed tests are `#[ignore]` (need a Docker daemon). With Docker
> up they now **pass end-to-end**: `container_integration` (ContainerHandle
> lifecycle, WaitFor readiness, Drop cleanup — F01/F03/F04/F05) and
> `ssh_backend_tests` + `runner_integration_tests` (ssh2 execute/upload/download/
> auth, Runner strategies — F09) all green against real containers.

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
| 14 | State persistence | 21 | ✅ wired + verified |
| 15 | ACME cert provisioning | 18 | ✅ wired + verified |
| 16 | Unix-socket control RPC | 20 | ✅ wired + verified |
| 17 | HTTP/2 proxy | 26 | ✅ fixed + verified |
| 18 | Zero-downtime deploy (drain) | 22 | ✅ wired |
| 19 | HTTP/3 proxy | 27 | ✅ built + wired + verified |

## Proxy → kamal-proxy parity (corrected)

| Capability | Status |
|---|---|
| HTTP/1.1 reverse proxy | ✅ e2e (Docker integration suite) |
| HTTP/2 termination | ✅ verified end-to-end |
| HTTP/3 (QUIC) termination | ✅ built + verified (foundation_http H3 server) |
| Let's Encrypt ACME | ✅ wired + verified (DNS-01 setter is a hook) |
| Zero-downtime deploys (drain) | ✅ wired |
| TCP/UDP health checks | ✅ e2e (Docker health eject/readmit) |
| Weighted round-robin (smooth WRR) | ✅ e2e (Docker round-robin) |
| Sticky sessions (cookie) | ✅ e2e (proxy_features) |
| Unix-socket admin interface | ✅ wired + verified |
| State persistence (cross-restart) | ✅ wired + verified |
| TCP/UDP passthrough | ✅ e2e (Docker passthrough + completion_splice) |
| WebSocket upgrade relay | ✅ e2e (proxy_features) |
| SSL redirect (80→443) | ✅ e2e (proxy_features) |
| Structured access logging | ✅ |

## Test results (corrected)

The previously reported "379 passed, 0 failed" was not reproducible (the platform
suites didn't compile/link). After this session's fixes:

| Suite | Result |
|-------|--------|
| foundation_proxy (default) | ✅ all green (F14/F15/F16 + SSL-redirect/sticky/drain/WebSocket e2e) |
| foundation_proxy (`--features quic`) | ✅ H3 server + H3 proxy e2e |
| foundation_proxy (`--features docker-tests`) | ✅ 9/9 real reverse-proxy e2e (forward, WRR, routing, 503, XFF, passthrough, health) |
| foundation_http (`--features quic`) | ✅ H3 server e2e (QUIC client ↔ echo) |
| foundation_deployment_platform (Docker: container + ssh + runner) | ✅ Part A F01-F05/F09 e2e |
| foundation_sshkit | ✅ green (russh removed) |
| foundation_testbed | ✅ builds under default features |

## Remaining work (tracked)

_SSH/BoringSSL link conflict — ✅ resolved (gated `boring` in wireguard; removed
russh). Platform default suite green._

1. ✅ Wire F14 state persistence into `ProxyServer`. — done
2. ✅ Wire F16 unix-socket control RPC into `ProxyServer`. — done
3. ✅ Wire F15 ACME cert provisioning into the TLS path (`AcmeCertManager`). — done
4. ✅ Integrate F19 HTTP/3 — built the QUIC/H3 serving layer in `foundation_http`,
   fixed `h3_proxy.rs`, wired into `ProxyServer` (`h3_bind` + `quic` feature),
   e2e-tested at both the http and proxy levels. — done

**All reopened proxy features (F14/F15/F16/F19) are now wired and verified.**

## Related specs

- **[Spec 54](../54-foundation-deployment-docker/)** — Our own Docker client
- **[Spec 41](../41-connectrpc/)** — ConnectRPC foundation
- **[Spec 55](../55-foundation-wireguard/)** — WireGuard mesh (source of the BoringSSL dep)
