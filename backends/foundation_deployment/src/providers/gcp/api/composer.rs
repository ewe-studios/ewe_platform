//! ComposerProvider - State-aware composer API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       composer API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::composer::{
    composer_projects_locations_environments_check_upgrade_builder, composer_projects_locations_environments_check_upgrade_task,
    composer_projects_locations_environments_create_builder, composer_projects_locations_environments_create_task,
    composer_projects_locations_environments_database_failover_builder, composer_projects_locations_environments_database_failover_task,
    composer_projects_locations_environments_delete_builder, composer_projects_locations_environments_delete_task,
    composer_projects_locations_environments_execute_airflow_command_builder, composer_projects_locations_environments_execute_airflow_command_task,
    composer_projects_locations_environments_load_snapshot_builder, composer_projects_locations_environments_load_snapshot_task,
    composer_projects_locations_environments_patch_builder, composer_projects_locations_environments_patch_task,
    composer_projects_locations_environments_poll_airflow_command_builder, composer_projects_locations_environments_poll_airflow_command_task,
    composer_projects_locations_environments_restart_web_server_builder, composer_projects_locations_environments_restart_web_server_task,
    composer_projects_locations_environments_save_snapshot_builder, composer_projects_locations_environments_save_snapshot_task,
    composer_projects_locations_environments_stop_airflow_command_builder, composer_projects_locations_environments_stop_airflow_command_task,
    composer_projects_locations_environments_user_workloads_config_maps_create_builder, composer_projects_locations_environments_user_workloads_config_maps_create_task,
    composer_projects_locations_environments_user_workloads_config_maps_delete_builder, composer_projects_locations_environments_user_workloads_config_maps_delete_task,
    composer_projects_locations_environments_user_workloads_config_maps_update_builder, composer_projects_locations_environments_user_workloads_config_maps_update_task,
    composer_projects_locations_environments_user_workloads_secrets_create_builder, composer_projects_locations_environments_user_workloads_secrets_create_task,
    composer_projects_locations_environments_user_workloads_secrets_delete_builder, composer_projects_locations_environments_user_workloads_secrets_delete_task,
    composer_projects_locations_environments_user_workloads_secrets_update_builder, composer_projects_locations_environments_user_workloads_secrets_update_task,
    composer_projects_locations_operations_delete_builder, composer_projects_locations_operations_delete_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::composer::Empty;
use crate::providers::gcp::clients::composer::ExecuteAirflowCommandResponse;
use crate::providers::gcp::clients::composer::Operation;
use crate::providers::gcp::clients::composer::PollAirflowCommandResponse;
use crate::providers::gcp::clients::composer::StopAirflowCommandResponse;
use crate::providers::gcp::clients::composer::UserWorkloadsConfigMap;
use crate::providers::gcp::clients::composer::UserWorkloadsSecret;
use crate::providers::gcp::clients::composer::ComposerProjectsLocationsEnvironmentsCheckUpgradeArgs;
use crate::providers::gcp::clients::composer::ComposerProjectsLocationsEnvironmentsCreateArgs;
use crate::providers::gcp::clients::composer::ComposerProjectsLocationsEnvironmentsDatabaseFailoverArgs;
use crate::providers::gcp::clients::composer::ComposerProjectsLocationsEnvironmentsDeleteArgs;
use crate::providers::gcp::clients::composer::ComposerProjectsLocationsEnvironmentsExecuteAirflowCommandArgs;
use crate::providers::gcp::clients::composer::ComposerProjectsLocationsEnvironmentsLoadSnapshotArgs;
use crate::providers::gcp::clients::composer::ComposerProjectsLocationsEnvironmentsPatchArgs;
use crate::providers::gcp::clients::composer::ComposerProjectsLocationsEnvironmentsPollAirflowCommandArgs;
use crate::providers::gcp::clients::composer::ComposerProjectsLocationsEnvironmentsRestartWebServerArgs;
use crate::providers::gcp::clients::composer::ComposerProjectsLocationsEnvironmentsSaveSnapshotArgs;
use crate::providers::gcp::clients::composer::ComposerProjectsLocationsEnvironmentsStopAirflowCommandArgs;
use crate::providers::gcp::clients::composer::ComposerProjectsLocationsEnvironmentsUserWorkloadsConfigMapsCreateArgs;
use crate::providers::gcp::clients::composer::ComposerProjectsLocationsEnvironmentsUserWorkloadsConfigMapsDeleteArgs;
use crate::providers::gcp::clients::composer::ComposerProjectsLocationsEnvironmentsUserWorkloadsConfigMapsUpdateArgs;
use crate::providers::gcp::clients::composer::ComposerProjectsLocationsEnvironmentsUserWorkloadsSecretsCreateArgs;
use crate::providers::gcp::clients::composer::ComposerProjectsLocationsEnvironmentsUserWorkloadsSecretsDeleteArgs;
use crate::providers::gcp::clients::composer::ComposerProjectsLocationsEnvironmentsUserWorkloadsSecretsUpdateArgs;
use crate::providers::gcp::clients::composer::ComposerProjectsLocationsOperationsDeleteArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// ComposerProvider with automatic state tracking.
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
/// let provider = ComposerProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct ComposerProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> ComposerProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new ComposerProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Composer projects locations environments check upgrade.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn composer_projects_locations_environments_check_upgrade(
        &self,
        args: &ComposerProjectsLocationsEnvironmentsCheckUpgradeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = composer_projects_locations_environments_check_upgrade_builder(
            &self.http_client,
            &args.environment,
        )
        .map_err(ProviderError::Api)?;

        let task = composer_projects_locations_environments_check_upgrade_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Composer projects locations environments create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn composer_projects_locations_environments_create(
        &self,
        args: &ComposerProjectsLocationsEnvironmentsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = composer_projects_locations_environments_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = composer_projects_locations_environments_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Composer projects locations environments database failover.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn composer_projects_locations_environments_database_failover(
        &self,
        args: &ComposerProjectsLocationsEnvironmentsDatabaseFailoverArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = composer_projects_locations_environments_database_failover_builder(
            &self.http_client,
            &args.environment,
        )
        .map_err(ProviderError::Api)?;

        let task = composer_projects_locations_environments_database_failover_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Composer projects locations environments delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn composer_projects_locations_environments_delete(
        &self,
        args: &ComposerProjectsLocationsEnvironmentsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = composer_projects_locations_environments_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = composer_projects_locations_environments_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Composer projects locations environments execute airflow command.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ExecuteAirflowCommandResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn composer_projects_locations_environments_execute_airflow_command(
        &self,
        args: &ComposerProjectsLocationsEnvironmentsExecuteAirflowCommandArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ExecuteAirflowCommandResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = composer_projects_locations_environments_execute_airflow_command_builder(
            &self.http_client,
            &args.environment,
        )
        .map_err(ProviderError::Api)?;

        let task = composer_projects_locations_environments_execute_airflow_command_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Composer projects locations environments load snapshot.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn composer_projects_locations_environments_load_snapshot(
        &self,
        args: &ComposerProjectsLocationsEnvironmentsLoadSnapshotArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = composer_projects_locations_environments_load_snapshot_builder(
            &self.http_client,
            &args.environment,
        )
        .map_err(ProviderError::Api)?;

        let task = composer_projects_locations_environments_load_snapshot_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Composer projects locations environments patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn composer_projects_locations_environments_patch(
        &self,
        args: &ComposerProjectsLocationsEnvironmentsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = composer_projects_locations_environments_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = composer_projects_locations_environments_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Composer projects locations environments poll airflow command.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PollAirflowCommandResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn composer_projects_locations_environments_poll_airflow_command(
        &self,
        args: &ComposerProjectsLocationsEnvironmentsPollAirflowCommandArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PollAirflowCommandResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = composer_projects_locations_environments_poll_airflow_command_builder(
            &self.http_client,
            &args.environment,
        )
        .map_err(ProviderError::Api)?;

        let task = composer_projects_locations_environments_poll_airflow_command_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Composer projects locations environments restart web server.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn composer_projects_locations_environments_restart_web_server(
        &self,
        args: &ComposerProjectsLocationsEnvironmentsRestartWebServerArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = composer_projects_locations_environments_restart_web_server_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = composer_projects_locations_environments_restart_web_server_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Composer projects locations environments save snapshot.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn composer_projects_locations_environments_save_snapshot(
        &self,
        args: &ComposerProjectsLocationsEnvironmentsSaveSnapshotArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = composer_projects_locations_environments_save_snapshot_builder(
            &self.http_client,
            &args.environment,
        )
        .map_err(ProviderError::Api)?;

        let task = composer_projects_locations_environments_save_snapshot_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Composer projects locations environments stop airflow command.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the StopAirflowCommandResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn composer_projects_locations_environments_stop_airflow_command(
        &self,
        args: &ComposerProjectsLocationsEnvironmentsStopAirflowCommandArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<StopAirflowCommandResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = composer_projects_locations_environments_stop_airflow_command_builder(
            &self.http_client,
            &args.environment,
        )
        .map_err(ProviderError::Api)?;

        let task = composer_projects_locations_environments_stop_airflow_command_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Composer projects locations environments user workloads config maps create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UserWorkloadsConfigMap result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn composer_projects_locations_environments_user_workloads_config_maps_create(
        &self,
        args: &ComposerProjectsLocationsEnvironmentsUserWorkloadsConfigMapsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UserWorkloadsConfigMap, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = composer_projects_locations_environments_user_workloads_config_maps_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = composer_projects_locations_environments_user_workloads_config_maps_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Composer projects locations environments user workloads config maps delete.
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
    pub fn composer_projects_locations_environments_user_workloads_config_maps_delete(
        &self,
        args: &ComposerProjectsLocationsEnvironmentsUserWorkloadsConfigMapsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = composer_projects_locations_environments_user_workloads_config_maps_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = composer_projects_locations_environments_user_workloads_config_maps_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Composer projects locations environments user workloads config maps update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UserWorkloadsConfigMap result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn composer_projects_locations_environments_user_workloads_config_maps_update(
        &self,
        args: &ComposerProjectsLocationsEnvironmentsUserWorkloadsConfigMapsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UserWorkloadsConfigMap, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = composer_projects_locations_environments_user_workloads_config_maps_update_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = composer_projects_locations_environments_user_workloads_config_maps_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Composer projects locations environments user workloads secrets create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UserWorkloadsSecret result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn composer_projects_locations_environments_user_workloads_secrets_create(
        &self,
        args: &ComposerProjectsLocationsEnvironmentsUserWorkloadsSecretsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UserWorkloadsSecret, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = composer_projects_locations_environments_user_workloads_secrets_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = composer_projects_locations_environments_user_workloads_secrets_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Composer projects locations environments user workloads secrets delete.
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
    pub fn composer_projects_locations_environments_user_workloads_secrets_delete(
        &self,
        args: &ComposerProjectsLocationsEnvironmentsUserWorkloadsSecretsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = composer_projects_locations_environments_user_workloads_secrets_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = composer_projects_locations_environments_user_workloads_secrets_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Composer projects locations environments user workloads secrets update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UserWorkloadsSecret result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn composer_projects_locations_environments_user_workloads_secrets_update(
        &self,
        args: &ComposerProjectsLocationsEnvironmentsUserWorkloadsSecretsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UserWorkloadsSecret, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = composer_projects_locations_environments_user_workloads_secrets_update_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = composer_projects_locations_environments_user_workloads_secrets_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Composer projects locations operations delete.
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
    pub fn composer_projects_locations_operations_delete(
        &self,
        args: &ComposerProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = composer_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = composer_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
