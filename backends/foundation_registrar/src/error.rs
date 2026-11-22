//! Why a registrar operation failed.

/// Why the registrar, its discovery, its probes or its registration client
/// failed.
///
/// The variants group the failure paths of spec 82 feature 01, section 6. A
/// caller that needs to tell an operator action (an expired certificate) apart
/// from an automatic retry (an unreachable proxy) matches on the variant.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RegistrarError {
    /// The manifest is not valid.
    #[error("manifest: {0}")]
    Manifest(String),
    /// The manifest version is not one that this build reads.
    #[error("manifest_version {found} is not supported (this build reads {supported})")]
    UnsupportedManifestVersion {
        /// The version in the file.
        found: u32,
        /// The version that this build reads.
        supported: u32,
    },
    /// The configuration is not usable.
    #[error("config: {0}")]
    Config(String),
    /// Enrollment failed.
    #[error("enrollment failed: {0}")]
    Enroll(String),
    /// A registration call failed with a Connect status.
    #[error("registration call failed: {code}: {message}")]
    Call {
        /// The Connect status code, for example `PERMISSION_DENIED`.
        code: String,
        /// The status message.
        message: String,
    },
    /// A newer sidecar with the same host ID replaced this session.
    #[error("session superseded by a newer sidecar with host_id {0}")]
    Superseded(String),
    /// Discovery of one entry failed.
    #[error("discovery of {entry}: {reason}")]
    Discovery {
        /// The manifest entry.
        entry: String,
        /// The reason.
        reason: String,
    },
    /// A metric source failed.
    #[error("metrics: {0}")]
    Metrics(String),
}

impl RegistrarError {
    /// True when the failure needs an operator: a new enrollment token.
    ///
    /// An expired or revoked certificate is the only failure that the sidecar
    /// cannot recover from alone (spec 82 feature 01, section 6.11).
    #[must_use]
    pub fn needs_operator(&self) -> bool {
        matches!(self, Self::Enroll(reason) if reason.contains("token"))
    }
}
