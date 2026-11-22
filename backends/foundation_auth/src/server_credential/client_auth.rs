//! The result of one authentication and the trait of one provider.

use std::future::Future;
use std::pin::Pin;

use foundation_netio::shared::http::Extensions;

use super::call::CallCredentials;
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

/// Who called. Every provider puts one into [`AuthInfo::artifacts`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Principal {
    /// The subject that the provider established.
    pub subject: String,
    /// How the provider established the subject.
    pub method: AuthMethod,
    /// The roles that the provider assigned.
    pub roles: Vec<String>,
}

/// The method that a provider used to establish a principal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthMethod {
    /// A client certificate with the given SHA-256 fingerprint.
    ClientCertificate {
        /// The SHA-256 fingerprint of the peer certificate, in lowercase hex.
        fingerprint_sha256: String,
    },
    /// A signed secret with the given identifier.
    SignedSecret {
        /// The identifier of the key or secret that signed the credential.
        secret_id: String,
    },
    /// A JSON Web Token (JWT) from the given issuer.
    Jwt {
        /// The issuer claim of the token.
        issuer: String,
    },
    /// A Cloudflare mutual-TLS (mTLS) header with the given fingerprint.
    CloudflareMtls {
        /// The SHA-256 fingerprint that the Worker reported.
        fingerprint_sha256: String,
    },
    /// One shared token. Every caller that shows it is the same principal.
    SharedToken {
        /// The principal name of the shared token.
        name: String,
    },
    /// `InsecureNoAuth` (development only).
    Insecure,
    /// A Durable Object: the Worker already checked the caller.
    Forwarded,
}

/// The future that a provider returns.
pub type AuthFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Option<AuthInfo>, AuthError>> + Send + 'a>>;

/// Checks the credentials of one call.
///
/// A provider is a service. It takes the `Ctx` and the keys that it needs at
/// construction (spec 84), and [`ClientAuth::authenticate`] takes only the
/// per-call facts.
pub trait ClientAuth: Send + Sync + 'static {
    /// The mode for logs and for `GetCapabilities.auth_modes`.
    fn mode(&self) -> &'static str;

    /// Check the credentials of one call.
    ///
    /// - `Ok(Some(info))`: the caller is known.
    /// - `Ok(None)`: the call has no credentials of this kind; a composite
    ///   provider tries the next provider.
    /// - `Err(_)`: the call has credentials of this kind, and they are not
    ///   good. A composite provider stops.
    fn authenticate<'a>(&'a self, call: CallCredentials<'a>) -> AuthFuture<'a>;
}

/// Why authentication did not succeed.
///
/// The message names the failure and never contains the credential, a key or a
/// fingerprint of the token.
#[derive(Debug, Clone, PartialEq, Eq, derive_more::Display, derive_more::Error)]
pub enum AuthError {
    /// The credentials are not valid.
    #[display("invalid credentials: {_0}")]
    Invalid(#[error(not(source))] String),
    /// The credentials are expired.
    #[display("credentials expired")]
    Expired,
    /// The credentials are not valid yet.
    #[display("credentials not valid yet")]
    NotYetValid,
    /// The credentials are revoked.
    #[display("credentials revoked")]
    Revoked,
    /// The credentials are already used.
    #[display("credentials already used")]
    Spent,
    /// The caller is known but not allowed.
    #[display("not allowed: {_0}")]
    Forbidden(#[error(not(source))] String),
    /// The auth store is unavailable.
    #[display("auth store unavailable: {_0}")]
    Unavailable(#[error(not(source))] String),
    /// The auth configuration is wrong.
    #[display("auth misconfigured: {_0}")]
    Misconfigured(#[error(not(source))] String),
}
