//! FirebasedatabaseProvider - State-aware firebasedatabase API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       firebasedatabase API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::firebasedatabase::{
    firebasedatabase_projects_locations_instances_create_builder, firebasedatabase_projects_locations_instances_create_task,
    firebasedatabase_projects_locations_instances_delete_builder, firebasedatabase_projects_locations_instances_delete_task,
    firebasedatabase_projects_locations_instances_disable_builder, firebasedatabase_projects_locations_instances_disable_task,
    firebasedatabase_projects_locations_instances_get_builder, firebasedatabase_projects_locations_instances_get_task,
    firebasedatabase_projects_locations_instances_list_builder, firebasedatabase_projects_locations_instances_list_task,
    firebasedatabase_projects_locations_instances_reenable_builder, firebasedatabase_projects_locations_instances_reenable_task,
    firebasedatabase_projects_locations_instances_undelete_builder, firebasedatabase_projects_locations_instances_undelete_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::firebasedatabase::DatabaseInstance;
use crate::providers::gcp::clients::firebasedatabase::ListDatabaseInstancesResponse;
use crate::providers::gcp::clients::firebasedatabase::FirebasedatabaseProjectsLocationsInstancesCreateArgs;
use crate::providers::gcp::clients::firebasedatabase::FirebasedatabaseProjectsLocationsInstancesDeleteArgs;
use crate::providers::gcp::clients::firebasedatabase::FirebasedatabaseProjectsLocationsInstancesDisableArgs;
use crate::providers::gcp::clients::firebasedatabase::FirebasedatabaseProjectsLocationsInstancesGetArgs;
use crate::providers::gcp::clients::firebasedatabase::FirebasedatabaseProjectsLocationsInstancesListArgs;
use crate::providers::gcp::clients::firebasedatabase::FirebasedatabaseProjectsLocationsInstancesReenableArgs;
use crate::providers::gcp::clients::firebasedatabase::FirebasedatabaseProjectsLocationsInstancesUndeleteArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// FirebasedatabaseProvider with automatic state tracking.
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
/// let provider = FirebasedatabaseProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct FirebasedatabaseProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> FirebasedatabaseProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new FirebasedatabaseProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Firebasedatabase projects locations instances create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DatabaseInstance result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebasedatabase_projects_locations_instances_create(
        &self,
        args: &FirebasedatabaseProjectsLocationsInstancesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DatabaseInstance, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebasedatabase_projects_locations_instances_create_builder(
            &self.http_client,
            &args.parent,
            &args.databaseId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = firebasedatabase_projects_locations_instances_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebasedatabase projects locations instances delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DatabaseInstance result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebasedatabase_projects_locations_instances_delete(
        &self,
        args: &FirebasedatabaseProjectsLocationsInstancesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DatabaseInstance, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebasedatabase_projects_locations_instances_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebasedatabase_projects_locations_instances_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebasedatabase projects locations instances disable.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DatabaseInstance result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebasedatabase_projects_locations_instances_disable(
        &self,
        args: &FirebasedatabaseProjectsLocationsInstancesDisableArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DatabaseInstance, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebasedatabase_projects_locations_instances_disable_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebasedatabase_projects_locations_instances_disable_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebasedatabase projects locations instances get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DatabaseInstance result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firebasedatabase_projects_locations_instances_get(
        &self,
        args: &FirebasedatabaseProjectsLocationsInstancesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DatabaseInstance, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebasedatabase_projects_locations_instances_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebasedatabase_projects_locations_instances_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebasedatabase projects locations instances list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDatabaseInstancesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firebasedatabase_projects_locations_instances_list(
        &self,
        args: &FirebasedatabaseProjectsLocationsInstancesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDatabaseInstancesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebasedatabase_projects_locations_instances_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.showDeleted,
        )
        .map_err(ProviderError::Api)?;

        let task = firebasedatabase_projects_locations_instances_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebasedatabase projects locations instances reenable.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DatabaseInstance result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebasedatabase_projects_locations_instances_reenable(
        &self,
        args: &FirebasedatabaseProjectsLocationsInstancesReenableArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DatabaseInstance, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebasedatabase_projects_locations_instances_reenable_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebasedatabase_projects_locations_instances_reenable_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebasedatabase projects locations instances undelete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DatabaseInstance result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebasedatabase_projects_locations_instances_undelete(
        &self,
        args: &FirebasedatabaseProjectsLocationsInstancesUndeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DatabaseInstance, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebasedatabase_projects_locations_instances_undelete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebasedatabase_projects_locations_instances_undelete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
