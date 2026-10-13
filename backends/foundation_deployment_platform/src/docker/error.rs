//! Docker operation errors.
//!
//! **WHY:** Docker lifecycle operations fail in distinct ways that callers
//! need to discriminate: connection failures (Docker not installed/running),
//! image pull failures, container creation/start errors, wait timeouts.
//! `is_connection_error()` enables graceful skip when Docker is absent.
//!
//! **WHAT:** A typed `DockerError` enum using `derive_more::Display`,
//! wrapped in `foundation_errstacks::ErrorTrace`. Type alias `DockerResult<T>`.
//!
//! **HOW:** Follows the workspace convention: `derive_more` for `Display`,
//! no `thiserror`/`anyhow`. `ErrorTrace` provides the `Error` impl. Public
//! APIs return `DockerResult<T>`.

use derive_more::Display;
use foundation_errstacks::ErrorTrace;

/// Type alias for all Docker operations.
pub type DockerResult<T> = Result<T, ErrorTrace<DockerError>>;

/// Errors from Docker container lifecycle operations.
///
/// `is_connection_error()` discriminates transient availability
/// failures from operational errors.
#[derive(Debug, Display)]
pub enum DockerError {
    /// Docker daemon is not reachable (socket missing, permission denied,
    /// daemon not running, connection timeout).
    #[display("Docker daemon not reachable: {_0}")]
    Connection(String),

    /// The daemon was reached and answered a request with an error status
    /// (`4xx`/`5xx`). The daemon *is* available, so this is never a
    /// connection error: a test must fail on it, not skip.
    #[display("Docker API error during {operation} (status {status}): {message}")]
    Api {
        /// What was being done, e.g. `image_inspect`.
        operation: String,
        /// The HTTP status the daemon answered with.
        status: u16,
        /// The daemon's error message.
        message: String,
    },

    /// The daemon was reached but shares no API version with this client.
    #[display("Docker daemon is incompatible with this client: {_0}")]
    IncompatibleDaemon(String),

    /// Image pull failed (registry unreachable, image not found, auth failure).
    #[display("Image pull failed for '{image}': {reason}")]
    ImagePull { image: String, reason: String },

    /// Image push failed (registry refused it, authentication failed).
    #[display("Image push failed: {_0}")]
    ImagePush(String),

    /// Image load failed (the archive was not a valid image tarball), reported
    /// inside the daemon's `200` progress stream.
    #[display("Image load failed: {_0}")]
    ImageLoad(String),

    /// Image build failed (a Dockerfile instruction returned non-zero, a COPY
    /// source was missing, the base image could not be resolved).
    #[display("Image build failed: {_0}")]
    ImageBuild(String),

    /// Container creation failed (invalid config, name conflict, resource limits).
    #[display("Container creation failed: {_0}")]
    ContainerCreate(String),

    /// Container start failed (port conflict, device missing, capability denied).
    #[display("Container start failed: {_0}")]
    ContainerStart(String),

    /// Container exited before wait strategy completed.
    #[display("Container exited with code {code} before becoming ready")]
    ContainerExited { code: i64 },

    /// Wait strategy timed out.
    #[display("Wait strategy '{strategy}' timed out after {elapsed:?}")]
    WaitTimeout {
        strategy: String,
        elapsed: std::time::Duration,
    },

    /// Network operation failed.
    #[display("Network operation failed: {_0}")]
    Network(String),

    /// Invalid configuration (missing required field, invalid port range, etc.).
    #[display("Invalid configuration: {_0}")]
    InvalidConfig(String),
}

impl std::error::Error for DockerError {}

impl DockerError {
    /// Returns `true` if this error indicates Docker is not available.
    #[must_use]
    pub fn is_connection_error(&self) -> bool {
        matches!(self, Self::Connection(_))
    }
}

/// Convenience: wrap a `DockerError` in an `ErrorTrace`.
#[must_use]
pub fn docker_err(e: DockerError) -> ErrorTrace<DockerError> {
    ErrorTrace::new(e)
}

/// Classify a `foundation_deployment_docker::DockerError` raised while doing
/// `operation` (e.g. `"image_inspect"`).
///
/// Only a daemon that could not be talked to at all becomes
/// [`DockerError::Connection`]. A daemon that answered — with an error status
/// or an API-version mismatch — was reachable, so those stay distinct: callers
/// that skip when Docker is absent must not skip on them.
#[must_use]
pub fn classify_docker_client_err(
    operation: &str,
    e: foundation_deployment_docker::DockerError,
) -> DockerError {
    use foundation_deployment_docker::DockerError as De;
    match e {
        De::Transport(m) | De::Unavailable(m) => DockerError::Connection(format!("{operation}: {m}")),
        De::Api { status, message } => DockerError::Api {
            operation: operation.to_string(),
            status,
            message,
        },
        e @ De::UnsupportedApiVersion { .. } => {
            DockerError::IncompatibleDaemon(format!("{operation}: {e}"))
        }
        De::JsonParse(m) => DockerError::InvalidConfig(format!("{operation}: {m}")),
        De::BuildFailed(m) => DockerError::ImageBuild(m),
        // The image name is not part of the client's error; the caller (which
        // knows it) builds `ImagePull` itself where it matters.
        De::PullFailed(m) => DockerError::ImagePull { image: operation.to_string(), reason: m },
        De::PushFailed(m) => DockerError::ImagePush(format!("{operation}: {m}")),
        De::LoadFailed(m) => DockerError::ImageLoad(format!("{operation}: {m}")),
    }
}

/// Map a `foundation_deployment_docker::DockerError` into our `ErrorTrace<DockerError>`.
///
/// Use with `.map_err(map_docker_client_err)` on `foundation_deployment_docker` calls
/// to convert errors without a manual match at each call site. See
/// [`classify_docker_client_err`] for how each kind maps; prefer that (with an
/// operation name) where the operation is known.
#[must_use]
pub fn map_docker_client_err(e: foundation_deployment_docker::DockerError) -> ErrorTrace<DockerError> {
    docker_err(classify_docker_client_err("docker request", e))
}
