//! The provider for a Durable Object (DO) behind a Worker.

use foundation_netio::shared::http::SimpleHeader;

use crate::server_credential::{
    AuthError, AuthFuture, AuthInfo, AuthMethod, CallCredentials, ClientAuth, Principal,
};
use crate::shared::middleware::AuthContext;

/// The header that the Worker writes after it checked the caller.
pub const FORWARDED_PRINCIPAL_HEADER: &str = "EWE-FORWARDED-PRINCIPAL";

/// Reads the principal that the Worker forwarded to a Durable Object.
///
/// The Worker deletes every `ewe-forwarded-*` header on the public request and
/// writes this one only after its own authentication, and the Durable Object is
/// reachable only through the Worker binding. Use it only inside a Durable
/// Object.
pub struct ForwardedPrincipalAuth;

impl std::fmt::Debug for ForwardedPrincipalAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ForwardedPrincipalAuth")
    }
}

impl Default for ForwardedPrincipalAuth {
    fn default() -> Self {
        Self
    }
}

impl ForwardedPrincipalAuth {
    /// The provider.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl ClientAuth for ForwardedPrincipalAuth {
    fn mode(&self) -> &'static str {
        "forwarded-principal"
    }

    fn authenticate<'a>(&'a self, call: CallCredentials<'a>) -> AuthFuture<'a> {
        Box::pin(async move {
            let Some(subject) = call
                .headers
                .get(&SimpleHeader::custom(FORWARDED_PRINCIPAL_HEADER))
                .and_then(|values| values.first())
                .map(|value| value.trim())
                .filter(|value| !value.is_empty())
            else {
                return Ok(None);
            };
            if subject.len() > 1024 {
                return Err(AuthError::Invalid(
                    "the forwarded principal is too long".to_string(),
                ));
            }

            let mut context = AuthContext::new(
                call.procedure.to_string(),
                call.connection.peer_addr.map(|address| address.to_string()),
                None,
            );
            context.sub = Some(subject.to_string());
            let principal = Principal {
                subject: subject.to_string(),
                method: AuthMethod::Forwarded,
                roles: Vec::new(),
            };
            Ok(Some(AuthInfo::new(context).with_artifact(principal)))
        })
    }
}
