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

### F03 networking & volumes — reopened and repaired (2026-07-17)

Marked "code present", and progress.md claimed `container_integration` covered
F01/**F03**/**F04**/F05 end-to-end. It did not: that suite has three test
functions, touches no networks, no volumes and no image builds, and imported
`ContainerGroup` without using it. `NetworkHandle` had **no consumer and no test
anywhere in the workspace** — only its `mod.rs` re-export.

Not just untested — materially incomplete against decision 10, with five silent
drops (each now fixed and covered by `network_volume_integration`, 4 e2e tests):

- `ContainerConfig::network_alias()` pushed to a field **nothing ever read** —
  aliases never reached Docker. Now sent via `networking_config` at create time.
- Named volumes were **filtered out of `Binds` entirely** (mapped to `None`), so
  `named_volume()` silently wrote to the container's own layer.
- `read_only` was ignored — the bind string never got its `:ro`, so a read-only
  mount was silently writable. Added `volume_read_only()`.
- `create_or_find(.., subnet)` ignored `subnet` (`_subnet`), which decision 10
  requires be honoured. Now passed through as an IPAM config.
- A container naming a network that did not exist **failed to start**; decision
  10 step 1 says the network is created first. `start_async` now `create_or_find`s it.

`NetworkHandle` was reshaped to decision 10's API (holds its own client so Drop
can work, `connect(container_id, aliases)`, `remove_on_drop`).

Found in the client while wiring it: `network_connect` ignored aliases despite its
doc promising endpoint config, and **`network_inspect` returned a struct with no
fields** — serde accepted the daemon's JSON and discarded all of it, so callers
learned nothing. Its test only asserted the call succeeded, which is all an empty
struct allows. Now decodes into `Network`.

**And two data-corruption bugs in `foundation_netio`'s chunked parser**, which is
why the volume tests kept reading empty logs (details in
`specifications/11-foundation-deployment/features/05-gcp-cloud-run-cli-provider/CR_BYTE_INVESTIGATION.md`
→ "Update 2026-07-17"):

- It **stripped every CR byte from every chunked body**. Docker log frames put the
  payload length in the header, so a 13-byte log line carries a literal `0x0D`;
  stripping it desynced the frame and logs came back empty. Wire-proven: dockerd
  sent 21 bytes, the client returned 20.
- After the chunk-size line it ran `eat_crlf` + `eat_newlines`, both looping until
  a non-CR/LF byte, where RFC 7230 §4.1 allows **exactly one** terminator — so data
  starting with CR/LF was eaten as framing and `read_exact` pulled the delimiter in
  as content. Same defect the April GCP fix removed from `eat_escaped_crlf`, in the
  two eaters it left behind.

### F04 image management — ported off the CLI (2026-07-17)

`DockerFileConfig::build_once` shelled out to `docker build` — needs the CLI on
`PATH`, no structured errors, and bypasses the bollard-free client decision 01
exists for. `DockerFileConfig` had no consumer and no test, so it never showed.

The native path did not work either: `image_build` sent **no body**, but `/build`
takes the context as a tar stream — so it could not build anything — and it threw
the response away (`Ok(Vec::new())`), hiding that the daemon reports **build
failures inside a 200**.

Now (decision 05, "How the build runs"): `ContextTar` packs the context (dir,
inline Dockerfile, or both), `image_build` streams it and parses the progress for
errors (`DockerError::BuildFailed` → platform `ImageBuild`), and
`DockerFileConfig::backend(..)` selects `Classic` (dockerd `POST /build`, default)
or `BuildKit(addr)` (buildkitd `Solve`, `dockerfile.v0`, behind the `buildkit`
feature).

`buildkit` collided with the BoringSSL/OpenSSL conflict again — it reaches
BoringSSL via `jwt-simple`→`foundation_auth`→`foundation_connectrpc`, while
`foundation_deployment_docker`'s `docker` feature pulled `foundation_sshkit`
(libssh2/OpenSSL) unconditionally. The SSH transport now lives behind its own
`ssh` feature, so `docker + buildkit` links.

Coverage: `foundation_deployment_docker::image_build_integration_tests` (4 e2e:
inline build, context files uploaded and COPY-able, build args, and a failing
build surfacing as an error) and `foundation_deployment_platform::image_build_integration`
(6: builds a runnable image and reads back what the Dockerfile made, context +
args, `was_cached` on the second call, failing build, untagged rejected, plus 2
BuildKit-backend tests against a real buildkitd — skipped when none is reachable).

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
| 03 | Networking & Volumes | ✅ **fixed + verified e2e** (was "code present"; `NetworkHandle` was dead, 5 options silently dropped — see below) |
| 04 | Image Management | ✅ **ported off the CLI + verified e2e** (was "code present"; `build_once` shelled out to `docker build` — see below) |
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
| 15 | ACME cert provisioning | 18 | ✅ wired + verified (Let's Encrypt path only — TLS termination itself is unproven and the Cloudflare provider is unbuilt: [spec 56 feature 07](../../56-vps-deployment-providers/features/07-tls-termination-and-cloudflare/feature.md)) |
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
| **TLS termination (HTTPS in → backend)** | 🔴 **wired, never exercised** — no test makes a TLS connection through the proxy; see [spec 56 feature 07](../../56-vps-deployment-providers/features/07-tls-termination-and-cloudflare/feature.md) |
| Let's Encrypt ACME | ✅ wired + verified (a mock CA signs the CSR → the cert builds a real `SSLAcceptor`); the DNS-01 setter is a caller hook |
| **Cloudflare cert provider** | 🔴 **not implemented** — `build_cert_manager` errors for `SslProvider::Cloudflare`; decision 18 wants ACME DNS-01 via the Cloudflare API (wildcards), see [spec 56 feature 07](../../56-vps-deployment-providers/features/07-tls-termination-and-cloudflare/feature.md) |
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

---

## Final completeness review (2026-07-17)

A full sweep of all 19 features and 29 decisions, after the F02/F03/F04 repairs.

### Verified green

Every claim in this file now names a test that exists and passes. Re-run this
session against real daemons:

| Area | Evidence |
|---|---|
| Part A (F01–F05) | `container_integration`, `docker_macro_tests` (3), `network_volume_integration` (4), `image_build_integration` (6) |
| Part B (F09–F11) | `ssh_backend_tests` (5), `runner_integration_tests` (1), `docker_provider_tests` (1) — all vms-gated, Docker-backed |
| Proxy data plane | `proxy_integration_tests` **9/9** (`--features docker-tests`): forward, WRR, host/path routing, 404/503, XFF + hop-by-hop, TCP **and UDP** passthrough, health eject/readmit |
| Proxy features | `proxy_features_tests` (sticky, drain, SSL-redirect, WebSocket), `persistence_restart_tests`, `control_socket_tests`, `acme_provisioning_tests` |
| H2/H3 | `h2_integration_tests`, `h3_integration_tests` + `foundation_http::http3_server_tests` (`--features quic`) |
| Remote Docker over SSH | `ssh_transport_tests` (info + container round-trip) vs docker-in-docker + sshd |
| Client | `foundation_deployment_docker` 101 passed (docker + integration-tests) |

Structural checks: **no `unimplemented!`/`todo!`** anywhere in the four crates,
and **no unused public type** in any hand-written module — the dead-module problem
that hid F03 (`NetworkHandle`) and F04 (`image.rs`) is gone. Every test file named
in this document and in `features/README.md` exists.

### Pending work — moved to [spec 56](../../56-vps-deployment-providers/)

**This spec is done.** Four items the review surfaced were never in its feature
set (01–19) and are pending; they moved to
**[spec 56 — VPS Deployment Providers](../../56-vps-deployment-providers/)**
rather than hold spec-53 open:

| Gap | Now |
|---|---|
| **TLS termination is wired but never exercised.** No test makes a TLS connection through the proxy; the ACME tests prove a cert *builds an acceptor*, and the SSL-redirect test only drives plain HTTP. | [spec 56, feature 07](../../56-vps-deployment-providers/features/07-tls-termination-and-cloudflare/feature.md) |
| **`SslProvider::Cloudflare` not implemented** — `build_cert_manager` errors out, and the message describes Origin CA while decision 18 asks for ACME DNS-01 *via* the Cloudflare API (wildcards). Unwired, not unbuildable: `Dns01Setter` + `upsert_dns_record`/`Txt` both exist. | [spec 56, feature 07](../../56-vps-deployment-providers/features/07-tls-termination-and-cloudflare/feature.md) |
| **Cloudflare DNS ops untested** — 12 tests, all on typed records (decision 24 ✅); `list/upsert/delete/find_zone/bootstrap_domain` have none. `find_zone(_domain)` **ignores its argument** and reports on the configured zone id instead — the same silently-dropped-parameter family as F03's `subnet` and F04's `memory`. | [spec 56, feature 07](../../56-vps-deployment-providers/features/07-tls-termination-and-cloudflare/feature.md) |
| **Decision 06 (cloud deployment) never built** — no cloud-init, no VM provisioning. Nothing blocks it: 4 of 5 steps exist and are verified; only VM creation needs an account. | [spec 56](../../56-vps-deployment-providers/) — features 01–06 (Hetzner/DigitalOcean/Linode crates, cloud-init + bootstrap, hardening, `VpsDeployment`) |

### Not gaps (checked, resolved)

- **The 3 `#[ignore]`d "known-fragile mock" tests** (`exec_create`,
  `network_create`, `volume_create`) are **redundant**: all three endpoints are
  covered against a **real daemon** by `network_volume_exec_integration_tests` and
  `integration_tests`, which pass. The mock races; the endpoints are proven.
- **Decision 29** (Docker test `block_on`) — "Investigated — resolved by spec-54".
- **Decisions 15/24** (Cloudflare transition, typed DNS records) — the typed
  records of 24 are covered; the transition's runtime surface is item 3 above.

## Related specs

- **[Spec 54](../54-foundation-deployment-docker/)** — Our own Docker client
- **[Spec 41](../41-connectrpc/)** — ConnectRPC foundation
- **[Spec 55](../55-foundation-wireguard/)** — WireGuard mesh (source of the BoringSSL dep)
