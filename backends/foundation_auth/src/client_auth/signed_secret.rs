//! The signed-secret provider: an `Authorization: Bearer <signed secret>`.
//!
//! WHY: a Worker, a browser or a native client that used Transport Layer
//! Security (TLS) server authentication shows an access secret on each call.
//! The provider checks the signature, the audience, the window, the use limit
//! and revocation, and then gives the handler an `AuthInfo`.
//!
//! WHAT: [`SignedSecretAuth`] and [`AccessPolicy`].
//!
//! HOW: the cheap "is this our credential kind" check comes first, so a
//! composite provider can try a JSON Web Token (JWT) next. Once the token looks
//! like a signed secret, every failure is an error, never `Ok(None)`.

use std::sync::Arc;

use foundation_netio::shared::http::SimpleHeader;

use crate::secret::{SecretError, SecretVerifier, VerifiedSecret};
use crate::server_credential::{
    AuthError, AuthFuture, AuthInfo, AuthMethod, CallCredentials, ClientAuth, Principal,
};
use crate::shared::middleware::AuthContext;
use crate::trust::{TrustError, TrustedClients};

/// The roles of an access secret, and its use limit.
#[derive(Debug, Clone)]
pub struct AccessPolicy {
    /// `None` = no limit from the server; the token `max_uses` still applies.
    pub max_uses: Option<u32>,
    /// The roles that the provider assigns to the caller.
    pub roles: Vec<String>,
}

impl Default for AccessPolicy {
    fn default() -> Self {
        Self {
            max_uses: None,
            roles: vec!["agent-rpc:client".to_string()],
        }
    }
}

impl AccessPolicy {
    /// Set the server-side use limit.
    #[must_use]
    pub fn with_max_uses(mut self, max_uses: u32) -> Self {
        self.max_uses = Some(max_uses);
        self
    }

    /// Set the roles.
    #[must_use]
    pub fn with_roles(mut self, roles: Vec<String>) -> Self {
        self.roles = roles;
        self
    }
}

/// The provider that checks an access signed secret on each call.
pub struct SignedSecretAuth {
    verifier: SecretVerifier,
    trust: Arc<TrustedClients>,
    policy: AccessPolicy,
}

impl std::fmt::Debug for SignedSecretAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SignedSecretAuth")
            .field("verifier", &self.verifier)
            .field("policy", &self.policy)
            .finish_non_exhaustive()
    }
}

impl SignedSecretAuth {
    /// A provider over `verifier` (purpose `Access`, audience = this host) and
    /// the trust store.
    #[must_use]
    pub fn new(verifier: SecretVerifier, trust: Arc<TrustedClients>, policy: AccessPolicy) -> Self {
        Self {
            verifier,
            trust,
            policy,
        }
    }
}

/// The token of an `Authorization: Bearer` header.
fn bearer_token(call: &CallCredentials<'_>) -> Result<Option<String>, AuthError> {
    let Some(header) = call
        .headers
        .get(&SimpleHeader::AUTHORIZATION)
        .and_then(|values| values.first())
    else {
        return Ok(None);
    };
    let Some(token) = crate::server_credential::extract_bearer_token(Some(header.as_str())) else {
        if header
            .as_bytes()
            .get(..7)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(b"Bearer "))
        {
            return Err(AuthError::Invalid("malformed bearer token".to_string()));
        }
        return Ok(None);
    };
    Ok(Some(token))
}

/// Whether a token has the shape of a signed secret.
///
/// A signed secret is base64url of a JSON object, so it has no full stop. A JWT
/// has two full stops that separate its three parts. The check only chooses the
/// provider; the verifier makes the real decision.
fn looks_like_signed_secret(token: &str) -> bool {
    !token.contains('.')
}

fn secret_error_to_auth(error: &SecretError) -> AuthError {
    match error {
        SecretError::Expired => AuthError::Expired,
        SecretError::NotYetValid => AuthError::NotYetValid,
        SecretError::UnsupportedVersion(_)
        | SecretError::UnsupportedAlgorithm(_)
        | SecretError::BadKey(_)
        | SecretError::BadRequest(_)
        | SecretError::Entropy(_) => AuthError::Misconfigured(error.to_string()),
        SecretError::Malformed(_)
        | SecretError::UnknownKey
        | SecretError::BadSignature
        | SecretError::WrongPurpose
        | SecretError::WrongAudience
        | SecretError::WindowTooLong => AuthError::Invalid(error.to_string()),
    }
}

fn trust_error_to_auth(error: &TrustError) -> AuthError {
    match error {
        TrustError::SecretRevoked | TrustError::ClientRevoked => AuthError::Revoked,
        TrustError::Spent => AuthError::Spent,
        TrustError::Storage(message) => AuthError::Unavailable(message.clone()),
        TrustError::Duplicate(message) => AuthError::Misconfigured(message.clone()),
        TrustError::UnknownClient(_) => {
            AuthError::Invalid("the secret names a client that is not enrolled".to_string())
        }
        TrustError::NotAtomic => AuthError::Misconfigured(error.to_string()),
    }
}

/// The limit that applies: the smaller of the server policy and the token.
fn effective_limit(server: Option<u32>, token: Option<u32>) -> Option<u32> {
    match (server, token) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

fn auth_info_for_secret(
    verified: &VerifiedSecret,
    roles: &[String],
    call: &CallCredentials<'_>,
) -> AuthInfo {
    let subject = verified
        .subject
        .clone()
        .unwrap_or_else(|| verified.id.clone());
    let mut context = AuthContext::new(
        call.procedure.to_string(),
        call.connection.peer_addr.map(|address| address.to_string()),
        None,
    );
    context.sub = Some(subject.clone());
    context.roles = roles.to_vec();
    let principal = Principal {
        subject,
        method: AuthMethod::SignedSecret {
            secret_id: verified.id.clone(),
        },
        roles: roles.to_vec(),
    };
    AuthInfo::new(context).with_artifact(principal)
}

impl ClientAuth for SignedSecretAuth {
    fn mode(&self) -> &'static str {
        "signed-secret"
    }

    fn authenticate<'a>(&'a self, call: CallCredentials<'a>) -> AuthFuture<'a> {
        Box::pin(async move {
            let Some(token) = bearer_token(&call)? else {
                return Ok(None);
            };
            if !looks_like_signed_secret(&token) {
                // A JWT: let a composite provider try the next provider.
                return Ok(None);
            }
            let verified = self
                .verifier
                .verify(&token)
                .map_err(|error| secret_error_to_auth(&error))?;
            self.trust
                .check_secret_not_revoked(&verified.id)
                .await
                .map_err(|error| trust_error_to_auth(&error))?;
            // A revocation of the client that an access secret names stops the
            // next call. An access secret from enrollment or refresh has `sub`
            // = the enrolled `client_id`; an operator-issued secret usually has
            // no subject.
            if let Some(client_id) = verified.subject.as_deref() {
                self.trust
                    .check_client_not_revoked(client_id)
                    .await
                    .map_err(|error| trust_error_to_auth(&error))?;
            }
            if let Some(limit) = effective_limit(self.policy.max_uses, verified.max_uses) {
                self.trust
                    .spend_secret(&verified.id, verified.valid_until, limit)
                    .await
                    .map_err(|error| trust_error_to_auth(&error))?;
            }
            Ok(Some(auth_info_for_secret(
                &verified,
                &self.policy.roles,
                &call,
            )))
        })
    }
}
