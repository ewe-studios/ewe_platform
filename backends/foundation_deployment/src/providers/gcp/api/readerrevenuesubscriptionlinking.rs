//! ReaderrevenuesubscriptionlinkingProvider - State-aware readerrevenuesubscriptionlinking API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       readerrevenuesubscriptionlinking API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::readerrevenuesubscriptionlinking::{
    readerrevenuesubscriptionlinking_publications_readers_delete_builder, readerrevenuesubscriptionlinking_publications_readers_delete_task,
    readerrevenuesubscriptionlinking_publications_readers_update_entitlements_builder, readerrevenuesubscriptionlinking_publications_readers_update_entitlements_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::readerrevenuesubscriptionlinking::DeleteReaderResponse;
use crate::providers::gcp::clients::readerrevenuesubscriptionlinking::ReaderEntitlements;
use crate::providers::gcp::clients::readerrevenuesubscriptionlinking::ReaderrevenuesubscriptionlinkingPublicationsReadersDeleteArgs;
use crate::providers::gcp::clients::readerrevenuesubscriptionlinking::ReaderrevenuesubscriptionlinkingPublicationsReadersUpdateEntitlementsArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// ReaderrevenuesubscriptionlinkingProvider with automatic state tracking.
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
/// let provider = ReaderrevenuesubscriptionlinkingProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct ReaderrevenuesubscriptionlinkingProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> ReaderrevenuesubscriptionlinkingProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new ReaderrevenuesubscriptionlinkingProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Readerrevenuesubscriptionlinking publications readers delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeleteReaderResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn readerrevenuesubscriptionlinking_publications_readers_delete(
        &self,
        args: &ReaderrevenuesubscriptionlinkingPublicationsReadersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeleteReaderResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = readerrevenuesubscriptionlinking_publications_readers_delete_builder(
            &self.http_client,
            &args.name,
            &args.force,
        )
        .map_err(ProviderError::Api)?;

        let task = readerrevenuesubscriptionlinking_publications_readers_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Readerrevenuesubscriptionlinking publications readers update entitlements.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ReaderEntitlements result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn readerrevenuesubscriptionlinking_publications_readers_update_entitlements(
        &self,
        args: &ReaderrevenuesubscriptionlinkingPublicationsReadersUpdateEntitlementsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ReaderEntitlements, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = readerrevenuesubscriptionlinking_publications_readers_update_entitlements_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = readerrevenuesubscriptionlinking_publications_readers_update_entitlements_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
