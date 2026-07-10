//! Shared proxy runtime state.
//!
//! WHY: The HTTP framework constructs handlers via `ServeFactory::create(&bag)`,
//! so all proxy state the handler needs — the router table, backend health and
//! load-balancer counters, and the pooled upstream client — must live in the
//! `ContextBag`. One `Arc<ProxyState>` carries it all.
//!
//! WHAT: [`ProxyState`] holds the [`Router`] and the shared [`SharedHttpClient`],
//! plus the scheme the front end terminates (used for `X-Forwarded-Proto`).

use std::sync::Arc;

use crate::forward::SharedHttpClient;
use crate::router::Router;

/// Immutable-after-startup proxy state shared by every request handler.
pub struct ProxyState {
    router: Arc<Router>,
    client: SharedHttpClient,
    scheme: String,
}

impl std::fmt::Debug for ProxyState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProxyState")
            .field("router", &self.router)
            .field("scheme", &self.scheme)
            .finish_non_exhaustive()
    }
}

impl ProxyState {
    /// Build proxy state from a router and pooled client.
    ///
    /// `scheme` is what the front end speaks to clients (`http` in stage 1); it
    /// becomes the `X-Forwarded-Proto` value sent upstream.
    #[must_use]
    pub fn new(router: Arc<Router>, client: SharedHttpClient, scheme: impl Into<String>) -> Self {
        Self {
            router,
            client,
            scheme: scheme.into(),
        }
    }

    #[must_use]
    pub fn router(&self) -> &Arc<Router> {
        &self.router
    }

    #[must_use]
    pub fn client(&self) -> &SharedHttpClient {
        &self.client
    }

    #[must_use]
    pub fn scheme(&self) -> &str {
        &self.scheme
    }
}
