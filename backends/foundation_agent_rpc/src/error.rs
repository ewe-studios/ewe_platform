//! `AgentRpcError` — the crate's single error type.
//!
//! WHY: RPC failures must reach clients as a machine-readable status code
//! (Connect code / `grpc-status`), never a bare string, and every layer of the
//! server should speak one error vocabulary.
//!
//! WHAT: [`AgentRpcError`] (one variant per failure class), [`AgentRpcResult`]
//! (`Result<T, ErrorTrace<AgentRpcError>>`, the errstacks convention), and the
//! conversion into [`ConnectError`] used at the RPC boundary.
//!
//! HOW: [`AgentRpcError::code`] is the single mapping table; `From` builds the
//! wire error from it, keeping the variant's `Display` as the message.

use foundation_connectrpc::{Code, ConnectError};
use foundation_errstacks::ErrorTrace;

/// Every way an agent-rpc operation can fail.
#[derive(Debug, Clone, PartialEq, Eq, derive_more::Display, derive_more::Error)]
pub enum AgentRpcError {
    /// A model could not be loaded (missing files, bad weights, backend error).
    #[display("model load failed: {_0}")]
    ModelLoad(#[error(not(source))] String),
    /// A request named a model the server does not host.
    #[display("unknown model: {_0}")]
    ModelNotFound(#[error(not(source))] String),
    /// Missing, invalid, replayed or expired client credentials.
    #[display("authentication failed: {_0}")]
    Auth(#[error(not(source))] String),
    /// No instance with this name exists.
    #[display("instance not found: {_0}")]
    InstanceNotFound(#[error(not(source))] String),
    /// An instance with this name already exists.
    #[display("instance already exists: {_0}")]
    InstanceExists(#[error(not(source))] String),
    /// An instance name failed validation.
    #[display("invalid instance name: {_0}")]
    InvalidName(#[error(not(source))] String),
    /// The instance is already running a turn.
    #[display("instance busy: {_0} is already running a turn")]
    InstanceBusy(#[error(not(source))] String),
    /// The configured instance limit is reached.
    #[display("instance limit reached ({_0} instances)")]
    InstanceLimit(#[error(not(source))] usize),
    /// The agent turn failed.
    #[display("inference failed: {_0}")]
    Inference(#[error(not(source))] String),
    /// A payload could not be encoded or decoded.
    #[display("codec error: {_0}")]
    Codec(#[error(not(source))] String),
    /// The server or a request is misconfigured.
    #[display("invalid configuration: {_0}")]
    Config(#[error(not(source))] String),
    /// Network / TLS transport failure.
    #[display("transport error: {_0}")]
    Transport(#[error(not(source))] String),
    /// The server is shutting down.
    #[display("server shut down")]
    Shutdown,
}

/// Result alias used across the crate.
pub type AgentRpcResult<T> = Result<T, ErrorTrace<AgentRpcError>>;

impl AgentRpcError {
    /// The Connect / gRPC status code this error is reported with.
    #[must_use]
    pub fn code(&self) -> Code {
        match self {
            AgentRpcError::ModelLoad(_) | AgentRpcError::Inference(_) => Code::Internal,
            AgentRpcError::ModelNotFound(_) | AgentRpcError::InstanceNotFound(_) => Code::NotFound,
            AgentRpcError::Auth(_) => Code::Unauthenticated,
            AgentRpcError::InstanceExists(_) => Code::AlreadyExists,
            AgentRpcError::InvalidName(_) | AgentRpcError::Codec(_) => Code::InvalidArgument,
            AgentRpcError::InstanceBusy(_) => Code::Aborted,
            AgentRpcError::InstanceLimit(_) => Code::ResourceExhausted,
            AgentRpcError::Config(_) => Code::FailedPrecondition,
            AgentRpcError::Transport(_) | AgentRpcError::Shutdown => Code::Unavailable,
        }
    }
}

impl From<AgentRpcError> for ConnectError {
    fn from(err: AgentRpcError) -> Self {
        ConnectError::new(err.code(), err.to_string())
    }
}

impl From<AgentRpcError> for ErrorTrace<ConnectError> {
    fn from(err: AgentRpcError) -> Self {
        ErrorTrace::new(ConnectError::from(err))
    }
}
