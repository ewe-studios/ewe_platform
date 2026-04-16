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
    workstations_projects_locations_get_builder, workstations_projects_locations_get_task,
    workstations_projects_locations_list_builder, workstations_projects_locations_list_task,
    workstations_projects_locations_operations_cancel_builder, workstations_projects_locations_operations_cancel_task,
    workstations_projects_locations_operations_delete_builder, workstations_projects_locations_operations_delete_task,
    workstations_projects_locations_operations_get_builder, workstations_projects_locations_operations_get_task,
    workstations_projects_locations_operations_list_builder, workstations_projects_locations_operations_list_task,
    workstations_projects_locations_workstation_clusters_create_builder, workstations_projects_locations_workstation_clusters_create_task,
    workstations_projects_locations_workstation_clusters_delete_builder, workstations_projects_locations_workstation_clusters_delete_task,
    workstations_projects_locations_workstation_clusters_get_builder, workstations_projects_locations_workstation_clusters_get_task,
    workstations_projects_locations_workstation_clusters_list_builder, workstations_projects_locations_workstation_clusters_list_task,
    workstations_projects_locations_workstation_clusters_patch_builder, workstations_projects_locations_workstation_clusters_patch_task,
    workstations_projects_locations_workstation_clusters_workstation_configs_create_builder, workstations_projects_locations_workstation_clusters_workstation_configs_create_task,
    workstations_projects_locations_workstation_clusters_workstation_configs_delete_builder, workstations_projects_locations_workstation_clusters_workstation_configs_delete_task,
    workstations_projects_locations_workstation_clusters_workstation_configs_get_builder, workstations_projects_locations_workstation_clusters_workstation_configs_get_task,
    workstations_projects_locations_workstation_clusters_workstation_configs_get_iam_policy_builder, workstations_projects_locations_workstation_clusters_workstation_configs_get_iam_policy_task,
    workstations_projects_locations_workstation_clusters_workstation_configs_list_builder, workstations_projects_locations_workstation_clusters_workstation_configs_list_task,
    workstations_projects_locations_workstation_clusters_workstation_configs_list_usable_builder, workstations_projects_locations_workstation_clusters_workstation_configs_list_usable_task,
    workstations_projects_locations_workstation_clusters_workstation_configs_patch_builder, workstations_projects_locations_workstation_clusters_workstation_configs_patch_task,
    workstations_projects_locations_workstation_clusters_workstation_configs_set_iam_policy_builder, workstations_projects_locations_workstation_clusters_workstation_configs_set_iam_policy_task,
    workstations_projects_locations_workstation_clusters_workstation_configs_test_iam_permissions_builder, workstations_projects_locations_workstation_clusters_workstation_configs_test_iam_permissions_task,
    workstations_projects_locations_workstation_clusters_workstation_configs_workstations_create_builder, workstations_projects_locations_workstation_clusters_workstation_configs_workstations_create_task,
    workstations_projects_locations_workstation_clusters_workstation_configs_workstations_delete_builder, workstations_projects_locations_workstation_clusters_workstation_configs_workstations_delete_task,
    workstations_projects_locations_workstation_clusters_workstation_configs_workstations_generate_access_token_builder, workstations_projects_locations_workstation_clusters_workstation_configs_workstations_generate_access_token_task,
    workstations_projects_locations_workstation_clusters_workstation_configs_workstations_get_builder, workstations_projects_locations_workstation_clusters_workstation_configs_workstations_get_task,
    workstations_projects_locations_workstation_clusters_workstation_configs_workstations_get_iam_policy_builder, workstations_projects_locations_workstation_clusters_workstation_configs_workstations_get_iam_policy_task,
    workstations_projects_locations_workstation_clusters_workstation_configs_workstations_list_builder, workstations_projects_locations_workstation_clusters_workstation_configs_workstations_list_task,
    workstations_projects_locations_workstation_clusters_workstation_configs_workstations_list_usable_builder, workstations_projects_locations_workstation_clusters_workstation_configs_workstations_list_usable_task,
    workstations_projects_locations_workstation_clusters_workstation_configs_workstations_patch_builder, workstations_projects_locations_workstation_clusters_workstation_configs_workstations_patch_task,
    workstations_projects_locations_workstation_clusters_workstation_configs_workstations_set_iam_policy_builder, workstations_projects_locations_workstation_clusters_workstation_configs_workstations_set_iam_policy_task,
    workstations_projects_locations_workstation_clusters_workstation_configs_workstations_start_builder, workstations_projects_locations_workstation_clusters_workstation_configs_workstations_start_task,
    workstations_projects_locations_workstation_clusters_workstation_configs_workstations_stop_builder, workstations_projects_locations_workstation_clusters_workstation_configs_workstations_stop_task,
    workstations_projects_locations_workstation_clusters_workstation_configs_workstations_test_iam_permissions_builder, workstations_projects_locations_workstation_clusters_workstation_configs_workstations_test_iam_permissions_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::workstations::GenerateAccessTokenResponse;
use crate::providers::gcp::clients::workstations::GoogleProtobufEmpty;
use crate::providers::gcp::clients::workstations::ListLocationsResponse;
use crate::providers::gcp::clients::workstations::ListOperationsResponse;
use crate::providers::gcp::clients::workstations::ListUsableWorkstationConfigsResponse;
use crate::providers::gcp::clients::workstations::ListUsableWorkstationsResponse;
use crate::providers::gcp::clients::workstations::ListWorkstationClustersResponse;
use crate::providers::gcp::clients::workstations::ListWorkstationConfigsResponse;
use crate::providers::gcp::clients::workstations::ListWorkstationsResponse;
use crate::providers::gcp::clients::workstations::Location;
use crate::providers::gcp::clients::workstations::Operation;
use crate::providers::gcp::clients::workstations::Policy;
use crate::providers::gcp::clients::workstations::TestIamPermissionsResponse;
use crate::providers::gcp::clients::workstations::Workstation;
use crate::providers::gcp::clients::workstations::WorkstationCluster;
use crate::providers::gcp::clients::workstations::WorkstationConfig;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsGetArgs;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsListArgs;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsOperationsListArgs;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsWorkstationClustersCreateArgs;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsWorkstationClustersDeleteArgs;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsWorkstationClustersGetArgs;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsWorkstationClustersListArgs;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsWorkstationClustersPatchArgs;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsCreateArgs;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsDeleteArgs;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsGetArgs;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsGetIamPolicyArgs;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsListArgs;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsListUsableArgs;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsPatchArgs;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsSetIamPolicyArgs;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsTestIamPermissionsArgs;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsWorkstationsCreateArgs;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsWorkstationsDeleteArgs;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsWorkstationsGenerateAccessTokenArgs;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsWorkstationsGetArgs;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsWorkstationsGetIamPolicyArgs;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsWorkstationsListArgs;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsWorkstationsListUsableArgs;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsWorkstationsPatchArgs;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsWorkstationsSetIamPolicyArgs;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsWorkstationsStartArgs;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsWorkstationsStopArgs;
use crate::providers::gcp::clients::workstations::WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsWorkstationsTestIamPermissionsArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// WorkstationsProvider with automatic state tracking.
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
/// let provider = WorkstationsProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct WorkstationsProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> WorkstationsProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new WorkstationsProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new WorkstationsProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Workstations projects locations get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn workstations_projects_locations_get(
        &self,
        args: &WorkstationsProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workstations_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = workstations_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workstations projects locations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListLocationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn workstations_projects_locations_list(
        &self,
        args: &WorkstationsProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workstations_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = workstations_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Workstations projects locations operations get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn workstations_projects_locations_operations_get(
        &self,
        args: &WorkstationsProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workstations_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = workstations_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workstations projects locations operations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListOperationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn workstations_projects_locations_operations_list(
        &self,
        args: &WorkstationsProjectsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workstations_projects_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = workstations_projects_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Workstations projects locations workstation clusters get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WorkstationCluster result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn workstations_projects_locations_workstation_clusters_get(
        &self,
        args: &WorkstationsProjectsLocationsWorkstationClustersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WorkstationCluster, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workstations_projects_locations_workstation_clusters_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = workstations_projects_locations_workstation_clusters_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workstations projects locations workstation clusters list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListWorkstationClustersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn workstations_projects_locations_workstation_clusters_list(
        &self,
        args: &WorkstationsProjectsLocationsWorkstationClustersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListWorkstationClustersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workstations_projects_locations_workstation_clusters_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = workstations_projects_locations_workstation_clusters_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Workstations projects locations workstation clusters workstation configs get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WorkstationConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn workstations_projects_locations_workstation_clusters_workstation_configs_get(
        &self,
        args: &WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WorkstationConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workstations_projects_locations_workstation_clusters_workstation_configs_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = workstations_projects_locations_workstation_clusters_workstation_configs_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workstations projects locations workstation clusters workstation configs get iam policy.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn workstations_projects_locations_workstation_clusters_workstation_configs_get_iam_policy(
        &self,
        args: &WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workstations_projects_locations_workstation_clusters_workstation_configs_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options_requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = workstations_projects_locations_workstation_clusters_workstation_configs_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workstations projects locations workstation clusters workstation configs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListWorkstationConfigsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn workstations_projects_locations_workstation_clusters_workstation_configs_list(
        &self,
        args: &WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListWorkstationConfigsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workstations_projects_locations_workstation_clusters_workstation_configs_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = workstations_projects_locations_workstation_clusters_workstation_configs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workstations projects locations workstation clusters workstation configs list usable.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListUsableWorkstationConfigsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn workstations_projects_locations_workstation_clusters_workstation_configs_list_usable(
        &self,
        args: &WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsListUsableArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListUsableWorkstationConfigsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workstations_projects_locations_workstation_clusters_workstation_configs_list_usable_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = workstations_projects_locations_workstation_clusters_workstation_configs_list_usable_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Workstations projects locations workstation clusters workstation configs workstations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Workstation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn workstations_projects_locations_workstation_clusters_workstation_configs_workstations_get(
        &self,
        args: &WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsWorkstationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Workstation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workstations_projects_locations_workstation_clusters_workstation_configs_workstations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = workstations_projects_locations_workstation_clusters_workstation_configs_workstations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workstations projects locations workstation clusters workstation configs workstations get iam policy.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn workstations_projects_locations_workstation_clusters_workstation_configs_workstations_get_iam_policy(
        &self,
        args: &WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsWorkstationsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workstations_projects_locations_workstation_clusters_workstation_configs_workstations_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options_requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = workstations_projects_locations_workstation_clusters_workstation_configs_workstations_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workstations projects locations workstation clusters workstation configs workstations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListWorkstationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn workstations_projects_locations_workstation_clusters_workstation_configs_workstations_list(
        &self,
        args: &WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsWorkstationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListWorkstationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workstations_projects_locations_workstation_clusters_workstation_configs_workstations_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = workstations_projects_locations_workstation_clusters_workstation_configs_workstations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workstations projects locations workstation clusters workstation configs workstations list usable.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListUsableWorkstationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn workstations_projects_locations_workstation_clusters_workstation_configs_workstations_list_usable(
        &self,
        args: &WorkstationsProjectsLocationsWorkstationClustersWorkstationConfigsWorkstationsListUsableArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListUsableWorkstationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workstations_projects_locations_workstation_clusters_workstation_configs_workstations_list_usable_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = workstations_projects_locations_workstation_clusters_workstation_configs_workstations_list_usable_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
