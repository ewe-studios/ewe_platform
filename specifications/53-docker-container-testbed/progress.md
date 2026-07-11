# Progress — Spec 53: Docker Container Testbed

**Last updated:** 2026-07-10

## All crates compile clean — 8/8 Docker proxy tests — 6/6 Docker sshkit tests

### foundation_proxy ✅ — Stage 1 data plane COMPLETE
- ProxyServer::start(): validates config (no duplicate (host, prefix) routes), binds
  TCP listener, spawns HTTP front end via foundation_http::HttpServer, spawns
  per-backend health probe threads, returns shutdown-capable handle.
- ProxyHandler (ServeFactory): host+path routing → weighted round-robin backend
  selection (smooth WRB, eligibility gates: Active + healthy + under capacity).
- HTTP forwarding: strip hop-by-hop (RFC 7230 §6.1), append X-Forwarded-For /
  X-Forwarded-Proto / X-Forwarded-Host, set upstream Connection: close.
- Health probes: per-backend OS threads, hysteresis state machine
  (healthy/unhealthy threshold), TCP-connect for tcp:// backends, HTTP GET for
  http/https.
- TCP passthrough: TcpPassthrough listener for tcp:// backends — raw byte splice,
  no HTTP semantics.
- Weighted round-robin load balancer (smooth WRB, nginx algorithm).
- BackendLease: RAII in-flight refcount, CAS-based capacity enforcement.
- Wildcard host matching (*.example.com) with exact-host tiebreak.
- Longest-path-prefix routing with segment-boundary matching.
- Config: three paths (builder, load_file TOML, proxy! macro deferred).
- Types: BackendTarget (URL+weight+max_conns), BackendProtocol (http/https/tcp
  inferred from scheme), BackendState (Active/Draining/Paused), SslConfig.

**Test coverage (2026-07-10):**
- Unit tests: config (6), balancer/runtime (4), backend_target (4),
  forward_headers (2), health_state (5), router (5) — **26 passed, 0 failed**.
- Docker integration tests (8/8 against real containers):
  - test_forward_roundtrips_to_backend ✅ (http-echo)
  - test_round_robin_spreads_across_backends ✅ (2 × http-echo)
  - test_host_routing_and_unknown_host_404 ✅ (2 × http-echo, host routing)
  - test_path_prefix_longest_wins ✅ (2 × http-echo, / + /api prefixes)
  - test_tcp_passthrough_roundtrips_raw_bytes ✅ (TcpPassthrough → http-echo)
  - test_hop_by_hop_stripped_and_xff_added ✅ (http-echo, hop-by-hop strip)
  - test_no_healthy_backend_returns_503 ✅ (health probe → 503)
  - test_health_ejects_and_readmits_backend ✅ (probe marks unhealthy → 503)
- Zero warnings from the crate (lib + tests).

### foundation_sshkit ✅
- Host: parse user@host:port, key/password/agent auth, proxy jump
- Command: shell builder (env, within, as_user, pty, background), to_shell_command()
- CommandResult: exit_code, stdout, stderr, runtime
- Backend trait: execute, upload, download
- Ssh2Backend: channel exec, SCP send/recv, uses ConnectionPool
- ConnectionPool: session cache with idle timeout, auth chain
- Runner: Parallel, Sequential, Group strategies
- RusshBackend (feature `russh-backend`): pure-Rust async backend — execute,
  SFTP upload/download, pubkey auth.

**Test coverage (2026-07-10):**
- Unit: host (8), command (8), pool (3), runner (4 non-Docker) — **23 passed**.
- Docker-backed SSH tests moved to `foundation_deployment_platform/tests/` to
  remove the conceptual inversion (leaf primitive depending on its consumer).
  - `ssh_backend_tests.rs`: execute + stderr + nonzero + upload/download + wrong-password — **5/5**.
  - `runner_integration_tests.rs`: all strategies against real container — **1/1**.

### foundation_deployment_platform ✅
- Provider trait: associated types Handle + Config + Error
- DockerProvider: wraps ContainerHandle to implement Provider
- ContainerHandle: RAII (start_async/start, shutdown, is_running, Drop)
- ContainerConfig: builder (image, port, env, network, wait, memory, cpus)
- ContainerGroup: multi-container topologies
- DockerClient: bollard wrapper (local + SSH stub)
- NetworkHandle: create_or_find, find, connect, remove
- WaitFor: Port (TCP backoff), Stdout (logs stream), Http (SimpleHttpClient), Composite
- ensure_image: pull-if-absent + always_pull (fixes "404: No such image" on cold cache)
- Integration tests: Redis start/stop/PING/SET/GET/Drop

### foundation_deployment_cloudflare ✅
- DnsRecord, Zone, DnsRecordType, ZoneStatus domain types
- CloudflareError + cf_err() helper (foundation_errstacks)
- CloudflareClient: from_env(), token(), http(), domain()
- dns_ops: list_dns_records, upsert_dns_record, delete_dns_record, bootstrap_domain
- Auth injection via FnOnce closure on ClientRequestBuilder

### foundation_macros ✅
- #[docker_container] proc macro (~200 lines)
- Parses: image, port, network, wait_stdout, wait_port, stop_timeout, memory, cpus, name
- Supports sync + async fn
- Re-exported from foundation_deployment_platform

## pipeline (foundation_core)
- pipe_mapped.rs: MappedSender/FilterMapReceiver with borrow-based transforms
  (zero-allocation, no extra valtron tasks). Extracted from pipe.rs for clarity.

## Spec-53 decisions (29 total — all written)

| # | Decision | Status |
|---|----------|--------|
| 01–13 | Testbed infrastructure, networking, cloud | ✅ Implemented |
| 14 | foundation_proxy architecture | ✅ Stage 1 done |
| 15 | Cloudflare client transition | ✅ Implemented |
| 16 | Deployment crate split | ✅ Implemented |
| 17 | UDP proxying | ✅ Implemented |
| 18 | TLS termination + auto-cert | 📋 Stage 2 |
| 19 | `proxy!` macro | 📋 Stage 2 |
| 20 | Unix-socket RPC | 📋 Stage 3 |
| 21 | State persistence | 📋 Stage 3 |
| 22 | Zero-downtime deploy + canary | 📋 Stage 3 |
| 23 | Writer affinity | 📋 Stage 3 |
| 24 | Cloudflare typed DNS records | 📋 Stage 2 |
| 25 | SSL redirect (HTTP→HTTPS) | 📋 Bundled with 18 |
| 26 | HTTP/2 proxy integration | 📋 Stage 4 |
| 27 | HTTP/3 (QUIC) proxy integration | 📋 Stage 4+ |
| 28 | Testbed migration: vms/ → platform | 📋 Minor refactor |
| 29 | Docker test `block_on` tokio fix | 📋 Bug fix |

## Stage 2 (next up)

| Feature | Decision | Dependencies |
|---------|----------|-------------|
| TLS + auto-cert | 18 | rustls, acme-micro, VFS, CloudflareClient |
| SSL redirect | 25 | Bundled with 18 — same PR |
| ~~`proxy!` macro tests~~ | 19 | ✅ **COMPLETE** — 7 compile-fail + 13 round-trip tests |
| Cloudflare typed records | 24 | ✅ **COMPLETE** — typed types + dns_ops refactor |

### ✅ `proxy!` macro test suite (Decision 19) — COMPLETE 2026-07-10

**Bugs found and fixed during testing:**
- `ServiceBlock::parse` missing `:` consumption between name and braced block
- `SslBlock::parse` missing trailing comma consumption inside `lets_encrypt`/`cloudflare`/`static_cert` blocks, and between `cert`/`key` fields

**Test coverage:**
- 7 trybuild compile-fail tests in `foundation_macros/tests/trybuild/fail/proxy_*.rs`
  (missing domain, missing public_ip, unknown top-level key, unknown service key,
  unknown health_check key, unknown SSL provider, missing service host)
- 13 round-trip tests in `foundation_proxy/tests/proxy_macro_tests.rs`
  (minimal, bind_addr, lets_encrypt/static_cert/cloudflare/none SSL, empty services,
  no backends, multiple backends, multiple services, path_prefix, health_check all fields,
  type check)
- All existing tests pass (28 macros + 52 trybuild + 41 proxy)

### ✅ Generator improvements — COMPLETE 2026-07-11

**What was done:**
- **gen_api binary** moved from `bin/platform` to `foundation_codegentools/src/cli/`
  with `command()` + `register()` API. Standalone binary at `src/bin/genapi.rs`.
- **Split-out provider routing**: `SPLIT_OUT_PROVIDERS` table maps provider names to
  crate paths. Adding new split-out providers is one line.
- **Query param fix**: generator now uses original spec name (`name.exact`) for URL
  keys instead of sanitized (`name_exact`). All 44 Cloudflare group modules regenerated.
- **Typed shared resources**: `generate_shared_module` now resolves schemas from the
  OpenAPI spec and generates properly typed structs instead of `HashMap<String, Value>`
  stubs. `resolve_schema_key()` bridges PascalCase ↔ snake_case naming conventions.
- **`generated/` isolation**: all generated files now live under `generated/` subdirectory.
  Hand-written code is never overwritten. Monolith providers get a thin parent `mod.rs`
  created once (never overwritten).
- **Result**: Cloudflare shared module went from 45 HashMap wrappers → 45 typed structs
  (19 remaining HashMap fallbacks for truly unresolvable allOf/oneOf schemas).

## Stage 3 (control plane)

| Feature | Decision | Dependencies |
|---------|----------|-------------|
| Unix-socket RPC | 20 | foundation_nativeapis |
| State persistence | 21 | foundation_db |
| Zero-downtime deploy | 22 | RPC (20) + state persistence (21) |
| Writer affinity | 23 | Cookie parsing in handler |

## Stage 4 (protocol upgrades)

| Feature | Decision | Dependencies |
|---------|----------|-------------|
| HTTP/2 proxy | 26 | foundation_netio HTTP/2 (landed) |
| HTTP/3 (QUIC) proxy | 27 | foundation_netio QUIC driver (F33) |

## Housekeeping

| Task | Decision |
|------|----------|
| Testbed vms/ migration | 28 |
| Docker test tokio fix | 29 |
