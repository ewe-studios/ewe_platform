//! Module for no ssl stand in to ensure compilation works in such cases

/// A TLS connector in a build without a TLS provider.
///
/// The type keeps the HTTP client API the same on every build. A request that
/// needs a secure connection fails at the connector, with `Unsupported`.
#[derive(Clone, Debug, Default)]
pub struct SSLConnector;

impl SSLConnector {
    /// A connector with no provider.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}
