---
feature: "SSH transport for DockerClient (ssh://user@host via docker system dial-stdio)"
description: "DockerClient::connect_ssh — sshkit exposes an exec channel as a duplex stream running `docker system dial-stdio`; HttpClient gains a caller-provided-stream transport mode; blocking ssh2 I/O is bridged to the valtron-async client. Larger — new transport shape + concurrency bridge, not crypto."
status: "complete"
priority: "low"
phase: 3
depends_on: ["02-unix-socket-transport"]
estimated_effort: "large"
created: 2026-07-15
---
# Feature 08: SSH transport (`ssh://user@host`)

## Why

`DOCKER_HOST=ssh://user@host` is Docker's way to drive a remote daemon over an
existing SSH login — no exposed TCP port, no cert management. It works by SSHing
in and running **`docker system dial-stdio`**, which bridges the command's
stdin/stdout to the remote `/var/run/docker.sock`. The client then speaks the
ordinary Docker HTTP API over that channel — the SSH channel *is* the socket.

## Why this is the larger lift (it's transport shape, not crypto)

Unlike TLS, nothing here is a wiring exercise — three genuinely new pieces:

1. **Expose an ssh2 channel as a duplex stream.** `foundation_sshkit`'s `Backend`
   trait only offers buffered `execute`/`upload`/`download`. Underneath it already
   does `session.channel_session()` + `channel.exec()`, and an ssh2 `Channel` is
   `Read + Write` — but it is not surfaced. Add an API that opens a channel,
   `exec("docker system dial-stdio")`, and hands back the live channel (keeping the
   owning `Session` alive alongside it).

2. **Inject a pre-established stream into the HTTP client.** `HttpClientBuilder`
   always *dials* (TCP or unix); it has no "speak HTTP over this stream I give you"
   mode. Add one. The BuildKit `H2Socket` enum (foundation_connectrpc) and
   `RustlsConnector::from_tcp_stream` (foundation_netio) are precedents for
   wrapping a caller-provided stream, but this is for the HTTP/1.1 client path.

3. **Bridge blocking ssh2 I/O to the valtron-async client.** ssh2 (libssh2) is
   synchronous; reads/writes on the channel would block a valtron pool worker. Use
   the dedicated-OS-thread + pipe pump pattern from the BuildKit session
   (`buildkit/session.rs`) — the subtle, error-prone part. **Alternative:**
   `foundation_sshkit`'s `russh-backend` (pure-Rust, async) could carry the channel
   as a native future and skip the bridge entirely; evaluate it first.

## Scope

| Crate | Change |
|-------|--------|
| `foundation_sshkit` | A `dial` / channel-stream API: open an exec channel and return it as a `Read + Write` duplex tied to a kept-alive session. |
| `foundation_netio` | `HttpClientBuilder` (or `Connection`) gains a "connect over this caller-provided stream" transport mode. |
| `foundation_deployment_docker` | `DockerClient::connect_ssh(url)`: parse `ssh://user@host:port`, open the sshkit channel on `docker system dial-stdio`, wire it as the HTTP transport (blocking→async bridge), `base_url = http://localhost`. `connect_with_defaults` handles `ssh://`. |

## Design sketch

```rust
// sshkit: expose a raw channel stream
let session = backend.session(&host)?;                 // authenticated ssh2 Session
let channel = session.dial("docker system dial-stdio")?; // Read + Write duplex

// docker: bridge the blocking channel into the async HTTP client
let http = HttpClientBuilder::new()
    .with_stream(bridge_blocking_to_async(channel))    // new transport mode
    .read_timeout(Duration::from_secs(120))
    .build();
DockerClient { http, remote_host: Some("localhost".into()), .. }
```

Auth (key / agent / password) comes from sshkit's `Host` config (ssh2 supports all
three). The blocking→async bridge mirrors `buildkit/session.rs`: a dedicated OS
thread owns the blocking channel read/write; `std::sync::mpsc` + a valtron pipe
carry bytes to/from the async client.

## Verification

- `DockerClient::connect_ssh` compiles.
- Integration test (gated on an SSH-reachable daemon): a host running sshd + docker
  where the test user can `docker system dial-stdio`; `info()` + a container
  round-trip over `connect_ssh`. Skips cleanly when unavailable (hard in CI —
  needs sshd+docker, e.g. a local container-in-container).

## Acceptance criteria

1. sshkit exposes an exec channel as a `Read + Write` duplex.
2. `HttpClientBuilder` can speak HTTP over a caller-provided stream.
3. `DockerClient::connect_ssh` runs `info()` and a container round-trip over SSH.
4. `connect_with_defaults` honors `ssh://user@host`.

## Note

Evaluate the `russh-backend` (async) up front — if it cleanly yields an async
duplex, blocker (3) largely disappears and this drops toward a medium effort.

## Outcome (complete 2026-07-15)

All acceptance criteria met — and the design turned out **medium**, not large,
because the "blocking→async bridge" (spec blocker 3) was unnecessary:

- **ssh2 backend, not russh.** The `ssh2` `Channel` is `Send + Sync + Clone`
  (`Arc`-backed) and blocking. That is exactly the shape the HTTP client already
  drives for a Unix socket — the request task blocks a valtron worker on
  `read`/`write` per connection. No dedicated pump thread, no mpsc, no async
  bridge. russh (async) would have *added* an async→blocking bridge, so it was
  rejected.

- **Reused the `Connector` seam, not a new transport mode.** `foundation_netio`
  already had `Connector` + `HttpConnectionPool::with_connector` (F11 `WireGuard`
  overlay) feeding `create_http_connection`. The SSH transport is a `Connector`
  that opens a fresh `docker system dial-stdio` channel and returns
  `Connection::Overlay`. Added `HttpClientBuilder::with_connector` +
  `NativeHttpClient::with_connector` to reach it from the builder.

- **Poolability via `should_pool()`, not a parallel bypass.** A `dial-stdio`
  channel is one-shot, so it must never be pooled. Rather than key off transport
  type, threaded `should_pool() -> bool` through
  `OverlayReadWrite` → `Connection` → `RawStream` → `IsUnixTransport` →
  `SharedByteBufferStream`; `return_to_pool` now consults it (subsuming the old
  `is_unix()` special-case). The SSH overlay returns `false`.

- **Session reuse.** `foundation_sshkit::Dialer` caches one authenticated session
  and mints a fresh channel per request, so sequential Docker calls (deploy:
  create → start → inspect) share a single SSH handshake.

Implementation: `foundation_sshkit` (`connect_session`, `ChannelStream`,
`Dialer`); `foundation_netio` (`with_connector` on builder+client,
`should_pool` threading); `foundation_deployment_docker`
(`client/ssh.rs`: `SshOverlay` + `SshConnector`, `DockerClient::connect_ssh`,
`connect_with_defaults` `ssh://` branch).

Verified against docker-in-docker + sshd (key auth via ssh-agent) —
`tests/ssh_transport_tests.rs` passes 2/2: `info` over SSH and a container
round-trip over `docker system dial-stdio`.
