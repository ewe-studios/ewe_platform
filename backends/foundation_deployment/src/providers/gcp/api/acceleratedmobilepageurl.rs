//! AcceleratedmobilepageurlProvider - State-aware acceleratedmobilepageurl API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       acceleratedmobilepageurl API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::acceleratedmobilepageurl::{
    acceleratedmobilepageurl_amp_urls_batch_get_builder, acceleratedmobilepageurl_amp_urls_batch_get_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::acceleratedmobilepageurl::BatchGetAmpUrlsResponse;
use crate::providers::gcp::clients::acceleratedmobilepageurl::AcceleratedmobilepageurlAmpUrlsBatchGetArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// AcceleratedmobilepageurlProvider with automatic state tracking.
///
/// # Type Parameters
///
/// * `S` - StateStore implementation (FileStateStore, SqliteStateStore, etc.)
///
/// # Example
///
/// ```rust
/// let state_store = FileStateStore::new("/path", "my-project", "dev");
/// let client = ProviderClient::new("my-project", "dev", state_store);
/// let http_client = SimpleHttpClient::new(...);
/// let provider = AcceleratedmobilepageurlProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct AcceleratedmobilepageurlProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> AcceleratedmobilepageurlProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new AcceleratedmobilepageurlProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Acceleratedmobilepageurl amp urls batch get.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchGetAmpUrlsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn acceleratedmobilepageurl_amp_urls_batch_get(
        &self,
        args: &AcceleratedmobilepageurlAmpUrlsBatchGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchGetAmpUrlsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = acceleratedmobilepageurl_amp_urls_batch_get_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = acceleratedmobilepageurl_amp_urls_batch_get_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
