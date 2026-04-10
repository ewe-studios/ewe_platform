//! AdexperiencereportProvider - State-aware adexperiencereport API client.
//!
//! No mutating endpoints to wrap.

#![cfg(feature = "gcp")]

use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use std::sync::Arc;

/// AdexperiencereportProvider with automatic state tracking.
#[derive(Clone)]
pub struct AdexperiencereportProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> AdexperiencereportProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new AdexperiencereportProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

}
