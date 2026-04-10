//! DeploymentmanagerProvider - State-aware deploymentmanager API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       deploymentmanager API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::deploymentmanager::{
    deploymentmanager_deployments_cancel_preview_builder, deploymentmanager_deployments_cancel_preview_task,
    deploymentmanager_deployments_delete_builder, deploymentmanager_deployments_delete_task,
    deploymentmanager_deployments_get_builder, deploymentmanager_deployments_get_task,
    deploymentmanager_deployments_get_iam_policy_builder, deploymentmanager_deployments_get_iam_policy_task,
    deploymentmanager_deployments_insert_builder, deploymentmanager_deployments_insert_task,
    deploymentmanager_deployments_list_builder, deploymentmanager_deployments_list_task,
    deploymentmanager_deployments_patch_builder, deploymentmanager_deployments_patch_task,
    deploymentmanager_deployments_set_iam_policy_builder, deploymentmanager_deployments_set_iam_policy_task,
    deploymentmanager_deployments_stop_builder, deploymentmanager_deployments_stop_task,
    deploymentmanager_deployments_test_iam_permissions_builder, deploymentmanager_deployments_test_iam_permissions_task,
    deploymentmanager_deployments_update_builder, deploymentmanager_deployments_update_task,
    deploymentmanager_manifests_get_builder, deploymentmanager_manifests_get_task,
    deploymentmanager_manifests_list_builder, deploymentmanager_manifests_list_task,
    deploymentmanager_operations_get_builder, deploymentmanager_operations_get_task,
    deploymentmanager_operations_list_builder, deploymentmanager_operations_list_task,
    deploymentmanager_resources_get_builder, deploymentmanager_resources_get_task,
    deploymentmanager_resources_list_builder, deploymentmanager_resources_list_task,
    deploymentmanager_types_list_builder, deploymentmanager_types_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::deploymentmanager::Deployment;
use crate::providers::gcp::clients::deploymentmanager::DeploymentsListResponse;
use crate::providers::gcp::clients::deploymentmanager::Manifest;
use crate::providers::gcp::clients::deploymentmanager::ManifestsListResponse;
use crate::providers::gcp::clients::deploymentmanager::Operation;
use crate::providers::gcp::clients::deploymentmanager::OperationsListResponse;
use crate::providers::gcp::clients::deploymentmanager::Policy;
use crate::providers::gcp::clients::deploymentmanager::Resource;
use crate::providers::gcp::clients::deploymentmanager::ResourcesListResponse;
use crate::providers::gcp::clients::deploymentmanager::TestPermissionsResponse;
use crate::providers::gcp::clients::deploymentmanager::TypesListResponse;
use crate::providers::gcp::clients::deploymentmanager::DeploymentmanagerDeploymentsCancelPreviewArgs;
use crate::providers::gcp::clients::deploymentmanager::DeploymentmanagerDeploymentsDeleteArgs;
use crate::providers::gcp::clients::deploymentmanager::DeploymentmanagerDeploymentsGetArgs;
use crate::providers::gcp::clients::deploymentmanager::DeploymentmanagerDeploymentsGetIamPolicyArgs;
use crate::providers::gcp::clients::deploymentmanager::DeploymentmanagerDeploymentsInsertArgs;
use crate::providers::gcp::clients::deploymentmanager::DeploymentmanagerDeploymentsListArgs;
use crate::providers::gcp::clients::deploymentmanager::DeploymentmanagerDeploymentsPatchArgs;
use crate::providers::gcp::clients::deploymentmanager::DeploymentmanagerDeploymentsSetIamPolicyArgs;
use crate::providers::gcp::clients::deploymentmanager::DeploymentmanagerDeploymentsStopArgs;
use crate::providers::gcp::clients::deploymentmanager::DeploymentmanagerDeploymentsTestIamPermissionsArgs;
use crate::providers::gcp::clients::deploymentmanager::DeploymentmanagerDeploymentsUpdateArgs;
use crate::providers::gcp::clients::deploymentmanager::DeploymentmanagerManifestsGetArgs;
use crate::providers::gcp::clients::deploymentmanager::DeploymentmanagerManifestsListArgs;
use crate::providers::gcp::clients::deploymentmanager::DeploymentmanagerOperationsGetArgs;
use crate::providers::gcp::clients::deploymentmanager::DeploymentmanagerOperationsListArgs;
use crate::providers::gcp::clients::deploymentmanager::DeploymentmanagerResourcesGetArgs;
use crate::providers::gcp::clients::deploymentmanager::DeploymentmanagerResourcesListArgs;
use crate::providers::gcp::clients::deploymentmanager::DeploymentmanagerTypesListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// DeploymentmanagerProvider with automatic state tracking.
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
/// let provider = DeploymentmanagerProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct DeploymentmanagerProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> DeploymentmanagerProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new DeploymentmanagerProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Deploymentmanager deployments cancel preview.
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
    pub fn deploymentmanager_deployments_cancel_preview(
        &self,
        args: &DeploymentmanagerDeploymentsCancelPreviewArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = deploymentmanager_deployments_cancel_preview_builder(
            &self.http_client,
            &args.project,
            &args.deployment,
        )
        .map_err(ProviderError::Api)?;

        let task = deploymentmanager_deployments_cancel_preview_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Deploymentmanager deployments delete.
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
    pub fn deploymentmanager_deployments_delete(
        &self,
        args: &DeploymentmanagerDeploymentsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = deploymentmanager_deployments_delete_builder(
            &self.http_client,
            &args.project,
            &args.deployment,
            &args.deletePolicy,
            &args.header.bypassBillingFilter,
        )
        .map_err(ProviderError::Api)?;

        let task = deploymentmanager_deployments_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Deploymentmanager deployments get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Deployment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn deploymentmanager_deployments_get(
        &self,
        args: &DeploymentmanagerDeploymentsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Deployment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = deploymentmanager_deployments_get_builder(
            &self.http_client,
            &args.project,
            &args.deployment,
            &args.header.bypassBillingFilter,
        )
        .map_err(ProviderError::Api)?;

        let task = deploymentmanager_deployments_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Deploymentmanager deployments get iam policy.
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
    pub fn deploymentmanager_deployments_get_iam_policy(
        &self,
        args: &DeploymentmanagerDeploymentsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = deploymentmanager_deployments_get_iam_policy_builder(
            &self.http_client,
            &args.project,
            &args.resource,
            &args.header.bypassBillingFilter,
            &args.optionsRequestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = deploymentmanager_deployments_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Deploymentmanager deployments insert.
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
    pub fn deploymentmanager_deployments_insert(
        &self,
        args: &DeploymentmanagerDeploymentsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = deploymentmanager_deployments_insert_builder(
            &self.http_client,
            &args.project,
            &args.createPolicy,
            &args.header.bypassBillingFilter,
            &args.preview,
        )
        .map_err(ProviderError::Api)?;

        let task = deploymentmanager_deployments_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Deploymentmanager deployments list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeploymentsListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn deploymentmanager_deployments_list(
        &self,
        args: &DeploymentmanagerDeploymentsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeploymentsListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = deploymentmanager_deployments_list_builder(
            &self.http_client,
            &args.project,
            &args.filter,
            &args.maxResults,
            &args.orderBy,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = deploymentmanager_deployments_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Deploymentmanager deployments patch.
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
    pub fn deploymentmanager_deployments_patch(
        &self,
        args: &DeploymentmanagerDeploymentsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = deploymentmanager_deployments_patch_builder(
            &self.http_client,
            &args.project,
            &args.deployment,
            &args.createPolicy,
            &args.deletePolicy,
            &args.header.bypassBillingFilter,
            &args.preview,
        )
        .map_err(ProviderError::Api)?;

        let task = deploymentmanager_deployments_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Deploymentmanager deployments set iam policy.
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
    pub fn deploymentmanager_deployments_set_iam_policy(
        &self,
        args: &DeploymentmanagerDeploymentsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = deploymentmanager_deployments_set_iam_policy_builder(
            &self.http_client,
            &args.project,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = deploymentmanager_deployments_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Deploymentmanager deployments stop.
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
    pub fn deploymentmanager_deployments_stop(
        &self,
        args: &DeploymentmanagerDeploymentsStopArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = deploymentmanager_deployments_stop_builder(
            &self.http_client,
            &args.project,
            &args.deployment,
        )
        .map_err(ProviderError::Api)?;

        let task = deploymentmanager_deployments_stop_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Deploymentmanager deployments test iam permissions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TestPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn deploymentmanager_deployments_test_iam_permissions(
        &self,
        args: &DeploymentmanagerDeploymentsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = deploymentmanager_deployments_test_iam_permissions_builder(
            &self.http_client,
            &args.project,
            &args.resource,
            &args.header.bypassBillingFilter,
        )
        .map_err(ProviderError::Api)?;

        let task = deploymentmanager_deployments_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Deploymentmanager deployments update.
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
    pub fn deploymentmanager_deployments_update(
        &self,
        args: &DeploymentmanagerDeploymentsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = deploymentmanager_deployments_update_builder(
            &self.http_client,
            &args.project,
            &args.deployment,
            &args.createPolicy,
            &args.deletePolicy,
            &args.header.bypassBillingFilter,
            &args.preview,
        )
        .map_err(ProviderError::Api)?;

        let task = deploymentmanager_deployments_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Deploymentmanager manifests get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Manifest result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn deploymentmanager_manifests_get(
        &self,
        args: &DeploymentmanagerManifestsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Manifest, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = deploymentmanager_manifests_get_builder(
            &self.http_client,
            &args.project,
            &args.deployment,
            &args.manifest,
            &args.header.bypassBillingFilter,
        )
        .map_err(ProviderError::Api)?;

        let task = deploymentmanager_manifests_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Deploymentmanager manifests list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ManifestsListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn deploymentmanager_manifests_list(
        &self,
        args: &DeploymentmanagerManifestsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ManifestsListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = deploymentmanager_manifests_list_builder(
            &self.http_client,
            &args.project,
            &args.deployment,
            &args.filter,
            &args.maxResults,
            &args.orderBy,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = deploymentmanager_manifests_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Deploymentmanager operations get.
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
    pub fn deploymentmanager_operations_get(
        &self,
        args: &DeploymentmanagerOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = deploymentmanager_operations_get_builder(
            &self.http_client,
            &args.project,
            &args.operation,
            &args.header.bypassBillingFilter,
        )
        .map_err(ProviderError::Api)?;

        let task = deploymentmanager_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Deploymentmanager operations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the OperationsListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn deploymentmanager_operations_list(
        &self,
        args: &DeploymentmanagerOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<OperationsListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = deploymentmanager_operations_list_builder(
            &self.http_client,
            &args.project,
            &args.filter,
            &args.maxResults,
            &args.orderBy,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = deploymentmanager_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Deploymentmanager resources get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Resource result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn deploymentmanager_resources_get(
        &self,
        args: &DeploymentmanagerResourcesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Resource, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = deploymentmanager_resources_get_builder(
            &self.http_client,
            &args.project,
            &args.deployment,
            &args.resource,
            &args.header.bypassBillingFilter,
        )
        .map_err(ProviderError::Api)?;

        let task = deploymentmanager_resources_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Deploymentmanager resources list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ResourcesListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn deploymentmanager_resources_list(
        &self,
        args: &DeploymentmanagerResourcesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ResourcesListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = deploymentmanager_resources_list_builder(
            &self.http_client,
            &args.project,
            &args.deployment,
            &args.filter,
            &args.maxResults,
            &args.orderBy,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = deploymentmanager_resources_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Deploymentmanager types list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TypesListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn deploymentmanager_types_list(
        &self,
        args: &DeploymentmanagerTypesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TypesListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = deploymentmanager_types_list_builder(
            &self.http_client,
            &args.project,
            &args.filter,
            &args.maxResults,
            &args.orderBy,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = deploymentmanager_types_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
