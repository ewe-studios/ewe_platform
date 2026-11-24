//! A composite provider that tries several providers in order.

use std::sync::Arc;

use crate::server_credential::{AuthFuture, CallCredentials, ClientAuth};

/// Tries providers in order. The first `Ok(Some)` wins. An `Err` stops the
/// chain: the caller showed credentials of that kind and they failed, so a
/// weaker provider must not accept the call instead.
pub struct AnyOf(pub Vec<Arc<dyn ClientAuth>>);

impl std::fmt::Debug for AnyOf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let modes: Vec<&'static str> = self.0.iter().map(|provider| provider.mode()).collect();
        f.debug_tuple("AnyOf").field(&modes).finish()
    }
}

impl AnyOf {
    /// A provider chain from an ordered list.
    #[must_use]
    pub fn new(providers: Vec<Arc<dyn ClientAuth>>) -> Self {
        Self(providers)
    }
}

impl ClientAuth for AnyOf {
    fn mode(&self) -> &'static str {
        "any-of"
    }

    fn authenticate<'a>(&'a self, call: CallCredentials<'a>) -> AuthFuture<'a> {
        Box::pin(async move {
            for provider in &self.0 {
                match provider.authenticate(call).await {
                    Ok(Some(info)) => return Ok(Some(info)),
                    Ok(None) => {}
                    Err(error) => return Err(error),
                }
            }
            Ok(None)
        })
    }
}
