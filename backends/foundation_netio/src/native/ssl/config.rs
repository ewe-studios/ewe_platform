/// Configuration for TLS verification.
///
/// This struct contains the necessary parameters for verifying and fixing TLS backends.
pub struct TlsVerificationConfig {
    /// Whether to verify certificates
    pub verify_certificates: bool,
    /// Whether to use default root certificates
    pub use_default_roots: bool,
    /// Custom root certificates (if not using default)
    pub custom_roots: Option<Vec<u8>>,
    /// Timeout for TLS connections
    pub timeout: Option<std::time::Duration>,
}
