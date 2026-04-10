//! BigquerydatatransferProvider - State-aware bigquerydatatransfer API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       bigquerydatatransfer API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::bigquerydatatransfer::{
    bigquerydatatransfer_projects_enroll_data_sources_builder, bigquerydatatransfer_projects_enroll_data_sources_task,
    bigquerydatatransfer_projects_data_sources_check_valid_creds_builder, bigquerydatatransfer_projects_data_sources_check_valid_creds_task,
    bigquerydatatransfer_projects_locations_enroll_data_sources_builder, bigquerydatatransfer_projects_locations_enroll_data_sources_task,
    bigquerydatatransfer_projects_locations_unenroll_data_sources_builder, bigquerydatatransfer_projects_locations_unenroll_data_sources_task,
    bigquerydatatransfer_projects_locations_data_sources_check_valid_creds_builder, bigquerydatatransfer_projects_locations_data_sources_check_valid_creds_task,
    bigquerydatatransfer_projects_locations_transfer_configs_create_builder, bigquerydatatransfer_projects_locations_transfer_configs_create_task,
    bigquerydatatransfer_projects_locations_transfer_configs_delete_builder, bigquerydatatransfer_projects_locations_transfer_configs_delete_task,
    bigquerydatatransfer_projects_locations_transfer_configs_patch_builder, bigquerydatatransfer_projects_locations_transfer_configs_patch_task,
    bigquerydatatransfer_projects_locations_transfer_configs_schedule_runs_builder, bigquerydatatransfer_projects_locations_transfer_configs_schedule_runs_task,
    bigquerydatatransfer_projects_locations_transfer_configs_start_manual_runs_builder, bigquerydatatransfer_projects_locations_transfer_configs_start_manual_runs_task,
    bigquerydatatransfer_projects_locations_transfer_configs_runs_delete_builder, bigquerydatatransfer_projects_locations_transfer_configs_runs_delete_task,
    bigquerydatatransfer_projects_transfer_configs_create_builder, bigquerydatatransfer_projects_transfer_configs_create_task,
    bigquerydatatransfer_projects_transfer_configs_delete_builder, bigquerydatatransfer_projects_transfer_configs_delete_task,
    bigquerydatatransfer_projects_transfer_configs_patch_builder, bigquerydatatransfer_projects_transfer_configs_patch_task,
    bigquerydatatransfer_projects_transfer_configs_schedule_runs_builder, bigquerydatatransfer_projects_transfer_configs_schedule_runs_task,
    bigquerydatatransfer_projects_transfer_configs_start_manual_runs_builder, bigquerydatatransfer_projects_transfer_configs_start_manual_runs_task,
    bigquerydatatransfer_projects_transfer_configs_runs_delete_builder, bigquerydatatransfer_projects_transfer_configs_runs_delete_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::bigquerydatatransfer::CheckValidCredsResponse;
use crate::providers::gcp::clients::bigquerydatatransfer::Empty;
use crate::providers::gcp::clients::bigquerydatatransfer::ScheduleTransferRunsResponse;
use crate::providers::gcp::clients::bigquerydatatransfer::StartManualTransferRunsResponse;
use crate::providers::gcp::clients::bigquerydatatransfer::TransferConfig;
use crate::providers::gcp::clients::bigquerydatatransfer::BigquerydatatransferProjectsDataSourcesCheckValidCredsArgs;
use crate::providers::gcp::clients::bigquerydatatransfer::BigquerydatatransferProjectsEnrollDataSourcesArgs;
use crate::providers::gcp::clients::bigquerydatatransfer::BigquerydatatransferProjectsLocationsDataSourcesCheckValidCredsArgs;
use crate::providers::gcp::clients::bigquerydatatransfer::BigquerydatatransferProjectsLocationsEnrollDataSourcesArgs;
use crate::providers::gcp::clients::bigquerydatatransfer::BigquerydatatransferProjectsLocationsTransferConfigsCreateArgs;
use crate::providers::gcp::clients::bigquerydatatransfer::BigquerydatatransferProjectsLocationsTransferConfigsDeleteArgs;
use crate::providers::gcp::clients::bigquerydatatransfer::BigquerydatatransferProjectsLocationsTransferConfigsPatchArgs;
use crate::providers::gcp::clients::bigquerydatatransfer::BigquerydatatransferProjectsLocationsTransferConfigsRunsDeleteArgs;
use crate::providers::gcp::clients::bigquerydatatransfer::BigquerydatatransferProjectsLocationsTransferConfigsScheduleRunsArgs;
use crate::providers::gcp::clients::bigquerydatatransfer::BigquerydatatransferProjectsLocationsTransferConfigsStartManualRunsArgs;
use crate::providers::gcp::clients::bigquerydatatransfer::BigquerydatatransferProjectsLocationsUnenrollDataSourcesArgs;
use crate::providers::gcp::clients::bigquerydatatransfer::BigquerydatatransferProjectsTransferConfigsCreateArgs;
use crate::providers::gcp::clients::bigquerydatatransfer::BigquerydatatransferProjectsTransferConfigsDeleteArgs;
use crate::providers::gcp::clients::bigquerydatatransfer::BigquerydatatransferProjectsTransferConfigsPatchArgs;
use crate::providers::gcp::clients::bigquerydatatransfer::BigquerydatatransferProjectsTransferConfigsRunsDeleteArgs;
use crate::providers::gcp::clients::bigquerydatatransfer::BigquerydatatransferProjectsTransferConfigsScheduleRunsArgs;
use crate::providers::gcp::clients::bigquerydatatransfer::BigquerydatatransferProjectsTransferConfigsStartManualRunsArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// BigquerydatatransferProvider with automatic state tracking.
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
/// let provider = BigquerydatatransferProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct BigquerydatatransferProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> BigquerydatatransferProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new BigquerydatatransferProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Bigquerydatatransfer projects enroll data sources.
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
    pub fn bigquerydatatransfer_projects_enroll_data_sources(
        &self,
        args: &BigquerydatatransferProjectsEnrollDataSourcesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquerydatatransfer_projects_enroll_data_sources_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquerydatatransfer_projects_enroll_data_sources_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquerydatatransfer projects data sources check valid creds.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CheckValidCredsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigquerydatatransfer_projects_data_sources_check_valid_creds(
        &self,
        args: &BigquerydatatransferProjectsDataSourcesCheckValidCredsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CheckValidCredsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquerydatatransfer_projects_data_sources_check_valid_creds_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquerydatatransfer_projects_data_sources_check_valid_creds_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquerydatatransfer projects locations enroll data sources.
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
    pub fn bigquerydatatransfer_projects_locations_enroll_data_sources(
        &self,
        args: &BigquerydatatransferProjectsLocationsEnrollDataSourcesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquerydatatransfer_projects_locations_enroll_data_sources_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquerydatatransfer_projects_locations_enroll_data_sources_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquerydatatransfer projects locations unenroll data sources.
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
    pub fn bigquerydatatransfer_projects_locations_unenroll_data_sources(
        &self,
        args: &BigquerydatatransferProjectsLocationsUnenrollDataSourcesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquerydatatransfer_projects_locations_unenroll_data_sources_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquerydatatransfer_projects_locations_unenroll_data_sources_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquerydatatransfer projects locations data sources check valid creds.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CheckValidCredsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigquerydatatransfer_projects_locations_data_sources_check_valid_creds(
        &self,
        args: &BigquerydatatransferProjectsLocationsDataSourcesCheckValidCredsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CheckValidCredsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquerydatatransfer_projects_locations_data_sources_check_valid_creds_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquerydatatransfer_projects_locations_data_sources_check_valid_creds_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquerydatatransfer projects locations transfer configs create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TransferConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigquerydatatransfer_projects_locations_transfer_configs_create(
        &self,
        args: &BigquerydatatransferProjectsLocationsTransferConfigsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TransferConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquerydatatransfer_projects_locations_transfer_configs_create_builder(
            &self.http_client,
            &args.parent,
            &args.authorizationCode,
            &args.serviceAccountName,
            &args.versionInfo,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquerydatatransfer_projects_locations_transfer_configs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquerydatatransfer projects locations transfer configs delete.
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
    pub fn bigquerydatatransfer_projects_locations_transfer_configs_delete(
        &self,
        args: &BigquerydatatransferProjectsLocationsTransferConfigsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquerydatatransfer_projects_locations_transfer_configs_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquerydatatransfer_projects_locations_transfer_configs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquerydatatransfer projects locations transfer configs patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TransferConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigquerydatatransfer_projects_locations_transfer_configs_patch(
        &self,
        args: &BigquerydatatransferProjectsLocationsTransferConfigsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TransferConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquerydatatransfer_projects_locations_transfer_configs_patch_builder(
            &self.http_client,
            &args.name,
            &args.authorizationCode,
            &args.serviceAccountName,
            &args.updateMask,
            &args.versionInfo,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquerydatatransfer_projects_locations_transfer_configs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquerydatatransfer projects locations transfer configs schedule runs.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ScheduleTransferRunsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigquerydatatransfer_projects_locations_transfer_configs_schedule_runs(
        &self,
        args: &BigquerydatatransferProjectsLocationsTransferConfigsScheduleRunsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ScheduleTransferRunsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquerydatatransfer_projects_locations_transfer_configs_schedule_runs_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquerydatatransfer_projects_locations_transfer_configs_schedule_runs_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquerydatatransfer projects locations transfer configs start manual runs.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the StartManualTransferRunsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigquerydatatransfer_projects_locations_transfer_configs_start_manual_runs(
        &self,
        args: &BigquerydatatransferProjectsLocationsTransferConfigsStartManualRunsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<StartManualTransferRunsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquerydatatransfer_projects_locations_transfer_configs_start_manual_runs_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquerydatatransfer_projects_locations_transfer_configs_start_manual_runs_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquerydatatransfer projects locations transfer configs runs delete.
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
    pub fn bigquerydatatransfer_projects_locations_transfer_configs_runs_delete(
        &self,
        args: &BigquerydatatransferProjectsLocationsTransferConfigsRunsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquerydatatransfer_projects_locations_transfer_configs_runs_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquerydatatransfer_projects_locations_transfer_configs_runs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquerydatatransfer projects transfer configs create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TransferConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigquerydatatransfer_projects_transfer_configs_create(
        &self,
        args: &BigquerydatatransferProjectsTransferConfigsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TransferConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquerydatatransfer_projects_transfer_configs_create_builder(
            &self.http_client,
            &args.parent,
            &args.authorizationCode,
            &args.serviceAccountName,
            &args.versionInfo,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquerydatatransfer_projects_transfer_configs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquerydatatransfer projects transfer configs delete.
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
    pub fn bigquerydatatransfer_projects_transfer_configs_delete(
        &self,
        args: &BigquerydatatransferProjectsTransferConfigsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquerydatatransfer_projects_transfer_configs_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquerydatatransfer_projects_transfer_configs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquerydatatransfer projects transfer configs patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TransferConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigquerydatatransfer_projects_transfer_configs_patch(
        &self,
        args: &BigquerydatatransferProjectsTransferConfigsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TransferConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquerydatatransfer_projects_transfer_configs_patch_builder(
            &self.http_client,
            &args.name,
            &args.authorizationCode,
            &args.serviceAccountName,
            &args.updateMask,
            &args.versionInfo,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquerydatatransfer_projects_transfer_configs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquerydatatransfer projects transfer configs schedule runs.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ScheduleTransferRunsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigquerydatatransfer_projects_transfer_configs_schedule_runs(
        &self,
        args: &BigquerydatatransferProjectsTransferConfigsScheduleRunsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ScheduleTransferRunsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquerydatatransfer_projects_transfer_configs_schedule_runs_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquerydatatransfer_projects_transfer_configs_schedule_runs_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquerydatatransfer projects transfer configs start manual runs.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the StartManualTransferRunsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigquerydatatransfer_projects_transfer_configs_start_manual_runs(
        &self,
        args: &BigquerydatatransferProjectsTransferConfigsStartManualRunsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<StartManualTransferRunsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquerydatatransfer_projects_transfer_configs_start_manual_runs_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquerydatatransfer_projects_transfer_configs_start_manual_runs_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquerydatatransfer projects transfer configs runs delete.
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
    pub fn bigquerydatatransfer_projects_transfer_configs_runs_delete(
        &self,
        args: &BigquerydatatransferProjectsTransferConfigsRunsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquerydatatransfer_projects_transfer_configs_runs_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquerydatatransfer_projects_transfer_configs_runs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
