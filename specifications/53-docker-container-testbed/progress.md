# Progress — Spec 53: Docker Container Testbed

**Last updated:** 2026-07-10

## Crates built (all compile clean)

### foundation_deployment_platform ✅
- Provider trait: associated types Handle + Config + Error
- DockerProvider: wraps ContainerHandle to implement Provider
- ContainerHandle: RAII (start_async/start, shutdown, is_running, Drop)
- ContainerConfig: builder (image, port, env, network, wait, memory, cpus)
- ContainerGroup: multi-container topologies
- DockerClient: bollard wrapper (local + SSH stub)
- NetworkHandle: create_or_find, find, connect, remove
- WaitFor: Port (TCP backoff), Stdout (logs stream), Http (SimpleHttpClient), Composite
- Integration tests: Redis start/stop/PING/SET/GET/Drop

### foundation_deployment_cloudflare ✅
- DnsRecord, Zone, DnsRecordType, ZoneStatus domain types
- CloudflareError + cf_err() helper (foundation_errstacks)
- CloudflareClient: from_env(), token(), http(), domain()
- dns_ops: list_dns_records, upsert_dns_record, delete_dns_record, bootstrap_domain
- Auth injection via FnOnce closure on ClientRequestBuilder
- Wraps auto-generated valtron TaskIterator functions from zones/mod.rs

### foundation_sshkit ✅
- Host: parse user@host:port, key/password/agent auth, proxy jump
- Command: shell builder (env, within, as_user, pty, background), to_shell_command()
- CommandResult: exit_code, stdout, stderr, runtime
- Backend trait: execute, upload, download
- Ssh2Backend: channel exec, SCP send/recv, uses ConnectionPool
- ConnectionPool: session cache with idle timeout, auth chain
- Runner: Parallel, Sequential, Group strategies
- RusshBackend (feature `russh-backend`): pure-Rust async backend — execute,
  SFTP upload/download, pubkey auth. Confirmed working (not a stub).

**Test coverage (2026-07-10):**
- Unit tests: host, command, pool (pre-existing).
- `tests/ssh2_backend_tests.rs` — Ssh2Backend against a real
  `lscr.io/linuxserver/openssh-server` container via the
  `foundation_deployment_platform` testbed. Covers execute (success, non-zero
  exit, stdout+stderr capture), upload↔download round-trip, wrong-password auth
  rejection (all `#[ignore]`, need Docker) + connection-refused (Docker-free).
- `tests/runner_tests.rs` — Parallel/Sequential/Group result-shape + per-host
  command-builder invocation (Docker-free, unreachable hosts) plus all three
  strategies against a real container (`#[ignore]`).
- `tests/russh_backend_tests.rs` (feature-gated `russh-backend`) — execute +
  SFTP round-trip against a container trusting a generated ed25519 key, plus a
  connection-refused path. ssh-keygen-gated (skips cleanly if absent).
- Container tests use `ContainerHandle::start_async` directly (the
  `#[docker_container]` macro drops its handle before the body and exposes no
  mapped port, so it is unusable where a test needs the host port).
- Runs (Docker available, `--profile uat`): default suite 5 non-Docker tests
  pass; `--ignored` ssh2 5/5 pass, runner 1/1 pass; `--features russh-backend
  --include-ignored` russh 3/3 pass. Zero warnings from the crate.

### foundation_proxy ✅
- ProxyConfig, ServiceConfig, SslConfig, BackendTarget, HealthCheckConfig
- ProxyError enum
- ProxyServer::start() with host dedup validation
- Three config paths: builder, load_file(), proxy! macro deferred

### foundation_macros ✅
- #[docker_container] proc macro (~200 lines)
- Parses: image, port, network, wait_stdout, wait_port, stop_timeout, memory, cpus, name
- Supports sync + async fn
- Re-exported from foundation_deployment_platform

## Next steps
- Testbed migration: move vms/ from foundation_testbed → foundation_deployment_platform
- ProxyServer HTTP routing + TLS cert provisioning
- CloudflareClient: deserialise HashMap responses into typed DnsRecord
