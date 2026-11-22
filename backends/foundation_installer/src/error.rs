//! Errors of the installer.
//!
//! One error type serves the parser, the adapters and the run. Every variant
//! carries enough text to find the fault in the manifest or on the host.

use std::path::PathBuf;
use std::time::Duration;

/// One problem in the manifest: its JSON path and a plain message.
///
/// The path uses the JSON Pointer form, for example
/// `/entries/0/source/url`. Spec 78 requires a path in every validation
/// error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldError {
    /// JSON Pointer of the fault.
    pub path: String,
    /// Plain description of the fault.
    pub message: String,
}

impl FieldError {
    /// Make one field error.
    #[must_use]
    pub fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }
}

impl core::fmt::Display for FieldError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{}: {}", self.path, self.message)
    }
}

/// Everything that can stop or fail an installer operation.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum InstallError {
    /// The manifest is not valid JSON or breaks a field rule. Every problem
    /// that the parser found is listed, with the entry name and field.
    #[error("manifest is invalid: {0:?}")]
    Parse(Vec<FieldError>),
    /// The manifest file cannot be read.
    #[error("cannot read the manifest {path}: {reason}")]
    ManifestIo {
        /// The path the installer tried to read.
        path: String,
        /// The reason from the operating system.
        reason: String,
    },
    /// The manifest file is larger than the configured limit.
    #[error("the manifest is {size} bytes, more than the {max} byte limit")]
    ManifestTooLarge {
        /// The size of the file.
        size: u64,
        /// The configured maximum.
        max: u64,
    },
    /// The manifest version is not one that this build reads.
    #[error("manifest_version {found} is not supported (this build reads {supported})")]
    UnsupportedManifestVersion {
        /// The value in the manifest.
        found: u32,
        /// The version that this build reads.
        supported: u32,
    },
    /// A source or deploy `kind` value is not part of the contract.
    #[error("unknown kind {kind:?} at {path}")]
    UnknownKind {
        /// JSON Pointer of the `kind` field.
        path: String,
        /// The value that the reader does not know.
        kind: String,
    },
    /// No source adapter is registered for a known source kind.
    #[error("no source adapter for kind {kind:?} (entry {entry})")]
    UnknownSourceKind {
        /// The entry name.
        entry: String,
        /// The source kind.
        kind: String,
    },
    /// No deploy adapter is registered for a known deploy kind.
    #[error("no deploy adapter for kind {kind:?} (entry {entry})")]
    UnknownDeployKind {
        /// The entry name.
        entry: String,
        /// The deploy kind.
        kind: String,
    },
    /// The deploy target cannot run the artifact that the source gives.
    #[error("deploy kind {deploy} cannot run a {artifact} artifact (entry {entry})")]
    Incompatible {
        /// The entry name.
        entry: String,
        /// The deploy kind.
        deploy: String,
        /// The artifact class: `file`, `tool` or `image`.
        artifact: String,
    },
    /// The source cannot get the software.
    #[error("cannot get the software of {entry}: {reason}")]
    Resolve {
        /// The entry name.
        entry: String,
        /// Why the source failed.
        reason: String,
    },
    /// The bytes do not match the checksum in the manifest.
    #[error("checksum mismatch for {entry}: expected {expected}, got {actual}")]
    ChecksumMismatch {
        /// The entry name.
        entry: String,
        /// The checksum in the manifest.
        expected: String,
        /// The checksum of the bytes that arrived.
        actual: String,
    },
    /// The deploy target cannot make the artifact run.
    #[error("cannot deploy {entry}: {reason}")]
    Apply {
        /// The entry name.
        entry: String,
        /// Why the deploy failed.
        reason: String,
    },
    /// The new version did not start inside the start timeout.
    #[error("{entry} did not start within {timeout:?}")]
    StartTimeout {
        /// The entry name.
        entry: String,
        /// The configured start timeout.
        timeout: Duration,
    },
    /// Another installer run holds the lock file.
    #[error("another installer run holds {0}")]
    Locked(PathBuf),
    /// The install record cannot be read or written.
    #[error("install record I/O: {0}")]
    Record(String),
    /// A side effect that the operation needs is not in the context.
    #[error("{0} is not available in this install context")]
    MissingSideEffect(String),
}

impl InstallError {
    /// The JSON paths of a parse error, in order. An empty list for any other
    /// error.
    #[must_use]
    pub fn field_paths(&self) -> Vec<&str> {
        match self {
            Self::Parse(faults) => faults.iter().map(|fault| fault.path.as_str()).collect(),
            _ => Vec::new(),
        }
    }
}
