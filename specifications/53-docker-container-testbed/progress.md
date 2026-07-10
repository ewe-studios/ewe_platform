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
| Cloudflare typed records | 24 | serde, Cloudflare API — **IN PROGRESS** (generator fix + typed types) |

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

### Active: Cloudflare typed records (Decision 24)

**Root cause:** The `zones/` module was generated using an older version of the generator
(`foundation_openapi/src/unified/generator.rs`) that didn't include query param
serialization. The current generator (lines 1278-1295) DOES emit query param code
(iterates `ep.query_params`, appends `name=urlencoding::encode(v)` to URL), but:

1. The Cloudflare crate wasn't regenerated after that generator change
2. `urlencoding` crate isn't in `foundation_deployment_cloudflare` deps (it's in
   `foundation_deployment` but generated code calls it directly)
3. Feature flag mismatch: Cargo.toml defines `zones` but generated code gates on
   `cloudflare_zones`

**Implementation plan:**

**Phase A — Regenerate from OpenAPI spec (fix generator gap):**
1. Add `urlencoding = "2.1"` to `foundation_deployment_cloudflare/Cargo.toml`
2. Fix feature flag: add `cloudflare_zones = ["zones"]` to Cargo.toml
3. Run `cargo run --bin ewe_platform -- gen_api cloudflare` to regenerate zones/
4. Verify generated `list_dns_records_request` now includes query param serialization
5. Fix any compile issues in regenerated output

**Phase B — Generator improvements (medium-term, save manual work):**
- Generator `update_cargo_toml()` only manages feature flags — teach it to also add
  `urlencoding` dep when query params are present (via flag or auto-detected)
- Consider: convert `urlencoding::encode` to use a shared helper from
  `foundation_deployment::providers::common` so generated crates don't each need the dep

**Phase C — Typed records (Decision 24 core):**
1. Add `DnsRecordInput`, `CloudflareResponse<T>`, `CloudflareApiError`, `ResultInfo` to types.rs
2. Add `From<DnsRecord> for DnsRecordInput`
3. Fix `drive_task` to preserve HTTP status codes from `ApiError::HttpStatus`
4. Replace `record_to_body` HashMap dance with typed `DnsRecordInput`
5. Replace response extraction (`.data.get("result")`) with typed `CloudflareResponse<T>`
6. Add `find_zone` and `delete_dns_records_by_name`
7. Expand tests in types_tests.rs
8. Update lib.rs re-exports

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
