//! SafebrowsingProvider - State-aware safebrowsing API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       safebrowsing API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::safebrowsing::{
    safebrowsing_hash_list_get_builder, safebrowsing_hash_list_get_task,
    safebrowsing_hash_lists_batch_get_builder, safebrowsing_hash_lists_batch_get_task,
    safebrowsing_hash_lists_list_builder, safebrowsing_hash_lists_list_task,
    safebrowsing_hashes_search_builder, safebrowsing_hashes_search_task,
    safebrowsing_urls_search_builder, safebrowsing_urls_search_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::safebrowsing::GoogleSecuritySafebrowsingV5BatchGetHashListsResponse;
use crate::providers::gcp::clients::safebrowsing::GoogleSecuritySafebrowsingV5HashList;
use crate::providers::gcp::clients::safebrowsing::GoogleSecuritySafebrowsingV5ListHashListsResponse;
use crate::providers::gcp::clients::safebrowsing::GoogleSecuritySafebrowsingV5SearchHashesResponse;
use crate::providers::gcp::clients::safebrowsing::GoogleSecuritySafebrowsingV5SearchUrlsResponse;
use crate::providers::gcp::clients::safebrowsing::SafebrowsingHashListGetArgs;
use crate::providers::gcp::clients::safebrowsing::SafebrowsingHashListsBatchGetArgs;
use crate::providers::gcp::clients::safebrowsing::SafebrowsingHashListsListArgs;
use crate::providers::gcp::clients::safebrowsing::SafebrowsingHashesSearchArgs;
use crate::providers::gcp::clients::safebrowsing::SafebrowsingUrlsSearchArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// SafebrowsingProvider with automatic state tracking.
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
/// let provider = SafebrowsingProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct SafebrowsingProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> SafebrowsingProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new SafebrowsingProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Safebrowsing hash list get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleSecuritySafebrowsingV5HashList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn safebrowsing_hash_list_get(
        &self,
        args: &SafebrowsingHashListGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleSecuritySafebrowsingV5HashList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = safebrowsing_hash_list_get_builder(
            &self.http_client,
            &args.name,
            &args.sizeConstraints.maxDatabaseEntries,
            &args.sizeConstraints.maxUpdateEntries,
            &args.version,
        )
        .map_err(ProviderError::Api)?;

        let task = safebrowsing_hash_list_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Safebrowsing hash lists batch get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleSecuritySafebrowsingV5BatchGetHashListsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn safebrowsing_hash_lists_batch_get(
        &self,
        args: &SafebrowsingHashListsBatchGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleSecuritySafebrowsingV5BatchGetHashListsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = safebrowsing_hash_lists_batch_get_builder(
            &self.http_client,
            &args.names,
            &args.sizeConstraints.maxDatabaseEntries,
            &args.sizeConstraints.maxUpdateEntries,
            &args.version,
        )
        .map_err(ProviderError::Api)?;

        let task = safebrowsing_hash_lists_batch_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Safebrowsing hash lists list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleSecuritySafebrowsingV5ListHashListsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn safebrowsing_hash_lists_list(
        &self,
        args: &SafebrowsingHashListsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleSecuritySafebrowsingV5ListHashListsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = safebrowsing_hash_lists_list_builder(
            &self.http_client,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = safebrowsing_hash_lists_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Safebrowsing hashes search.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleSecuritySafebrowsingV5SearchHashesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn safebrowsing_hashes_search(
        &self,
        args: &SafebrowsingHashesSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleSecuritySafebrowsingV5SearchHashesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = safebrowsing_hashes_search_builder(
            &self.http_client,
            &args.hashPrefixes,
        )
        .map_err(ProviderError::Api)?;

        let task = safebrowsing_hashes_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Safebrowsing urls search.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleSecuritySafebrowsingV5SearchUrlsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn safebrowsing_urls_search(
        &self,
        args: &SafebrowsingUrlsSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleSecuritySafebrowsingV5SearchUrlsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = safebrowsing_urls_search_builder(
            &self.http_client,
            &args.urls,
        )
        .map_err(ProviderError::Api)?;

        let task = safebrowsing_urls_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
