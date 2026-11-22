//! One shared token provider.

use foundation_netio::shared::http::SimpleHeader;

use super::bearer::is_bearer_scheme;
use super::call::CallCredentials;
use super::client_auth::{AuthError, AuthFuture, AuthInfo, AuthMethod, ClientAuth, Principal};
use super::secret::Secret;
use crate::shared::middleware::AuthContext;

/// The principal name when the caller gives no name.
const DEFAULT_NAME: &str = "shared-token";

/// One shared token.
///
/// Every caller that shows the token is the same principal. The provider
/// returns `Ok(Some(..))` on a match, `Ok(None)` when the call has no Bearer
/// credential, and `Err(AuthError::Invalid)` when the token is wrong.
///
/// Use it for a loopback test and for a closed control plane. It is not for a
/// production service on an open network: one token is one principal, and
/// revocation means a restart.
pub struct StaticToken {
    token: Secret,
    name: String,
}

impl StaticToken {
    /// A provider for `token`. The principal name is `shared-token`.
    #[must_use]
    pub fn new(token: Secret) -> Self {
        Self {
            token,
            name: DEFAULT_NAME.to_string(),
        }
    }

    /// A provider with a principal name.
    #[must_use]
    pub fn named(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }
}

impl ClientAuth for StaticToken {
    fn mode(&self) -> &'static str {
        "shared-token"
    }

    fn authenticate<'a>(&'a self, call: CallCredentials<'a>) -> AuthFuture<'a> {
        Box::pin(async move {
            let Some(header) = call
                .headers
                .get(&SimpleHeader::AUTHORIZATION)
                .and_then(|values| values.first())
            else {
                return Ok(None);
            };

            let Some(presented) = Secret::from_bearer(Some(header.as_str())) else {
                // A different scheme is not a credential of this kind. A
                // Bearer scheme that carries no valid token is a bad
                // credential of this kind.
                if is_bearer_scheme(header) {
                    return Err(AuthError::Invalid("malformed bearer token".to_string()));
                }
                return Ok(None);
            };

            if !presented.ct_eq(&self.token) {
                return Err(AuthError::Invalid("invalid bearer token".to_string()));
            }

            let mut context = AuthContext::new(call.procedure.to_string(), None, None);
            context.sub = Some(self.name.clone());
            let principal = Principal {
                subject: self.name.clone(),
                method: AuthMethod::SharedToken {
                    name: self.name.clone(),
                },
                roles: Vec::new(),
            };
            Ok(Some(AuthInfo::new(context).with_artifact(principal)))
        })
    }
}
