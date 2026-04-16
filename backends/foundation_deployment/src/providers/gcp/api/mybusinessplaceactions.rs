//! MybusinessplaceactionsProvider - State-aware mybusinessplaceactions API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       mybusinessplaceactions API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::mybusinessplaceactions::{
    mybusinessplaceactions_locations_place_action_links_create_builder, mybusinessplaceactions_locations_place_action_links_create_task,
    mybusinessplaceactions_locations_place_action_links_delete_builder, mybusinessplaceactions_locations_place_action_links_delete_task,
    mybusinessplaceactions_locations_place_action_links_get_builder, mybusinessplaceactions_locations_place_action_links_get_task,
    mybusinessplaceactions_locations_place_action_links_list_builder, mybusinessplaceactions_locations_place_action_links_list_task,
    mybusinessplaceactions_locations_place_action_links_patch_builder, mybusinessplaceactions_locations_place_action_links_patch_task,
    mybusinessplaceactions_place_action_type_metadata_list_builder, mybusinessplaceactions_place_action_type_metadata_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::mybusinessplaceactions::Empty;
use crate::providers::gcp::clients::mybusinessplaceactions::ListPlaceActionLinksResponse;
use crate::providers::gcp::clients::mybusinessplaceactions::ListPlaceActionTypeMetadataResponse;
use crate::providers::gcp::clients::mybusinessplaceactions::PlaceActionLink;
use crate::providers::gcp::clients::mybusinessplaceactions::MybusinessplaceactionsLocationsPlaceActionLinksCreateArgs;
use crate::providers::gcp::clients::mybusinessplaceactions::MybusinessplaceactionsLocationsPlaceActionLinksDeleteArgs;
use crate::providers::gcp::clients::mybusinessplaceactions::MybusinessplaceactionsLocationsPlaceActionLinksGetArgs;
use crate::providers::gcp::clients::mybusinessplaceactions::MybusinessplaceactionsLocationsPlaceActionLinksListArgs;
use crate::providers::gcp::clients::mybusinessplaceactions::MybusinessplaceactionsLocationsPlaceActionLinksPatchArgs;
use crate::providers::gcp::clients::mybusinessplaceactions::MybusinessplaceactionsPlaceActionTypeMetadataListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// MybusinessplaceactionsProvider with automatic state tracking.
///
/// # Type Parameters
///
/// * `S` - StateStore implementation (FileStateStore, SqliteStateStore, etc.)
/// * `R` - DNS resolver type for HTTP client
///
/// # Example
///
/// ```rust
/// let state_store = FileStateStore::new("/path", "my-project", "dev");
/// let http_client = SimpleHttpClient::with_resolver(StaticSocketAddr::new(addr));
/// let client = ProviderClient::new("my-project", "dev", state_store, http_client);
/// let provider = MybusinessplaceactionsProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct MybusinessplaceactionsProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> MybusinessplaceactionsProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new MybusinessplaceactionsProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new MybusinessplaceactionsProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Mybusinessplaceactions locations place action links create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PlaceActionLink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn mybusinessplaceactions_locations_place_action_links_create(
        &self,
        args: &MybusinessplaceactionsLocationsPlaceActionLinksCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PlaceActionLink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessplaceactions_locations_place_action_links_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessplaceactions_locations_place_action_links_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Mybusinessplaceactions locations place action links delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Empty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn mybusinessplaceactions_locations_place_action_links_delete(
        &self,
        args: &MybusinessplaceactionsLocationsPlaceActionLinksDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessplaceactions_locations_place_action_links_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessplaceactions_locations_place_action_links_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Mybusinessplaceactions locations place action links get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PlaceActionLink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn mybusinessplaceactions_locations_place_action_links_get(
        &self,
        args: &MybusinessplaceactionsLocationsPlaceActionLinksGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PlaceActionLink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessplaceactions_locations_place_action_links_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessplaceactions_locations_place_action_links_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Mybusinessplaceactions locations place action links list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListPlaceActionLinksResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn mybusinessplaceactions_locations_place_action_links_list(
        &self,
        args: &MybusinessplaceactionsLocationsPlaceActionLinksListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListPlaceActionLinksResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessplaceactions_locations_place_action_links_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessplaceactions_locations_place_action_links_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Mybusinessplaceactions locations place action links patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PlaceActionLink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn mybusinessplaceactions_locations_place_action_links_patch(
        &self,
        args: &MybusinessplaceactionsLocationsPlaceActionLinksPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PlaceActionLink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessplaceactions_locations_place_action_links_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessplaceactions_locations_place_action_links_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Mybusinessplaceactions place action type metadata list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListPlaceActionTypeMetadataResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn mybusinessplaceactions_place_action_type_metadata_list(
        &self,
        args: &MybusinessplaceactionsPlaceActionTypeMetadataListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListPlaceActionTypeMetadataResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessplaceactions_place_action_type_metadata_list_builder(
            &self.http_client,
            &args.filter,
            &args.languageCode,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessplaceactions_place_action_type_metadata_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
