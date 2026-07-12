# 07 — Unix Socket Transport

**Date:** 2026-07-10
**Updated:** 2026-07-12 (F51 alignment — Feature 00)
**Status:** Resolved

## Decision

Implement Unix socket transport for `DynNetClient` as the primary Docker connection method.
The Docker daemon listens on `/var/run/docker.sock` by default on Linux.

## Why Unix sockets

1. **Default Docker transport** — `docker` CLI uses Unix socket by default
2. **No TLS needed** — Unix sockets are local-only, no encryption overhead
3. **Permission-based auth** — Socket file permissions (root or `docker` group) control access
4. **No network exposure** — Docker API never exposed on TCP

## How DynNetClient connects

`DynNetClient` is built via `HttpClientBuilder`. Unix socket support is configured through
the builder — either via a proxy config or a connector override:

### URL scheme-based transport selection

Detect `unix://` scheme in the base URL and route to `UnixStream::connect()` instead of
`TcpStream::connect()`:

```rust
enum Transport {
    Tcp,
    Unix(PathBuf),
    #[cfg(feature = "tls")] Tls(TlsConfig),
}

fn resolve_transport(base_url: &str) -> Transport {
    if let Some(path) = base_url.strip_prefix("unix://") {
        Transport::Unix(PathBuf::from(path))
    } else {
        Transport::Tcp
    }
}
```

When building a request:
```rust
fn build_request(&self, method: &str, path: &str) -> PreparedRequestBuilder {
    let url = match &self.transport {
        Transport::Unix(_) => {
            // Use fake host "localhost" for HTTP/1.1 Host header requirement
            // Unix socket handles actual connection
            format!("http://localhost{}", path)
        }
        Transport::Tcp => {
            format!("http://{}{}", self.host, path)
        }
    };
    PreparedRequestBuilder::get(&url).unwrap()
}
```

## Docker socket locations

| Platform | Path | Notes |
|----------|------|-------|
| Linux (rootful) | `/var/run/docker.sock` | Default, requires root or `docker` group |
| Linux (rootless) | `/run/user/$UID/docker.sock` | Rootless Docker |
| Linux (rootless Podman) | `/run/user/$UID/podman/podman.sock` | Rootless Podman |
| macOS (Docker Desktop) | `~/.docker/run/docker.sock` | Docker Desktop for Mac |
| WSL2 | `/var/run/docker.sock` | Same as Linux |

### Connection resolution order

```rust
fn resolve_docker_socket() -> PathBuf {
    // 1. DOCKER_HOST env var (unix:///path)
    if let Ok(host) = env::var("DOCKER_HOST") {
        if let Some(path) = host.strip_prefix("unix://") {
            return PathBuf::from(path);
        }
    }
    // 2. Rootless Docker socket
    if let Ok(uid) = env::var("UID") {
        let path = format!("/run/user/{}/docker.sock", uid);
        if Path::new(&path).exists() {
            return PathBuf::from(path);
        }
    }
    // 3. Default system socket
    PathBuf::from("/var/run/docker.sock")
}
```

## HTTP semantics over Unix sockets

HTTP over Unix sockets works identically to TCP:
- Same HTTP/1.1 request format
- Same `Host` header (use `localhost` or `docker` as the host)
- Same response parsing
- Only the transport layer differs (UnixStream vs TcpStream)

## Connection pooling

Unix socket connections can be pooled like TCP:
- One persistent connection per socket path
- Keep-alive works the same
- No DNS resolution needed

## Podman compatibility

Podman implements a Docker-compatible API over Unix sockets. The same HTTP endpoints work
(`/containers/create`, `/images/list`, etc.) with minor differences. Our Unix socket transport
automatically works with Podman — no special code needed.

## Related decisions

- **[04 — HTTP via DynNetClient + PreparedRequestBuilder](04-http-via-simple-http-client.md)** — HTTP client pattern
- **[Feature 00 — DynNetClient + PreparedRequestBuilder alignment](../features/00-dynnetclient-codegen-alignment/feature.md)** — Full design
