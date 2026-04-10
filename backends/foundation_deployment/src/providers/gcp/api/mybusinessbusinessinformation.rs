//! MybusinessbusinessinformationProvider - State-aware mybusinessbusinessinformation API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       mybusinessbusinessinformation API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::mybusinessbusinessinformation::{
    mybusinessbusinessinformation_accounts_locations_create_builder, mybusinessbusinessinformation_accounts_locations_create_task,
    mybusinessbusinessinformation_google_locations_search_builder, mybusinessbusinessinformation_google_locations_search_task,
    mybusinessbusinessinformation_locations_delete_builder, mybusinessbusinessinformation_locations_delete_task,
    mybusinessbusinessinformation_locations_patch_builder, mybusinessbusinessinformation_locations_patch_task,
    mybusinessbusinessinformation_locations_update_attributes_builder, mybusinessbusinessinformation_locations_update_attributes_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::mybusinessbusinessinformation::Attributes;
use crate::providers::gcp::clients::mybusinessbusinessinformation::Empty;
use crate::providers::gcp::clients::mybusinessbusinessinformation::Location;
use crate::providers::gcp::clients::mybusinessbusinessinformation::SearchGoogleLocationsResponse;
use crate::providers::gcp::clients::mybusinessbusinessinformation::MybusinessbusinessinformationAccountsLocationsCreateArgs;
use crate::providers::gcp::clients::mybusinessbusinessinformation::MybusinessbusinessinformationGoogleLocationsSearchArgs;
use crate::providers::gcp::clients::mybusinessbusinessinformation::MybusinessbusinessinformationLocationsDeleteArgs;
use crate::providers::gcp::clients::mybusinessbusinessinformation::MybusinessbusinessinformationLocationsPatchArgs;
use crate::providers::gcp::clients::mybusinessbusinessinformation::MybusinessbusinessinformationLocationsUpdateAttributesArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// MybusinessbusinessinformationProvider with automatic state tracking.
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
/// let provider = MybusinessbusinessinformationProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct MybusinessbusinessinformationProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> MybusinessbusinessinformationProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new MybusinessbusinessinformationProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Mybusinessbusinessinformation accounts locations create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Location result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn mybusinessbusinessinformation_accounts_locations_create(
        &self,
        args: &MybusinessbusinessinformationAccountsLocationsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessbusinessinformation_accounts_locations_create_builder(
            &self.http_client,
            &args.parent,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessbusinessinformation_accounts_locations_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Mybusinessbusinessinformation google locations search.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchGoogleLocationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn mybusinessbusinessinformation_google_locations_search(
        &self,
        args: &MybusinessbusinessinformationGoogleLocationsSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchGoogleLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessbusinessinformation_google_locations_search_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessbusinessinformation_google_locations_search_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Mybusinessbusinessinformation locations delete.
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
    pub fn mybusinessbusinessinformation_locations_delete(
        &self,
        args: &MybusinessbusinessinformationLocationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessbusinessinformation_locations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessbusinessinformation_locations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Mybusinessbusinessinformation locations patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Location result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn mybusinessbusinessinformation_locations_patch(
        &self,
        args: &MybusinessbusinessinformationLocationsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessbusinessinformation_locations_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessbusinessinformation_locations_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Mybusinessbusinessinformation locations update attributes.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Attributes result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn mybusinessbusinessinformation_locations_update_attributes(
        &self,
        args: &MybusinessbusinessinformationLocationsUpdateAttributesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Attributes, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessbusinessinformation_locations_update_attributes_builder(
            &self.http_client,
            &args.name,
            &args.attributeMask,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessbusinessinformation_locations_update_attributes_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
