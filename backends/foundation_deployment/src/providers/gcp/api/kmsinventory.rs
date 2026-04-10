//! KmsinventoryProvider - State-aware kmsinventory API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       kmsinventory API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::kmsinventory::{
    kmsinventory_organizations_protected_resources_search_builder, kmsinventory_organizations_protected_resources_search_task,
    kmsinventory_projects_crypto_keys_list_builder, kmsinventory_projects_crypto_keys_list_task,
    kmsinventory_projects_locations_key_rings_crypto_keys_get_protected_resources_summary_builder, kmsinventory_projects_locations_key_rings_crypto_keys_get_protected_resources_summary_task,
    kmsinventory_projects_protected_resources_search_builder, kmsinventory_projects_protected_resources_search_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::kmsinventory::GoogleCloudKmsInventoryV1ListCryptoKeysResponse;
use crate::providers::gcp::clients::kmsinventory::GoogleCloudKmsInventoryV1ProtectedResourcesSummary;
use crate::providers::gcp::clients::kmsinventory::GoogleCloudKmsInventoryV1SearchProtectedResourcesResponse;
use crate::providers::gcp::clients::kmsinventory::KmsinventoryOrganizationsProtectedResourcesSearchArgs;
use crate::providers::gcp::clients::kmsinventory::KmsinventoryProjectsCryptoKeysListArgs;
use crate::providers::gcp::clients::kmsinventory::KmsinventoryProjectsLocationsKeyRingsCryptoKeysGetProtectedResourcesSummaryArgs;
use crate::providers::gcp::clients::kmsinventory::KmsinventoryProjectsProtectedResourcesSearchArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// KmsinventoryProvider with automatic state tracking.
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
/// let provider = KmsinventoryProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct KmsinventoryProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> KmsinventoryProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new KmsinventoryProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Kmsinventory organizations protected resources search.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudKmsInventoryV1SearchProtectedResourcesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn kmsinventory_organizations_protected_resources_search(
        &self,
        args: &KmsinventoryOrganizationsProtectedResourcesSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudKmsInventoryV1SearchProtectedResourcesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = kmsinventory_organizations_protected_resources_search_builder(
            &self.http_client,
            &args.scope,
            &args.cryptoKey,
            &args.pageSize,
            &args.pageToken,
            &args.resourceTypes,
        )
        .map_err(ProviderError::Api)?;

        let task = kmsinventory_organizations_protected_resources_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Kmsinventory projects crypto keys list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudKmsInventoryV1ListCryptoKeysResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn kmsinventory_projects_crypto_keys_list(
        &self,
        args: &KmsinventoryProjectsCryptoKeysListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudKmsInventoryV1ListCryptoKeysResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = kmsinventory_projects_crypto_keys_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = kmsinventory_projects_crypto_keys_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Kmsinventory projects locations key rings crypto keys get protected resources summary.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudKmsInventoryV1ProtectedResourcesSummary result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn kmsinventory_projects_locations_key_rings_crypto_keys_get_protected_resources_summary(
        &self,
        args: &KmsinventoryProjectsLocationsKeyRingsCryptoKeysGetProtectedResourcesSummaryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudKmsInventoryV1ProtectedResourcesSummary, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = kmsinventory_projects_locations_key_rings_crypto_keys_get_protected_resources_summary_builder(
            &self.http_client,
            &args.name,
            &args.fallbackScope,
        )
        .map_err(ProviderError::Api)?;

        let task = kmsinventory_projects_locations_key_rings_crypto_keys_get_protected_resources_summary_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Kmsinventory projects protected resources search.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudKmsInventoryV1SearchProtectedResourcesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn kmsinventory_projects_protected_resources_search(
        &self,
        args: &KmsinventoryProjectsProtectedResourcesSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudKmsInventoryV1SearchProtectedResourcesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = kmsinventory_projects_protected_resources_search_builder(
            &self.http_client,
            &args.scope,
            &args.cryptoKey,
            &args.pageSize,
            &args.pageToken,
            &args.resourceTypes,
        )
        .map_err(ProviderError::Api)?;

        let task = kmsinventory_projects_protected_resources_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
