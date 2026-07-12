# 07 — Unix Socket Transport

**Date:** 2026-07-10
**Updated:** 2026-07-12 (accurate codebase trace)
**Status:** Resolved

## Decision

Unix socket transport for Docker is **nearly there**. The `Connection` enum already
has a `Unix(UnixStream)` variant with full read/write/timeout impls. What's missing
is the factory + config wiring — ~30 lines across two files.

## Current state: what already exists

### 1. `Connection::Unix(UnixStream)` — fully wired

`backends/foundation_netio/src/native/connection.rs:359-362`:

```rust
pub enum Connection {
    Tcp(TcpStream),
    #[cfg(unix)]
    Unix(unix_net::UnixStream),
    Tls(TlsStream),
    #[cfg(unix)]
    Completion(Box<dyn CompletionReadWrite>),
}
```

The `Unix` variant already has full trait impls for `Read`, `Write`,
`ReadTimeoutOperations`, `set_read_timeout`, `raw_fd`, etc. (see lines 420-450).
The stream type exists, it's tested, it works.

### 2. The connection chain — traced end to end

```
H1Transport::open()
  → self.client.open_exchange(prepared)              // h1.rs:95
    → HttpExchangeTask::new(req, pool, config)         // http_client_impl.rs:573
      → GetHttpRequestRedirectTask
        → pool.create_connection_with_proxy(&url, proxy, timeout)  // request_redirect.rs:180
          → self.create_http_connection(url, timeout)  // connection.rs:1096
            → HttpClientConnection::connect_http_host_port(url, resolver, timeout, tls)
              → Connection::with_timeout(addr)          // connection.rs:168
                → Connection::Tcp(TcpStream::connect(addr))
```

### 3. ClientConfig — already carries transport configuration

`backends/foundation_netio/src/shared/client/config.rs:41-48`:

```rust
pub struct ClientConfig {
    pub proxy: Option<proxy::ProxyConfig>,
    pub proxy_from_env: bool,
    // ... timeouts, redirects, headers, limits, retries ...
}
```

`proxy` is already plumbed through `create_connection_with_proxy()` — the exact
same pattern works for Unix sockets.

## What's missing (~30 lines)

### A. `Connection::connect_unix()` factory (netio, ~8 lines)

`backends/foundation_netio/src/native/connection.rs` — add to `impl Connection`:

```rust
#[cfg(unix)]
pub fn connect_unix(path: impl AsRef<Path>) -> Result<Self, Box<dyn Error + Send + Sync + 'static>> {
    let stream = unix_net::UnixStream::connect(path)?;
    Ok(Self::Unix(stream))
}
```

### B. `ClientConfig.unix_socket` field (netio, ~2 lines)

`backends/foundation_netio/src/shared/client/config.rs`:

```rust
pub struct ClientConfig {
    // ... existing fields ...
    /// Connect via Unix socket instead of TCP (Docker daemon, BuildKitd).
    /// When set, DNS resolution is skipped and the HTTP request is sent
    /// over this socket path. The URL host is used for the Host header only.
    #[cfg(unix)]
    pub unix_socket: Option<std::path::PathBuf>,
}
```

### C. `HttpClientBuilder::unix_socket()` builder method (netio, ~5 lines)

`backends/foundation_netio/src/network_client.rs` — add to `impl HttpClientBuilder`:

```rust
#[cfg(unix)]
#[must_use]
pub fn unix_socket(mut self, path: impl Into<std::path::PathBuf>) -> Self {
    self.config.unix_socket = Some(path.into());
    self
}
```

### D. Connection routing (netio, ~15 lines)

`backends/foundation_netio/src/http/connection.rs` — in `create_connection_with_proxy`,
add a check before the proxy/direct-connect logic:

```rust
// Check for Unix socket transport BEFORE DNS resolution.
#[cfg(unix)]
if let Some(socket_path) = &self.config.unix_socket {
    let connection = Connection::connect_unix(socket_path)
        .map_err(|e| HttpClientError::ConnectionFailed(e.to_string()))?;
    let url_host = url.host_str().unwrap_or("localhost");
    let stream = SharedByteBufferStream::rwrite(
        RawStream::from_connection(connection)
            .map_err(|e| HttpClientError::ConnectionFailed(e.to_string()))?,
    );
    return Ok(HttpClientConnection {
        stream,
        host: url_host.to_string(),
        port: 0,  // Unix sockets have no port
    });
}
```

## Why this is enough

The connection is the **only** platform-specific piece. Everything above it —
HTTP request rendering, response parsing, `HttpExchange` splitting,
`TransportStream` piping — is platform-agnostic. Once `HttpClientConnection`
holds a Unix-backed stream:

- **`NativeHttpClient::open_exchange()`** works unchanged — it calls
  `GetHttpRequestRedirectTask` which calls `pool.create_connection_with_proxy()`,
  which now routes to Unix socket when configured.
- **`H1Transport`** works unchanged — it calls `self.client.open_exchange(prepared)`.
- **`Client<Req, Res>`** works unchanged — it calls `transport.open(desc)`.
- **BuildKit** (decision 06) works unchanged — it uses `H1Transport` over
  `DynNetClient` with Unix socket config.

## What the connectrpc stack sees

From the perspective of `foundation_connectrpc`, Unix sockets are transparent.
The `Transport` trait's `open()` returns a `TransportStream` with byte-level
pipe halves — the transport doesn't know or care whether bytes came over TCP,
TLS, or a Unix socket. The protocol reader/writer tasks sit on top of those
pipe halves and process `Frame`s.

```
┌──────────────────────────────────────────┐
│  Client<SolveRequest, SolveResponse>     │  ← typed RPC
│  Transport::open(RequestDescriptor)      │  ← byte-level seam
├──────────────────────────────────────────┤
│  H1Transport                             │
│  self.client.open_exchange(prepared)     │  ← HttpClient trait
├──────────────────────────────────────────┤
│  NativeHttpClient / DynNetClient         │
│  pool.create_connection_with_proxy(…)    │
├──────────────────────────────────────────┤
│  Connection::Unix(UnixStream)  ← NEW     │  ← actual byte transport
│  /var/run/docker.sock                    │
│  /run/buildkit/buildkitd.sock            │
└──────────────────────────────────────────┘
```

## DockerClient with Unix socket

```rust
use foundation_netio::{DynNetClient, HttpClientBuilder};

pub struct DockerClient {
    http: DynNetClient,
}

impl DockerClient {
    /// Connect to the Docker daemon via Unix socket.
    pub fn connect_unix(socket_path: impl Into<PathBuf>) -> Result<Self, DockerError> {
        let http = HttpClientBuilder::new()
            .unix_socket(socket_path)
            .connect_timeout(Duration::from_secs(5))
            .build();

        Ok(Self { http, /* ... */ })
    }

    /// Auto-detect the Docker socket.
    pub fn connect_with_defaults() -> Result<Self, DockerError> {
        let socket = resolve_docker_socket();
        Self::connect_unix(socket)
    }
}
```

Generated API functions work unchanged:
```rust
let response = list_containers(&client.http, &args).await?;
```

## Docker socket locations (unchanged)

| Platform | Path | Notes |
|----------|------|-------|
| Linux (rootful) | `/var/run/docker.sock` | Default |
| Linux (rootless) | `/run/user/$UID/docker.sock` | Rootless Docker |
| Linux (rootless Podman) | `/run/user/$UID/podman/podman.sock` | Rootless Podman |
| macOS (Docker Desktop) | `~/.docker/run/docker.sock` | Docker Desktop for Mac |

## BuildKit socket

BuildKitd listens on `/run/buildkit/buildkitd.sock` by default. The same
`DynNetClient` configured with that socket path connects to BuildKit through
`H1Transport`:

```rust
let http = HttpClientBuilder::new()
    .unix_socket("/run/buildkit/buildkitd.sock")
    .build();

let transport: Arc<dyn Transport> = Arc::new(H1Transport::new(http));
let client: Client<SolveRequest, SolveResponse> = Client::new(
    transport, "http://localhost/moby.buildkit.v1.Control",
    ProcedureCodecs::defaults(),
    ClientOptions::new().with_grpc(),
)?;
```

## Related decisions

- **[04 — HTTP via DynNetClient + PreparedRequestBuilder](04-http-via-simple-http-client.md)** — HTTP client
- **[06 — BuildKit via foundation_connectrpc](06-buildkit-connectrpc.md)** — BuildKit RPC
- **[Feature 01 — DynNetClient + PreparedRequestBuilder alignment](../features/01-dynnetclient-codegen-alignment/feature.md)** — Full alignment
