//! The Cloudflare client-certificate providers.
//!
//! WHY: Cloudflare ends Transport Layer Security (TLS) at its edge. A Worker
//! never sees the handshake and never sees the chain. It sees the certificate
//! facts that Cloudflare already checked, in `request.cf.tlsClientAuth`. The
//! Worker host copies those facts into the request extensions, and
//! [`CfMtlsAuth`] checks them against the trust store.
//!
//! WHAT: [`CfTlsClientAuth`] (the facts) and [`CfMtlsAuth`] (the provider).
//!
//! HOW: the provider trusts Cloudflare's verdict and then looks up the
//! certificate fingerprint, exactly as the native provider does.

use std::sync::Arc;

use foundation_context::{Clock, Ctx};

use crate::server_credential::{
    AuthError, AuthFuture, AuthInfo, AuthMethod, CallCredentials, ClientAuth, Principal,
};
use crate::shared::middleware::AuthContext;
use crate::trust::{TrustError, TrustedClients};

/// The `SUCCESS` value of [`CfTlsClientAuth::cert_verified`].
pub const CERT_VERIFIED_SUCCESS: &str = "SUCCESS";

/// The client-certificate facts that Cloudflare gives a Worker in
/// `request.cf.tlsClientAuth`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CfTlsClientAuth {
    /// The client showed a certificate.
    pub cert_presented: bool,
    /// `SUCCESS`, `FAILED:<reason>` or `NONE`.
    pub cert_verified: String,
    /// Lowercase hexadecimal SHA-256 of the certificate, the same form as
    /// `sha256_hex`.
    pub cert_fingerprint_sha256: String,
    /// The Distinguished Name (DN) of the certificate subject.
    pub cert_subject_dn: String,
    /// The end of the validity window, as Cloudflare gives it.
    pub cert_not_after: String,
    /// The certificate serial number.
    pub cert_serial: String,
}

impl CfTlsClientAuth {
    /// Whether Cloudflare verified the certificate.
    #[must_use]
    pub fn is_verified(&self) -> bool {
        self.cert_presented && self.cert_verified == CERT_VERIFIED_SUCCESS
    }
}

/// The provider that checks Cloudflare's client-certificate facts on a Worker.
///
/// It returns `Ok(None)` when the request holds no facts or the client showed
/// no certificate. It refuses a certificate that Cloudflare did not verify. By
/// default it also requires the fingerprint to be in the trust store.
pub struct CfMtlsAuth {
    trust: Arc<TrustedClients>,
    require_registered: bool,
    clock: Arc<dyn Clock>,
}

impl std::fmt::Debug for CfMtlsAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CfMtlsAuth")
            .field("require_registered", &self.require_registered)
            .finish_non_exhaustive()
    }
}

impl CfMtlsAuth {
    /// A provider over `trust`, with the clock from `ctx`. The certificate must
    /// also be in the trust store.
    #[must_use]
    pub fn new(trust: Arc<TrustedClients>, ctx: &Ctx) -> Self {
        Self {
            trust,
            require_registered: true,
            clock: ctx.clock(),
        }
    }

    /// Say whether the fingerprint must also be in the trust store. Default
    /// `true`.
    #[must_use]
    pub fn with_require_registered(mut self, require_registered: bool) -> Self {
        self.require_registered = require_registered;
        self
    }

    fn now(&self) -> u64 {
        u64::try_from(foundation_compact::time::unix_seconds(
            self.clock.wall_now(),
        ))
        .unwrap_or(0)
    }
}

fn trust_error_to_auth(error: &TrustError) -> AuthError {
    AuthError::Unavailable(error.to_string())
}

impl ClientAuth for CfMtlsAuth {
    fn mode(&self) -> &'static str {
        "cf-mtls"
    }

    fn authenticate<'a>(&'a self, call: CallCredentials<'a>) -> AuthFuture<'a> {
        Box::pin(async move {
            let Some(facts) = call.extensions.get::<CfTlsClientAuth>() else {
                return Ok(None);
            };
            if !facts.cert_presented {
                return Ok(None);
            }
            if facts.cert_verified != CERT_VERIFIED_SUCCESS {
                return Err(AuthError::Invalid(format!(
                    "Cloudflare did not verify the certificate: {}",
                    facts.cert_verified
                )));
            }

            let fingerprint = facts.cert_fingerprint_sha256.to_ascii_lowercase();
            let subject = if self.require_registered {
                let Some(client) = self
                    .trust
                    .get(&fingerprint)
                    .await
                    .map_err(|error| trust_error_to_auth(&error))?
                else {
                    return Err(AuthError::Invalid(
                        "certificate is not in the trust store".to_string(),
                    ));
                };
                if client.revoked_at.is_some() {
                    return Err(AuthError::Revoked);
                }
                if self.now() >= client.expires_at {
                    return Err(AuthError::Expired);
                }
                client.client_name
            } else {
                fingerprint.clone()
            };

            let mut context = AuthContext::new(
                call.procedure.to_string(),
                call.connection.peer_addr.map(|address| address.to_string()),
                None,
            );
            context.sub = Some(subject.clone());
            let principal = Principal {
                subject,
                method: AuthMethod::CloudflareMtls {
                    fingerprint_sha256: fingerprint,
                },
                roles: Vec::new(),
            };
            Ok(Some(AuthInfo::new(context).with_artifact(principal)))
        })
    }
}
