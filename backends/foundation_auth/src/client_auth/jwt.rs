//! The JSON Web Token (JWT) provider.
//!
//! WHY: an operator asked for JWT authentication for clients that already hold
//! a token from an identity provider (owner decision 4). It is not the default:
//! the signed secret is.
//!
//! WHAT: [`JwtAuth`], over the crate's [`JwtVerifier`].
//!
//! HOW: the verifier reads the time from the injected `Ctx`, so an expiry check
//! follows simulated time in a deterministic simulation test (DST).

use std::sync::Arc;

use foundation_context::{Clock, Ctx};
use foundation_netio::shared::http::SimpleHeader;

use crate::server_credential::{
    AuthError, AuthFuture, AuthInfo, AuthMethod, CallCredentials, ClientAuth, Principal,
};
use crate::shared::jwt::{JwtError, JwtVerifier};
use crate::shared::middleware::AuthContext;

/// The provider that checks a JWT bearer token.
pub struct JwtAuth {
    verifier: JwtVerifier,
    clock: Arc<dyn Clock>,
}

impl std::fmt::Debug for JwtAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JwtAuth").finish_non_exhaustive()
    }
}

impl JwtAuth {
    /// A provider over `verifier`, with the clock from `ctx`.
    #[must_use]
    pub fn new(verifier: JwtVerifier, ctx: &Ctx) -> Self {
        Self {
            verifier,
            clock: ctx.clock(),
        }
    }

    fn now_unix(&self) -> u64 {
        u64::try_from(foundation_compact::time::unix_seconds(
            self.clock.wall_now(),
        ))
        .unwrap_or(0)
    }
}

/// Whether a token has the shape of a JWT: three dot-separated parts.
fn looks_like_jwt(token: &str) -> bool {
    token.split('.').count() == 3
}

fn jwt_error_to_auth(error: &JwtError) -> AuthError {
    // jwt-simple reports an expired token inside its verification error. The
    // message names no secret, so it is safe to return.
    AuthError::Invalid(error.to_string())
}

impl ClientAuth for JwtAuth {
    fn mode(&self) -> &'static str {
        "jwt"
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
            let Some(token) = crate::server_credential::extract_bearer_token(Some(header.as_str()))
            else {
                if header
                    .as_bytes()
                    .get(..7)
                    .is_some_and(|prefix| prefix.eq_ignore_ascii_case(b"Bearer "))
                {
                    return Err(AuthError::Invalid("malformed bearer token".to_string()));
                }
                return Ok(None);
            };
            if !looks_like_jwt(&token) {
                // Not a JWT: let a composite provider try the next provider.
                return Ok(None);
            }
            let claims = self
                .verifier
                .verify_at(&token, self.now_unix())
                .map_err(|error| jwt_error_to_auth(&error))?;

            let mut context = AuthContext::new(
                call.procedure.to_string(),
                call.connection.peer_addr.map(|address| address.to_string()),
                None,
            );
            context.sub = Some(claims.sub.clone());
            let roles: Vec<String> = claims.scope.as_deref().map_or_else(Vec::new, |scope| {
                scope.split_whitespace().map(str::to_string).collect()
            });
            context.roles.clone_from(&roles);
            let principal = Principal {
                subject: claims.sub.clone(),
                method: AuthMethod::Jwt {
                    issuer: claims.iss.clone(),
                },
                roles,
            };
            Ok(Some(AuthInfo::new(context).with_artifact(principal)))
        })
    }
}
