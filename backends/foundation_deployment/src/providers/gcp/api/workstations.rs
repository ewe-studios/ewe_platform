//! WorkstationsProvider - State-aware workstations API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       workstations API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::workstations::{
    workstations_projects_locations_operations_cancel_builder, workstations_projects_locations_operations_cancel_task,
    workstations_projects_locations_operations_delete_builder, workstations_projects_locations_operations_delete_task,
    workstations_projects_locations_workstation_clusters_create_builder, workstations_projects_locations_workstation_clusters_create_task,
    workstations_projects_locations_workstation_clusters_delete_builder, workstations_projects_locations_workstation_clusters_delete_task,
    workstations_projects_locations_workstation_clusters_patch_builder, workstations_projects_locations_workstation_clusters_patch_task,
    workstations_projects_locations_workstation_clusters_workstation_configs_create_builder, workstations_projects_locations_workstation_clusters_workstation_configs_create_task,
    workstations_projects_locations_workstation_clusters_workstation_configs_delete_builder, workstations_projects_locations_workstation_clusters_workstation_configs_delete_task,
    workstations_projects_locations_workstation_clusters_workstation_configs_patch_builder, workstations_projects_locations_workstation_clusters_workstation_configs_patch_task,
    workstations_projects_locations_workstation_clusters_workstation_configs_set_iam_policy_builder, workstations_projects_locations_workstation_clusters_workstation_configs_set_iam_policy_task,
    workstations_projects_locations_workstation_clusters_workstation_configs_test_iam_permissions_builder, workstations_projects_locations_workstation_clusters_workstation_configs_test_iam_permissions_task,
    workstations_projects_locations_workstation_clusters_workstation_configs_workstations_create_builder, workstations_projects_locations_workstation_clusters_workstation_configs_workstations_create_task,
    workstations_projects_locations_workstation_clusters_workstation_configs_workstations_delete_builder, workstations_projects_locations_workstation_clusters_workstation_configs_workstations_delete_task,
    workstations_projects_locations_workstation_clusters_workstation_configs_workstations_generate_access_token_builder, workstations_projects_locations_workstation_clusters_workstation_configs_workstations_generate_access_token_task,
    workstations_projects_locations_workstation_clusters_workstation_configs_workstations_patch_builder, workstations_projects_locations_workstation_clusters_workstation_configs_workstations_patch_task,
    workstations_projects_locations_workstation_clusters_workstation_configs_workstations_set_iam_policy_builder, workstations_projects_locations_workstation_clusters_workstation_configs_workstations_set_iam_policy_task,
    workstations_projects_locations_workstation_clusters_workstation_configs_workstations_start_builder, workstations_projects_locations_workstation_clusters_workstation_configs_workstations_start_task,
    workstations_projects_locations_workstation_clusters_workstation_configs_workstations_stop_builder, workstations_projects_locations_workstation_clusters_workstation_configs_workstations_stop_task,
    workstations_projects_locations_workstation_clusters_workstation_configs_workstations_test_iam_permissions_builder, workstations_projects_locations_workstation_clusters_workstation_configs_workstations_test_iam_permissions_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::workstations::GenerateAccessTokenResponse;
use crate::providers::gcp::clients::workstations::GoogleProtobufEmpty;
use crate::providers::gcp::clients::workstations::Operation;
use crate::providers::gcp::clients::workstations::Policy;
use crate::providers::gcp::clients::workstations::TestIamPermissionsResponse;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsWorkstationClustersCreateArgs;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsWorkstationClustersDeleteArgs;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsWorkstationClustersPatchArgs;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsCreateArgs;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsDeleteArgs;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsPatchArgs;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsSetIamPolicyArgs;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsTestIamPermissionsArgs;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsWorkstationsCreateArgs;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsWorkstationsDeleteArgs;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsWorkstationsGenerateAccessTokenArgs;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsWorkstationsPatchArgs;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsWorkstationsSetIamPolicyArgs;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsWorkstationsStartArgs;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsWorkstationsStopArgs;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsWorkstationsTestIamPermissionsArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// WorkstationsProvider with automatic state tracking.
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
/// let provider = WorkstationsProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct WorkstationsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> WorkstationsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new WorkstationsProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Workstations projects locations operations cancel.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn workstations_projects_locations_operations_cancel(
        &self,
        args: &WorkstationsProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workstations_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = workstations_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workstations projects locations operations delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn workstations_projects_locations_operations_delete(
        &self,
        args: &WorkstationsProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workstations_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = workstations_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workstations projects locations workstation clusters create.
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
    pub fn workstations_projects_locations_workstation_clusters_create(
        &self,
        args: &WorkstationsProjectsLocationsWorkstationClustersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workstations_projects_locations_workstation_clusters_create_builder(
            &self.http_client,
            &args.parent,
            &args.validateOnly,
            &args.workstationClusterId,
        )
        .map_err(ProviderError::Api)?;

        let task = workstations_projects_locations_workstation_clusters_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workstations projects locations workstation clusters delete.
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
    pub fn workstations_projects_locations_workstation_clusters_delete(
        &self,
        args: &WorkstationsProjectsLocationsWorkstationClustersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workstations_projects_locations_workstation_clusters_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.force,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = workstations_projects_locations_workstation_clusters_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workstations projects locations workstation clusters patch.
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
    pub fn workstations_projects_locations_workstation_clusters_patch(
        &self,
        args: &WorkstationsProjectsLocationsWorkstationClustersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workstations_projects_locations_workstation_clusters_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = workstations_projects_locations_workstation_clusters_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workstations projects locations workstation clusters workstation configs create.
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
    pub fn workstations_projects_locations_workstation_clusters_workstation_configs_create(
        &self,
        args: &WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workstations_projects_locations_workstation_clusters_workstation_configs_create_builder(
            &self.http_client,
            &args.parent,
            &args.validateOnly,
            &args.workstationConfigId,
        )
        .map_err(ProviderError::Api)?;

        let task = workstations_projects_locations_workstation_clusters_workstation_configs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workstations projects locations workstation clusters workstation configs delete.
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
    pub fn workstations_projects_locations_workstation_clusters_workstation_configs_delete(
        &self,
        args: &WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workstations_projects_locations_workstation_clusters_workstation_configs_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.force,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = workstations_projects_locations_workstation_clusters_workstation_configs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workstations projects locations workstation clusters workstation configs patch.
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
    pub fn workstations_projects_locations_workstation_clusters_workstation_configs_patch(
        &self,
        args: &WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workstations_projects_locations_workstation_clusters_workstation_configs_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = workstations_projects_locations_workstation_clusters_workstation_configs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workstations projects locations workstation clusters workstation configs set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn workstations_projects_locations_workstation_clusters_workstation_configs_set_iam_policy(
        &self,
        args: &WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workstations_projects_locations_workstation_clusters_workstation_configs_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = workstations_projects_locations_workstation_clusters_workstation_configs_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workstations projects locations workstation clusters workstation configs test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn workstations_projects_locations_workstation_clusters_workstation_configs_test_iam_permissions(
        &self,
        args: &WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workstations_projects_locations_workstation_clusters_workstation_configs_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = workstations_projects_locations_workstation_clusters_workstation_configs_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workstations projects locations workstation clusters workstation configs workstations create.
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
    pub fn workstations_projects_locations_workstation_clusters_workstation_configs_workstations_create(
        &self,
        args: &WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsWorkstationsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workstations_projects_locations_workstation_clusters_workstation_configs_workstations_create_builder(
            &self.http_client,
            &args.parent,
            &args.validateOnly,
            &args.workstationId,
        )
        .map_err(ProviderError::Api)?;

        let task = workstations_projects_locations_workstation_clusters_workstation_configs_workstations_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workstations projects locations workstation clusters workstation configs workstations delete.
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
    pub fn workstations_projects_locations_workstation_clusters_workstation_configs_workstations_delete(
        &self,
        args: &WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsWorkstationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workstations_projects_locations_workstation_clusters_workstation_configs_workstations_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = workstations_projects_locations_workstation_clusters_workstation_configs_workstations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workstations projects locations workstation clusters workstation configs workstations generate access token.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GenerateAccessTokenResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn workstations_projects_locations_workstation_clusters_workstation_configs_workstations_generate_access_token(
        &self,
        args: &WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsWorkstationsGenerateAccessTokenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GenerateAccessTokenResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workstations_projects_locations_workstation_clusters_workstation_configs_workstations_generate_access_token_builder(
            &self.http_client,
            &args.workstation,
        )
        .map_err(ProviderError::Api)?;

        let task = workstations_projects_locations_workstation_clusters_workstation_configs_workstations_generate_access_token_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workstations projects locations workstation clusters workstation configs workstations patch.
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
    pub fn workstations_projects_locations_workstation_clusters_workstation_configs_workstations_patch(
        &self,
        args: &WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsWorkstationsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workstations_projects_locations_workstation_clusters_workstation_configs_workstations_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = workstations_projects_locations_workstation_clusters_workstation_configs_workstations_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workstations projects locations workstation clusters workstation configs workstations set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn workstations_projects_locations_workstation_clusters_workstation_configs_workstations_set_iam_policy(
        &self,
        args: &WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsWorkstationsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workstations_projects_locations_workstation_clusters_workstation_configs_workstations_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = workstations_projects_locations_workstation_clusters_workstation_configs_workstations_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workstations projects locations workstation clusters workstation configs workstations start.
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
    pub fn workstations_projects_locations_workstation_clusters_workstation_configs_workstations_start(
        &self,
        args: &WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsWorkstationsStartArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workstations_projects_locations_workstation_clusters_workstation_configs_workstations_start_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = workstations_projects_locations_workstation_clusters_workstation_configs_workstations_start_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workstations projects locations workstation clusters workstation configs workstations stop.
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
    pub fn workstations_projects_locations_workstation_clusters_workstation_configs_workstations_stop(
        &self,
        args: &WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsWorkstationsStopArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workstations_projects_locations_workstation_clusters_workstation_configs_workstations_stop_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = workstations_projects_locations_workstation_clusters_workstation_configs_workstations_stop_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workstations projects locations workstation clusters workstation configs workstations test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn workstations_projects_locations_workstation_clusters_workstation_configs_workstations_test_iam_permissions(
        &self,
        args: &WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsWorkstationsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workstations_projects_locations_workstation_clusters_workstation_configs_workstations_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = workstations_projects_locations_workstation_clusters_workstation_configs_workstations_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
