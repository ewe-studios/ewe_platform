//! The typed error, its code table and its wire form.

use std::fmt;

use foundation_connectrpc::{Code, ConnectError, ErrorDetail};
use foundation_errstacks::ErrorTrace;
use serde::{Deserialize, Serialize};

use crate::shared::driver::HostReport;
use crate::shared::ids::DriverId;
use crate::shared::platform::PlatformKind;

/// The result type of every fallible call in this crate.
pub type SandboxResult<T> = Result<T, ErrorTrace<SandboxError>>;

/// The wire type name of a [`SandboxError`] detail.
pub const SANDBOX_ERROR_TYPE: &str = "ewe.sandbox.v1.SandboxError";

/// The full type URL of a [`SandboxError`] detail.
pub const SANDBOX_ERROR_TYPE_URL: &str = "type.googleapis.com/ewe.sandbox.v1.SandboxError";

/// The error code of the slave.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SlaveErrorCode {
    /// The request is malformed.
    BadRequest,
    /// The named object does not exist.
    NotFound,
    /// The caller may not do this.
    PermissionDenied,
    /// The program did not start.
    SpawnFailed,
    /// The root set-up failed.
    RootSetupFailed,
    /// A mount failed.
    MountFailed,
    /// An input or output call failed.
    Io,
    /// The object already exists.
    AlreadyExists,
    /// The slave did not receive `Prepare`.
    NotPrepared,
    /// An internal failure.
    Internal,
}

impl fmt::Display for SlaveErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::BadRequest => "bad_request",
            Self::NotFound => "not_found",
            Self::PermissionDenied => "permission_denied",
            Self::SpawnFailed => "spawn_failed",
            Self::RootSetupFailed => "root_setup_failed",
            Self::MountFailed => "mount_failed",
            Self::Io => "io",
            Self::AlreadyExists => "already_exists",
            Self::NotPrepared => "not_prepared",
            Self::Internal => "internal",
        };
        formatter.write_str(name)
    }
}

/// Every way a sandbox operation can fail.
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, derive_more::Display, derive_more::Error,
)]
pub enum SandboxError {
    /// No host side-effect bundle is configured.
    #[display("the sandbox host is not configured")]
    HostNotConfigured,
    /// No registered driver can run the spec.
    #[display("no usable driver: {reports:?}")]
    NoDriver {
        /// The report of every driver that the control plane tried.
        reports: Vec<HostReport>,
    },
    /// One driver cannot run here.
    #[display("driver {driver} is unavailable: {reason}")]
    DriverUnavailable {
        /// The driver that cannot run.
        driver: DriverId,
        /// Why the driver cannot run.
        reason: String,
    },
    /// A program file is missing.
    #[display("program not found: {name}")]
    BinaryNotFound {
        /// The program name.
        name: String,
    },
    /// A library file is missing.
    #[display("library not found: {name}")]
    LibraryNotFound {
        /// The library name.
        name: String,
    },
    /// A library is older than the platform needs.
    #[display("library is too old: it has no symbol {symbol}")]
    LibraryTooOld {
        /// The missing symbol.
        symbol: String,
    },
    /// A required program or library is not signed.
    #[display("the program or library is not signed")]
    NotSigned,
    /// A spec field is wrong.
    #[display("invalid spec field {field}: {reason}")]
    InvalidSpec {
        /// The field name.
        field: String,
        /// Why the field is wrong.
        reason: String,
    },
    /// The host cannot use this root file system kind.
    #[display("the host does not support this root file system")]
    UnsupportedRootFs,
    /// The host cannot use this network mode.
    #[display("the host does not support this network mode")]
    UnsupportedNetwork,
    /// The host cannot mount a host directory.
    #[display("the host does not support host mounts")]
    UnsupportedMount,
    /// The runtime cannot do this operation.
    #[display("unsupported capability: {capability}")]
    UnsupportedCapability {
        /// The capability name.
        capability: String,
    },
    /// A Unix domain socket path is too long.
    #[display("socket path is longer than {max} bytes: {path}")]
    SocketPathTooLong {
        /// The path.
        path: String,
        /// The largest accepted length in bytes.
        max: usize,
    },
    /// The substrate stopped during a start.
    #[display("the substrate exited during the start (code {code:?})")]
    SubstrateExited {
        /// The exit code, when the substrate exited with one.
        code: Option<i32>,
        /// The last bytes of the substrate log.
        log_tail: Vec<u8>,
    },
    /// The substrate API returned an error.
    #[display("the substrate API returned status {status}: {message}")]
    SubstrateApi {
        /// The HTTP status.
        status: u16,
        /// The API message.
        message: String,
    },
    /// The substrate did not reach the deadline.
    #[display("boot timeout at stage {stage}")]
    BootTimeout {
        /// The start stage that timed out.
        stage: String,
        /// The last bytes of the substrate log.
        log_tail: Vec<u8>,
    },
    /// The slave handshake failed.
    #[display("the slave handshake failed")]
    SlaveHandshake,
    /// The named object does not exist.
    #[display("not found: {what}")]
    NotFound {
        /// The object name.
        what: String,
    },
    /// The sandbox is not ready.
    #[display("the sandbox is stopped")]
    SandboxStopped,
    /// A command passed its time limit.
    #[display("the command passed its time limit")]
    ExecTimeout,
    /// The slave returned an error.
    #[display("slave error {code}: {message}")]
    SlaveError {
        /// The slave error code.
        code: SlaveErrorCode,
        /// The operating system error number.
        errno: Option<i32>,
        /// The slave message.
        message: String,
    },
    /// A sequence number is not the next one.
    #[display("the sequence number is not the next one")]
    SequenceGap,
    /// The input queue of a command is full.
    #[display("the input queue is full")]
    InputQueueFull,
    /// A response stream ended before its last event.
    #[display("the response stream was lost")]
    StreamLost,
    /// The slave has too many open sessions.
    #[display("the slave has too many open sessions")]
    TooManySessions,
    /// An artifact hash does not match.
    #[display("artifact hash mismatch: expected {expected}, got {actual}")]
    ArtifactHashMismatch {
        /// The expected hash.
        expected: String,
        /// The actual hash.
        actual: String,
    },
    /// An artifact path is not safe.
    #[display("unsafe artifact path: {path}")]
    ArtifactUnsafePath {
        /// The path.
        path: String,
    },
    /// An artifact is too large.
    #[display("artifact is larger than the limit {limit}: {message}")]
    ArtifactTooLarge {
        /// The limit in bytes.
        limit: u64,
        /// The measured size or the reason.
        message: String,
    },
    /// An image architecture does not match the host.
    #[display("image {image} does not match host {host}")]
    ImageArchMismatch {
        /// The image.
        image: String,
        /// The host.
        host: String,
    },
    /// One build step failed.
    #[display("build step {line} failed with exit {exit:?} and signal {signal:?}")]
    BuildStepFailed {
        /// The build file line.
        line: u32,
        /// The exit code.
        exit: Option<i32>,
        /// The signal.
        signal: Option<i32>,
    },
    /// A remote peer returned an error.
    #[display("remote error {code}: {message}")]
    Rpc {
        /// The Connect code name.
        code: String,
        /// The remote message.
        message: String,
    },
    /// Credentials are missing or wrong.
    #[display("unauthorized")]
    Unauthorized,
    /// A host input or output call failed.
    #[display("I/O error in {op}: {message}")]
    Io {
        /// The operation.
        op: String,
        /// The error message.
        message: String,
    },
    /// The platform wrapper is not the expected part.
    #[display("platform mismatch: expected {expected}, got {actual}")]
    PlatformMismatch {
        /// The expected part.
        expected: PlatformKind,
        /// The actual part.
        actual: PlatformKind,
    },
}

impl SandboxError {
    /// The Connect code of this error.
    #[must_use]
    pub fn code(&self) -> Code {
        match self {
            Self::InvalidSpec { .. }
            | Self::UnsupportedRootFs
            | Self::UnsupportedNetwork
            | Self::UnsupportedMount
            | Self::SocketPathTooLong { .. } => Code::InvalidArgument,
            Self::NotFound { .. }
            | Self::SlaveError {
                code: SlaveErrorCode::NotFound,
                ..
            } => Code::NotFound,
            Self::SlaveError {
                code: SlaveErrorCode::PermissionDenied,
                ..
            } => Code::PermissionDenied,
            Self::SandboxStopped
            | Self::HostNotConfigured
            | Self::NoDriver { .. }
            | Self::DriverUnavailable { .. }
            | Self::BinaryNotFound { .. }
            | Self::LibraryNotFound { .. }
            | Self::LibraryTooOld { .. }
            | Self::NotSigned
            | Self::UnsupportedCapability { .. }
            | Self::PlatformMismatch { .. } => Code::FailedPrecondition,
            Self::BootTimeout { .. } | Self::ExecTimeout => Code::DeadlineExceeded,
            Self::TooManySessions | Self::InputQueueFull => Code::ResourceExhausted,
            Self::SequenceGap => Code::OutOfRange,
            Self::StreamLost => Code::Aborted,
            Self::Unauthorized => Code::Unauthenticated,
            Self::SubstrateExited { .. }
            | Self::SubstrateApi { .. }
            | Self::SlaveHandshake
            | Self::SlaveError { .. }
            | Self::ArtifactHashMismatch { .. }
            | Self::ArtifactUnsafePath { .. }
            | Self::ArtifactTooLarge { .. }
            | Self::ImageArchMismatch { .. }
            | Self::BuildStepFailed { .. }
            | Self::Io { .. } => Code::Internal,
            Self::Rpc { code, .. } => Code::from_str(code).unwrap_or(Code::Unknown),
        }
    }

    /// The variant name, as it appears on the wire.
    #[must_use]
    pub fn variant_name(&self) -> &'static str {
        match self {
            Self::HostNotConfigured => "HostNotConfigured",
            Self::NoDriver { .. } => "NoDriver",
            Self::DriverUnavailable { .. } => "DriverUnavailable",
            Self::BinaryNotFound { .. } => "BinaryNotFound",
            Self::LibraryNotFound { .. } => "LibraryNotFound",
            Self::LibraryTooOld { .. } => "LibraryTooOld",
            Self::NotSigned => "NotSigned",
            Self::InvalidSpec { .. } => "InvalidSpec",
            Self::UnsupportedRootFs => "UnsupportedRootFs",
            Self::UnsupportedNetwork => "UnsupportedNetwork",
            Self::UnsupportedMount => "UnsupportedMount",
            Self::UnsupportedCapability { .. } => "UnsupportedCapability",
            Self::SocketPathTooLong { .. } => "SocketPathTooLong",
            Self::SubstrateExited { .. } => "SubstrateExited",
            Self::SubstrateApi { .. } => "SubstrateApi",
            Self::BootTimeout { .. } => "BootTimeout",
            Self::SlaveHandshake => "SlaveHandshake",
            Self::NotFound { .. } => "NotFound",
            Self::SandboxStopped => "SandboxStopped",
            Self::ExecTimeout => "ExecTimeout",
            Self::SlaveError { .. } => "SlaveError",
            Self::SequenceGap => "SequenceGap",
            Self::InputQueueFull => "InputQueueFull",
            Self::StreamLost => "StreamLost",
            Self::TooManySessions => "TooManySessions",
            Self::ArtifactHashMismatch { .. } => "ArtifactHashMismatch",
            Self::ArtifactUnsafePath { .. } => "ArtifactUnsafePath",
            Self::ArtifactTooLarge { .. } => "ArtifactTooLarge",
            Self::ImageArchMismatch { .. } => "ImageArchMismatch",
            Self::BuildStepFailed { .. } => "BuildStepFailed",
            Self::Rpc { .. } => "Rpc",
            Self::Unauthorized => "Unauthorized",
            Self::Io { .. } => "Io",
            Self::PlatformMismatch { .. } => "PlatformMismatch",
        }
    }

    /// Converts this error to a Connect error with one wire detail.
    ///
    /// Every text field and every log tail is cut to `max_message` bytes.
    #[must_use]
    pub fn to_connect(&self, max_message: usize) -> ConnectError {
        let bounded = self.truncated(max_message);
        let value = serde_json::to_vec(&bounded.to_wire()).unwrap_or_else(|_| b"{}".to_vec());
        ConnectError::new(self.code(), cut(&self.to_string(), max_message))
            .with_detail(ErrorDetail::from_raw(SANDBOX_ERROR_TYPE_URL, value))
    }

    /// Converts a Connect error back to a sandbox error.
    ///
    /// A detail of this crate gives its original variant and fields. Every
    /// other error becomes [`SandboxError::Rpc`].
    #[must_use]
    pub fn from_connect(error: &ConnectError) -> Self {
        for detail in error.details() {
            if detail.type_name() != SANDBOX_ERROR_TYPE {
                continue;
            }
            let Ok(wire) = serde_json::from_slice::<SandboxErrorWire>(detail.value()) else {
                continue;
            };
            if let Some(error) = wire.into_error() {
                return error;
            }
        }
        Self::Rpc {
            code: error.code().as_str().to_string(),
            message: error.message().to_string(),
        }
    }

    /// The wire detail of this error, with unit variants as a null field set.
    fn to_wire(&self) -> SandboxErrorWire {
        let value = serde_json::to_value(self).unwrap_or(serde_json::Value::Null);
        let fields = match &value {
            serde_json::Value::Object(map) => map
                .values()
                .next()
                .cloned()
                .unwrap_or(serde_json::Value::Null),
            _ => serde_json::Value::Null,
        };
        SandboxErrorWire {
            variant: self.variant_name().to_string(),
            fields,
        }
    }

    /// A clone with every text field and log tail cut to `max` bytes.
    fn truncated(&self, max: usize) -> Self {
        match self.clone() {
            Self::NoDriver { reports } => Self::NoDriver {
                reports: reports.iter().map(|report| report.truncated(max)).collect(),
            },
            Self::DriverUnavailable { driver, reason } => Self::DriverUnavailable {
                driver,
                reason: cut(&reason, max),
            },
            Self::BinaryNotFound { name } => Self::BinaryNotFound {
                name: cut(&name, max),
            },
            Self::LibraryNotFound { name } => Self::LibraryNotFound {
                name: cut(&name, max),
            },
            Self::LibraryTooOld { symbol } => Self::LibraryTooOld {
                symbol: cut(&symbol, max),
            },
            Self::InvalidSpec { field, reason } => Self::InvalidSpec {
                field: cut(&field, max),
                reason: cut(&reason, max),
            },
            Self::UnsupportedCapability { capability } => Self::UnsupportedCapability {
                capability: cut(&capability, max),
            },
            Self::SocketPathTooLong { path, max: limit } => Self::SocketPathTooLong {
                path: cut(&path, max),
                max: limit,
            },
            Self::SubstrateExited { code, log_tail } => Self::SubstrateExited {
                code,
                log_tail: cut_bytes(log_tail, max),
            },
            Self::SubstrateApi { status, message } => Self::SubstrateApi {
                status,
                message: cut(&message, max),
            },
            Self::BootTimeout { stage, log_tail } => Self::BootTimeout {
                stage: cut(&stage, max),
                log_tail: cut_bytes(log_tail, max),
            },
            Self::NotFound { what } => Self::NotFound {
                what: cut(&what, max),
            },
            Self::SlaveError {
                code,
                errno,
                message,
            } => Self::SlaveError {
                code,
                errno,
                message: cut(&message, max),
            },
            Self::ArtifactHashMismatch { expected, actual } => Self::ArtifactHashMismatch {
                expected: cut(&expected, max),
                actual: cut(&actual, max),
            },
            Self::ArtifactUnsafePath { path } => Self::ArtifactUnsafePath {
                path: cut(&path, max),
            },
            Self::ArtifactTooLarge { limit, message } => Self::ArtifactTooLarge {
                limit,
                message: cut(&message, max),
            },
            Self::ImageArchMismatch { image, host } => Self::ImageArchMismatch {
                image: cut(&image, max),
                host: cut(&host, max),
            },
            Self::Rpc { code, message } => Self::Rpc {
                code,
                message: cut(&message, max),
            },
            Self::Io { op, message } => Self::Io {
                op: cut(&op, max),
                message: cut(&message, max),
            },
            other => other,
        }
    }
}

/// The JSON body of the Connect error detail.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SandboxErrorWire {
    /// The variant name.
    pub variant: String,
    /// The fields of the variant, or null for a unit variant.
    pub fields: serde_json::Value,
}

impl SandboxErrorWire {
    /// Rebuilds the error, or `None` when the variant is unknown.
    fn into_error(self) -> Option<SandboxError> {
        let value = if self.fields.is_null() {
            serde_json::Value::String(self.variant)
        } else {
            let mut map = serde_json::Map::new();
            map.insert(self.variant, self.fields);
            serde_json::Value::Object(map)
        };
        serde_json::from_value(value).ok()
    }
}

impl From<SandboxError> for ErrorTrace<SandboxError> {
    fn from(error: SandboxError) -> Self {
        ErrorTrace::new(error)
    }
}

impl From<SandboxError> for ErrorTrace<ConnectError> {
    fn from(error: SandboxError) -> Self {
        ErrorTrace::new(error.to_connect(64 * 1024))
    }
}

/// Cuts text to at most `max` bytes on a character boundary.
pub(crate) fn cut(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_string();
    }
    let mut end = max;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_string()
}

/// Cuts bytes to at most `max` bytes.
fn cut_bytes(mut bytes: Vec<u8>, max: usize) -> Vec<u8> {
    bytes.truncate(max);
    bytes
}
