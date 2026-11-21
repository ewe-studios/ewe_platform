//! The result of one authentication.

use foundation_netio::shared::http::Extensions;

use crate::shared::middleware::AuthContext;

/// The result of one successful authentication.
#[derive(Clone, Debug)]
pub struct AuthInfo {
    /// The principal: subject, scopes, roles.
    pub context: AuthContext,
    /// The artifacts that the provider proved: verified claims, a peer
    /// certificate, a session.
    pub artifacts: Extensions,
}

impl AuthInfo {
    /// A result with the given context and no artifacts.
    #[must_use]
    pub fn new(context: AuthContext) -> Self {
        Self {
            context,
            artifacts: Extensions::new(),
        }
    }

    /// Add a typed artifact to the result.
    #[must_use]
    pub fn with_artifact<T: Send + Sync + 'static>(mut self, artifact: T) -> Self {
        self.artifacts.insert(artifact);
        self
    }
}
