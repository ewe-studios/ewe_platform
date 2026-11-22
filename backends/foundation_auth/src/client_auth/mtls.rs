//! The native mutual-TLS (mTLS) client-authentication provider.

use std::sync::Arc;

use foundation_context::{Clock, Ctx};

use crate::mtls::peer_identity;
use crate::server_credential::{
    AuthError, AuthFuture, AuthInfo, AuthMethod, CallCredentials, ClientAuth, Principal,
};
use crate::shared::middleware::AuthContext;
use crate::trust::{TrustError, TrustedClient, TrustedClients};

/// What the provider does when the last-seen write fails.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SeenErrorPolicy {
    /// Return the error to the caller. This is the default: a store failure
    /// must reach the caller, never only a log line.
    #[default]
    Fail,
    /// Log the error and accept the call.
    Log,
}

/// The provider that reads the client certificate of a native connection.
///
/// The handshake proves the certificate chain. This provider proves the right
/// to call, on every call: the fingerprint must be in the trust store, the
/// client must not be revoked, and the call must be inside the client's
/// validity window. A revocation therefore stops the next call on a live
/// keep-alive connection.
pub struct MtlsAuth {
    trust: Arc<TrustedClients>,
    clock: Arc<dyn Clock>,
    on_seen_error: SeenErrorPolicy,
}

impl std::fmt::Debug for MtlsAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MtlsAuth")
            .field("on_seen_error", &self.on_seen_error)
            .finish_non_exhaustive()
    }
}

impl MtlsAuth {
    /// A provider over `trust`, with the clock from `ctx`.
    #[must_use]
    pub fn new(trust: Arc<TrustedClients>, ctx: &Ctx) -> Self {
        Self {
            trust,
            clock: ctx.clock(),
            on_seen_error: SeenErrorPolicy::default(),
        }
    }

    /// Choose what a failed last-seen write does.
    #[must_use]
    pub fn with_seen_error_policy(mut self, policy: SeenErrorPolicy) -> Self {
        self.on_seen_error = policy;
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

fn auth_info_for_certificate(
    client: &TrustedClient,
    fingerprint_sha256: String,
    call: &CallCredentials<'_>,
) -> AuthInfo {
    let mut context = AuthContext::new(
        call.procedure.to_string(),
        call.connection.peer_addr.map(|address| address.to_string()),
        None,
    );
    context.sub = Some(client.client_name.clone());
    let principal = Principal {
        subject: client.client_name.clone(),
        method: AuthMethod::ClientCertificate { fingerprint_sha256 },
        roles: Vec::new(),
    };
    AuthInfo::new(context).with_artifact(principal)
}

impl ClientAuth for MtlsAuth {
    fn mode(&self) -> &'static str {
        "mtls"
    }

    fn authenticate<'a>(&'a self, call: CallCredentials<'a>) -> AuthFuture<'a> {
        Box::pin(async move {
            let Some(peer) = peer_identity(call.connection) else {
                return Ok(None);
            };
            let fingerprint = peer.fingerprint_sha256;
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
            let now = self.now();
            if now >= client.expires_at {
                return Err(AuthError::Expired);
            }
            if let Err(error) = self.trust.touch(&fingerprint).await {
                match self.on_seen_error {
                    SeenErrorPolicy::Fail => return Err(trust_error_to_auth(&error)),
                    SeenErrorPolicy::Log => {
                        tracing::warn!(error = %error, "recording last-seen failed");
                    }
                }
            }
            Ok(Some(auth_info_for_certificate(&client, fingerprint, &call)))
        })
    }
}
