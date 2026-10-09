---
feature: "Unix socket transport for DynNetClient"
description: "Add Connection::connect_unix() factory, ClientConfig.unix_socket field, HttpClientBuilder::unix_socket() builder, and routing check in create_connection_with_proxy() — ~30 lines in foundation_netio"
status: "completed"
priority: "high"
phase: 0
depends_on: ["01-dynnetclient-preparedrequest-surface"]
estimated_effort: "small"
created: 2026-07-12
---
# Feature 02: Unix socket transport for DynNetClient

## Why

Docker daemon and BuildKitd listen on Unix sockets (`/var/run/docker.sock`,
`/run/buildkit/buildkitd.sock`). `DynNetClient` must connect over Unix sockets
as the primary transport — Unix sockets are Docker's default, require no TLS,
and authenticate via file permissions.

Decision 07 traced the full connection chain. The `Connection` enum already has
a `Unix(UnixStream)` variant with full `Read`/`Write`/timeout impls. What's
missing is the factory method, config field, builder method, and routing logic
— ~30 lines across 3 files.

## What changes

### A. `Connection::connect_unix()` factory (~8 lines)

`backends/foundation_netio/src/native/connection.rs`:

```rust
#[cfg(unix)]
pub fn connect_unix(
    path: impl AsRef<Path>,
) -> Result<Self, Box<dyn Error + Send + Sync + 'static>> {
    let stream = unix_net::UnixStream::connect(path)?;
    Ok(Self::Unix(stream))
}
```

### B. `ClientConfig.unix_socket` field (~2 lines)

`backends/foundation_netio/src/shared/client/config.rs`:

```rust
pub struct ClientConfig {
    // ... existing fields ...
    /// Connect via Unix socket instead of TCP (Docker daemon, BuildKitd).
    #[cfg(unix)]
    pub unix_socket: Option<std::path::PathBuf>,
}
```

Default: `None`.

### C. `HttpClientBuilder::unix_socket()` builder (~5 lines)

`backends/foundation_netio/src/network_client.rs`:

```rust
#[cfg(unix)]
#[must_use]
pub fn unix_socket(mut self, path: impl Into<std::path::PathBuf>) -> Self {
    self.config.unix_socket = Some(path.into());
    self
}
```

### D. Connection routing at the config-aware task sites (~40 lines, as-built)

**Implementation note (2026-07-12):** the original sketch put the check inside
`HttpConnectionPool::create_connection_with_proxy()`, but `HttpConnectionPool`
does **not** hold `ClientConfig` (only `pool`/`resolver`/`tls_connector`) and
`HttpClientConnection`'s fields are private. So the socket construction is a new
**pool method** and the routing decision lives at the two places that *do* hold
the config and create connections.

New pool method `backends/foundation_netio/src/http/connection.rs`:

```rust
#[cfg(unix)]
pub fn create_connection_unix(
    &self,
    url: &Uri,
    socket_path: &std::path::Path,
) -> Result<HttpClientConnection, HttpClientError> {
    let host = url.host_str().unwrap_or_else(|| "localhost".to_string());
    let connection = Connection::connect_unix(socket_path)
        .map_err(|e| HttpClientError::ConnectionFailed(e.to_string()))?;
    let stream = SharedByteBufferStream::rwrite(
        RawStream::from_connection(connection)
            .map_err(|e| HttpClientError::ConnectionFailed(e.to_string()))?,
    );
    Ok(HttpClientConnection { stream, host, port: 0 })
}
```

Routing (before the proxy computation) in **both** connection-creating tasks:

- `http/tasks/request_redirect.rs` — the `SendRequestTask` path used by
  `HttpClient::send()` / `send_async()` (this is the path the generated Docker
  client uses).
- `http/tasks/request_stream.rs` — the `HttpExchangeTask` path used by
  `open_exchange()` (streaming / split_exchange).

```rust
#[cfg(unix)]
let unix_socket = config.unix_socket.clone();
#[cfg(not(unix))]
let unix_socket: Option<std::path::PathBuf> = None;

let connection_result = if let Some(socket_path) = unix_socket {
    pool.create_connection_unix(&descriptor.request_uri, &socket_path)
} else {
    // ... existing env-proxy + create_connection_with_proxy ...
};
```

## Why this is enough

The connection is the **only** platform-specific piece. Everything above it —
HTTP rendering, `HttpExchange` splitting, `TransportStream` piping,
`Client<Req,Res>` typed RPC — is byte-level and works unchanged.

Once `HttpClientConnection` holds a Unix-backed stream:
- `NativeHttpClient::open_exchange()` works unchanged
- `H1Transport` works unchanged
- `Client<Req,Res>` works unchanged
- BuildKit (Feature 05) works unchanged

## Scope

| Crate | Change |
|-------|--------|
| `foundation_netio` | Parts A–D: factory, config field, builder, routing |

## Verification

- `cargo check -p foundation_netio --features multi` — compiles on Linux
- `cargo check -p foundation_netio --features wasm-fetch --target wasm32-unknown-unknown` — wasm compiles (unix cfg-gated)
- `HttpClientBuilder::new().unix_socket("/var/run/docker.sock").build()` — compiles
- `DockerClient::connect_with_defaults()` resolves and connects to the Docker socket
- `GET /v1.53/version` over Unix socket returns valid JSON

## Acceptance criteria

1. `HttpClientBuilder::new().unix_socket(path).build()` produces a working `DynNetClient`
2. HTTP requests over Unix socket return correct responses (same semantics as TCP)
3. No DNS resolution occurs when `unix_socket` is set
4. BuildKit (Feature 05) works over the same Unix socket transport
5. wasm32 target compiles (unix cfg-gated)
