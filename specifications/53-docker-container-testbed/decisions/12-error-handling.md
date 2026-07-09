# 06 — Error Handling

**Date:** 2026-07-08
**Status:** Resolved

## Decision

Use `foundation_errstacks` (the workspace's standard error crate) with
`derive_more` for the `DockerError` enum — no `thiserror`, no `anyhow`.
`derive_more` provides the `Display`, `Error`, and `From` derives; field
doc-comments are the primary documentation channel. Provide
`is_connection_error()` to discriminate "Docker not available" from
operational failures, enabling graceful skip in CI.

## Table of Contents

1. [Error variants](#error-variants)
2. [is_connection_error()](#is_connection_error)
3. [From impls](#from-impls)
4. [Proc macro integration](#proc-macro-integration)
5. [Tracing integration](#tracing-integration)

---

## Error variants

```rust
use derive_more::{Display, Error, From};
use foundation_errstacks::{ErrorTrace, PlainResultExt};

/// Errors from Docker container lifecycle operations.
///
/// Built on `foundation_errstacks` — all public APIs return
/// `Result<T, ErrorTrace<DockerError>>` for full stack context.
/// The `is_connection_error()` method discriminates transient
/// availability failures from operational errors.
#[derive(Debug, Display, Error, From)]
pub enum DockerError {
    /// Docker daemon is not reachable (socket missing, permission denied,
    /// daemon not running, connection timeout).
    #[display("Docker daemon not reachable: {_0}")]
    Connection(String),

    /// Image pull failed (registry unreachable, image not found, auth failure).
    #[display("Image pull failed for '{image}': {reason}")]
    ImagePull { image: String, reason: String },

    /// Container creation failed (invalid config, name conflict, resource limits).
    #[display("Container creation failed: {_0}")]
    ContainerCreate(String),

    /// Container start failed (port conflict, device missing, capability denied).
    #[display("Container start failed: {_0}")]
    ContainerStart(String),

    /// Container exited before wait strategy completed.
    #[display("Container exited with code {code} before becoming ready")]
    ContainerExited { code: i64 },

    /// Wait strategy timed out — container is running but never became ready.
    #[display("Wait strategy '{strategy}' timed out after {elapsed:?}")]
    WaitTimeout { strategy: String, elapsed: Duration },

    /// Network operation failed (create, connect, disconnect, inspect).
    #[display("Network operation failed: {_0}")]
    Network(String),

    /// Invalid configuration (missing required field, invalid port range, etc.).
    #[display("Invalid configuration: {_0}")]
    InvalidConfig(String),

    /// Underlying bollard error — transparently wraps bollard's error type.
    #[display("bollard error: {_0}")]
    #[from]
    Bollard(bollard::errors::Error),
}
```

`derive_more` provides `Display` (via `#[display("...")]`), `Error` (generates
`source()` delegation to inner errors where applicable), and `From` (generates
`From<bollard::errors::Error> for DockerError` from the `#[from]` attribute).
No proc-macro dependency beyond `derive_more` and `foundation_errstacks` —
both already in the workspace.

---

## is_connection_error()

```rust
impl DockerError {
    /// Returns `true` if this error indicates Docker is not available —
    /// the daemon is unreachable, the socket is missing, or the user lacks
    /// permission. Used by the proc macro to implement graceful skip
    /// (test passes, main returns early) when Docker is absent.
    pub fn is_connection_error(&self) -> bool {
        matches!(self, DockerError::Connection(_))
    }
}
```

The `Connection` variant is constructed by `DockerClient::connect_local()` and
`DockerClient::connect_ssh()` when:

- `/var/run/docker.sock` does not exist (`ENOENT`)
- Permission denied (`EACCES`)
- Connection refused (`ECONNREFUSED`)
- `DOCKER_HOST` is set to an unreachable address
- SSH connection fails (host unreachable, key rejected, auth failure)

Operational errors (image not found, port conflict, wait timeout) do NOT
return `true` from `is_connection_error()`. These are real failures that should
fail the test or propagate to the caller.

---

## From impls

The `#[from]` attribute on the `Bollard` variant (via `derive_more::From`)
generates `From<bollard::errors::Error> for DockerError` automatically —
callers use `?` on bollard results and get a `DockerError`.

Additional manual `From` impls for cases `derive_more` can't express:

```rust
impl From<std::io::Error> for DockerError {
    fn from(e: std::io::Error) -> Self {
        match e.kind() {
            std::io::ErrorKind::NotFound
            | std::io::ErrorKind::PermissionDenied
            | std::io::ErrorKind::ConnectionRefused => {
                DockerError::Connection(e.to_string())
            }
            _ => DockerError::Bollard(bollard::errors::Error::IOError(e)),
        }
    }
}
```

---

## Proc macro integration

The `#[docker_container]` macro generates error handling with two branches:

```rust
match ContainerHandle::start(cfg).await {
    Ok(handle) => handle,           // container started successfully
    Err(e) if e.is_connection_error() => {
        tracing::warn!("SKIP: Docker not available ({e})");
        return;                      // test passes, main returns early
    }
    Err(e) => {
        panic!("Failed to start Docker container: {e}");
    }
}
```

The `return` on connection error means:
- For `#[test]` functions: the test function returns with no panics → test
  passes.
- For `main()`: returns early (exit code 0). Future enhancement: return
  non-zero exit code for "required" containers.
- For regular functions: returns the default value (problematic for
  non-`()`-returning fns; requires `required = true` or a default-value
  attribute).

The `required = true` attribute on the macro disables graceful skip:

```rust
#[docker_container(image = "redis:7", port = 6379, required = true)]
fn must_have_docker() { ... }
// → panics on Docker connection error, no skip
```

---

## Tracing integration

All lifecycle stages emit tracing spans and events:

```
INFO docker_container{image="redis:7"}: Pulling image
INFO docker_container{image="redis:7"}: Image pulled (3 layers, 12.4 MB)
INFO docker_container{image="redis:7"}: Container created (id=a1b2c3)
INFO docker_container{image="redis:7"}: Container started
INFO docker_container{image="redis:7"}: Waiting for readiness (Port(6379))
INFO docker_container{image="redis:7"}: Container ready (6379 → host:32768)
--- user body runs ---
INFO docker_container{image="redis:7"}: Stopping container (10s timeout)
INFO docker_container{image="redis:7"}: Container removed
```

If Docker is unavailable:
```
WARN docker_container{image="redis:7"}: SKIP: Docker not available (socket /var/run/docker.sock not found)
```

This enables debugging via `RUST_LOG=foundation_deployment_platform=debug cargo test`.
