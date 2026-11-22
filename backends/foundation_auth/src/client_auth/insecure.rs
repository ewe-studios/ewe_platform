//! The plaintext provider for development.

use crate::server_credential::{
    AuthError, AuthFuture, AuthInfo, AuthMethod, CallCredentials, ClientAuth, Principal,
};
use crate::shared::middleware::AuthContext;

/// The subject of a caller that this provider accepted.
const SUBJECT: &str = "insecure";

/// Accepts every call.
///
/// It refuses a peer that is not on the loopback address unless
/// `allow_non_loopback` is set. A Worker has no peer address, so a Worker must
/// set it. It logs a warning when it is built.
///
/// Development only.
#[derive(Debug, Clone)]
pub struct InsecureNoAuth {
    /// Accept a peer that is not on the loopback address.
    pub allow_non_loopback: bool,
}

impl Default for InsecureNoAuth {
    fn default() -> Self {
        Self::new()
    }
}

impl InsecureNoAuth {
    /// A provider that accepts only a loopback peer. It logs a warning.
    #[must_use]
    pub fn new() -> Self {
        tracing::warn!(
            "InsecureNoAuth is active: every loopback call is accepted without credentials. \
             Development only."
        );
        Self {
            allow_non_loopback: false,
        }
    }

    /// Say whether a peer that is not on the loopback address is accepted.
    #[must_use]
    pub fn with_allow_non_loopback(mut self, allow: bool) -> Self {
        self.allow_non_loopback = allow;
        self
    }
}

impl ClientAuth for InsecureNoAuth {
    fn mode(&self) -> &'static str {
        "insecure"
    }

    fn authenticate<'a>(&'a self, call: CallCredentials<'a>) -> AuthFuture<'a> {
        Box::pin(async move {
            match call.connection.peer_addr {
                Some(address) if address.ip().is_loopback() => {}
                _ if self.allow_non_loopback => {}
                Some(address) => {
                    return Err(AuthError::Forbidden(format!(
                        "insecure auth accepts only a loopback peer, not {address}"
                    )));
                }
                None => {
                    return Err(AuthError::Forbidden(
                        "insecure auth has no peer address; set allow_non_loopback on a host \
                         without a peer address"
                            .to_string(),
                    ));
                }
            }

            let mut context = AuthContext::new(
                call.procedure.to_string(),
                call.connection.peer_addr.map(|address| address.to_string()),
                None,
            );
            context.sub = Some(SUBJECT.to_string());
            let principal = Principal {
                subject: SUBJECT.to_string(),
                method: AuthMethod::Insecure,
                roles: Vec::new(),
            };
            Ok(Some(AuthInfo::new(context).with_artifact(principal)))
        })
    }
}
