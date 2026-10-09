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

    /// Image pull failed (registry unreachable, image not found, auth failure).
    #[display("Image pull failed for '{image}': {reason}")]
    ImagePull { image: String, reason: String },

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

/// Map a `foundation_deployment_docker::DockerError` into our `ErrorTrace<DockerError>`.
///
/// Use with `.map_err(map_docker_client_err)` on `foundation_deployment_docker` calls
/// to convert errors without a manual match at each call site.
#[must_use]
pub fn map_docker_client_err(e: foundation_deployment_docker::DockerError) -> ErrorTrace<DockerError> {
    use foundation_deployment_docker::DockerError as De;
    docker_err(match e {
        De::Transport(m) | De::Unavailable(m) => DockerError::Connection(m),
        De::JsonParse(m) => DockerError::InvalidConfig(m),
        De::BuildFailed(m) => DockerError::ImageBuild(m),
        De::Api { status, message } => {
            DockerError::Connection(format!("docker API {status}: {message}"))
        }
    })
}
