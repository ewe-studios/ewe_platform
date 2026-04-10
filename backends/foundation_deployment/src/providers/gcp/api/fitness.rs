//! FitnessProvider - State-aware fitness API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       fitness API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::fitness::{
    fitness_users_data_sources_create_builder, fitness_users_data_sources_create_task,
    fitness_users_data_sources_delete_builder, fitness_users_data_sources_delete_task,
    fitness_users_data_sources_get_builder, fitness_users_data_sources_get_task,
    fitness_users_data_sources_list_builder, fitness_users_data_sources_list_task,
    fitness_users_data_sources_update_builder, fitness_users_data_sources_update_task,
    fitness_users_data_sources_data_point_changes_list_builder, fitness_users_data_sources_data_point_changes_list_task,
    fitness_users_data_sources_datasets_delete_builder, fitness_users_data_sources_datasets_delete_task,
    fitness_users_data_sources_datasets_get_builder, fitness_users_data_sources_datasets_get_task,
    fitness_users_data_sources_datasets_patch_builder, fitness_users_data_sources_datasets_patch_task,
    fitness_users_dataset_aggregate_builder, fitness_users_dataset_aggregate_task,
    fitness_users_sessions_delete_builder, fitness_users_sessions_delete_task,
    fitness_users_sessions_list_builder, fitness_users_sessions_list_task,
    fitness_users_sessions_update_builder, fitness_users_sessions_update_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::fitness::AggregateResponse;
use crate::providers::gcp::clients::fitness::DataSource;
use crate::providers::gcp::clients::fitness::Dataset;
use crate::providers::gcp::clients::fitness::ListDataPointChangesResponse;
use crate::providers::gcp::clients::fitness::ListDataSourcesResponse;
use crate::providers::gcp::clients::fitness::ListSessionsResponse;
use crate::providers::gcp::clients::fitness::Session;
use crate::providers::gcp::clients::fitness::FitnessUsersDataSourcesCreateArgs;
use crate::providers::gcp::clients::fitness::FitnessUsersDataSourcesDataPointChangesListArgs;
use crate::providers::gcp::clients::fitness::FitnessUsersDataSourcesDatasetsDeleteArgs;
use crate::providers::gcp::clients::fitness::FitnessUsersDataSourcesDatasetsGetArgs;
use crate::providers::gcp::clients::fitness::FitnessUsersDataSourcesDatasetsPatchArgs;
use crate::providers::gcp::clients::fitness::FitnessUsersDataSourcesDeleteArgs;
use crate::providers::gcp::clients::fitness::FitnessUsersDataSourcesGetArgs;
use crate::providers::gcp::clients::fitness::FitnessUsersDataSourcesListArgs;
use crate::providers::gcp::clients::fitness::FitnessUsersDataSourcesUpdateArgs;
use crate::providers::gcp::clients::fitness::FitnessUsersDatasetAggregateArgs;
use crate::providers::gcp::clients::fitness::FitnessUsersSessionsDeleteArgs;
use crate::providers::gcp::clients::fitness::FitnessUsersSessionsListArgs;
use crate::providers::gcp::clients::fitness::FitnessUsersSessionsUpdateArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// FitnessProvider with automatic state tracking.
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
/// let provider = FitnessProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct FitnessProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> FitnessProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new FitnessProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Fitness users data sources create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DataSource result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn fitness_users_data_sources_create(
        &self,
        args: &FitnessUsersDataSourcesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DataSource, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = fitness_users_data_sources_create_builder(
            &self.http_client,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = fitness_users_data_sources_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Fitness users data sources delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DataSource result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn fitness_users_data_sources_delete(
        &self,
        args: &FitnessUsersDataSourcesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DataSource, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = fitness_users_data_sources_delete_builder(
            &self.http_client,
            &args.userId,
            &args.dataSourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = fitness_users_data_sources_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Fitness users data sources get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DataSource result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn fitness_users_data_sources_get(
        &self,
        args: &FitnessUsersDataSourcesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DataSource, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = fitness_users_data_sources_get_builder(
            &self.http_client,
            &args.userId,
            &args.dataSourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = fitness_users_data_sources_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Fitness users data sources list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDataSourcesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn fitness_users_data_sources_list(
        &self,
        args: &FitnessUsersDataSourcesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDataSourcesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = fitness_users_data_sources_list_builder(
            &self.http_client,
            &args.userId,
            &args.dataTypeName,
        )
        .map_err(ProviderError::Api)?;

        let task = fitness_users_data_sources_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Fitness users data sources update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DataSource result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn fitness_users_data_sources_update(
        &self,
        args: &FitnessUsersDataSourcesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DataSource, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = fitness_users_data_sources_update_builder(
            &self.http_client,
            &args.userId,
            &args.dataSourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = fitness_users_data_sources_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Fitness users data sources data point changes list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDataPointChangesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn fitness_users_data_sources_data_point_changes_list(
        &self,
        args: &FitnessUsersDataSourcesDataPointChangesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDataPointChangesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = fitness_users_data_sources_data_point_changes_list_builder(
            &self.http_client,
            &args.userId,
            &args.dataSourceId,
            &args.limit,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = fitness_users_data_sources_data_point_changes_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Fitness users data sources datasets delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn fitness_users_data_sources_datasets_delete(
        &self,
        args: &FitnessUsersDataSourcesDatasetsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = fitness_users_data_sources_datasets_delete_builder(
            &self.http_client,
            &args.userId,
            &args.dataSourceId,
            &args.datasetId,
        )
        .map_err(ProviderError::Api)?;

        let task = fitness_users_data_sources_datasets_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Fitness users data sources datasets get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Dataset result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn fitness_users_data_sources_datasets_get(
        &self,
        args: &FitnessUsersDataSourcesDatasetsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Dataset, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = fitness_users_data_sources_datasets_get_builder(
            &self.http_client,
            &args.userId,
            &args.dataSourceId,
            &args.datasetId,
            &args.limit,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = fitness_users_data_sources_datasets_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Fitness users data sources datasets patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Dataset result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn fitness_users_data_sources_datasets_patch(
        &self,
        args: &FitnessUsersDataSourcesDatasetsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Dataset, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = fitness_users_data_sources_datasets_patch_builder(
            &self.http_client,
            &args.userId,
            &args.dataSourceId,
            &args.datasetId,
        )
        .map_err(ProviderError::Api)?;

        let task = fitness_users_data_sources_datasets_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Fitness users dataset aggregate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AggregateResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn fitness_users_dataset_aggregate(
        &self,
        args: &FitnessUsersDatasetAggregateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AggregateResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = fitness_users_dataset_aggregate_builder(
            &self.http_client,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = fitness_users_dataset_aggregate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Fitness users sessions delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn fitness_users_sessions_delete(
        &self,
        args: &FitnessUsersSessionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = fitness_users_sessions_delete_builder(
            &self.http_client,
            &args.userId,
            &args.sessionId,
        )
        .map_err(ProviderError::Api)?;

        let task = fitness_users_sessions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Fitness users sessions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListSessionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn fitness_users_sessions_list(
        &self,
        args: &FitnessUsersSessionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSessionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = fitness_users_sessions_list_builder(
            &self.http_client,
            &args.userId,
            &args.activityType,
            &args.endTime,
            &args.includeDeleted,
            &args.pageToken,
            &args.startTime,
        )
        .map_err(ProviderError::Api)?;

        let task = fitness_users_sessions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Fitness users sessions update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Session result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn fitness_users_sessions_update(
        &self,
        args: &FitnessUsersSessionsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Session, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = fitness_users_sessions_update_builder(
            &self.http_client,
            &args.userId,
            &args.sessionId,
        )
        .map_err(ProviderError::Api)?;

        let task = fitness_users_sessions_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
