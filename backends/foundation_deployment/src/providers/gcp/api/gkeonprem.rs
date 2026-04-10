//! GkeonpremProvider - State-aware gkeonprem API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       gkeonprem API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::gkeonprem::{
    gkeonprem_projects_locations_bare_metal_admin_clusters_create_builder, gkeonprem_projects_locations_bare_metal_admin_clusters_create_task,
    gkeonprem_projects_locations_bare_metal_admin_clusters_enroll_builder, gkeonprem_projects_locations_bare_metal_admin_clusters_enroll_task,
    gkeonprem_projects_locations_bare_metal_admin_clusters_patch_builder, gkeonprem_projects_locations_bare_metal_admin_clusters_patch_task,
    gkeonprem_projects_locations_bare_metal_admin_clusters_query_version_config_builder, gkeonprem_projects_locations_bare_metal_admin_clusters_query_version_config_task,
    gkeonprem_projects_locations_bare_metal_admin_clusters_set_iam_policy_builder, gkeonprem_projects_locations_bare_metal_admin_clusters_set_iam_policy_task,
    gkeonprem_projects_locations_bare_metal_admin_clusters_test_iam_permissions_builder, gkeonprem_projects_locations_bare_metal_admin_clusters_test_iam_permissions_task,
    gkeonprem_projects_locations_bare_metal_admin_clusters_unenroll_builder, gkeonprem_projects_locations_bare_metal_admin_clusters_unenroll_task,
    gkeonprem_projects_locations_bare_metal_clusters_create_builder, gkeonprem_projects_locations_bare_metal_clusters_create_task,
    gkeonprem_projects_locations_bare_metal_clusters_delete_builder, gkeonprem_projects_locations_bare_metal_clusters_delete_task,
    gkeonprem_projects_locations_bare_metal_clusters_enroll_builder, gkeonprem_projects_locations_bare_metal_clusters_enroll_task,
    gkeonprem_projects_locations_bare_metal_clusters_patch_builder, gkeonprem_projects_locations_bare_metal_clusters_patch_task,
    gkeonprem_projects_locations_bare_metal_clusters_query_version_config_builder, gkeonprem_projects_locations_bare_metal_clusters_query_version_config_task,
    gkeonprem_projects_locations_bare_metal_clusters_set_iam_policy_builder, gkeonprem_projects_locations_bare_metal_clusters_set_iam_policy_task,
    gkeonprem_projects_locations_bare_metal_clusters_test_iam_permissions_builder, gkeonprem_projects_locations_bare_metal_clusters_test_iam_permissions_task,
    gkeonprem_projects_locations_bare_metal_clusters_unenroll_builder, gkeonprem_projects_locations_bare_metal_clusters_unenroll_task,
    gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_create_builder, gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_create_task,
    gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_delete_builder, gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_delete_task,
    gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_enroll_builder, gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_enroll_task,
    gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_patch_builder, gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_patch_task,
    gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_set_iam_policy_builder, gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_set_iam_policy_task,
    gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_test_iam_permissions_builder, gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_test_iam_permissions_task,
    gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_unenroll_builder, gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_unenroll_task,
    gkeonprem_projects_locations_operations_cancel_builder, gkeonprem_projects_locations_operations_cancel_task,
    gkeonprem_projects_locations_operations_delete_builder, gkeonprem_projects_locations_operations_delete_task,
    gkeonprem_projects_locations_vmware_admin_clusters_create_builder, gkeonprem_projects_locations_vmware_admin_clusters_create_task,
    gkeonprem_projects_locations_vmware_admin_clusters_enroll_builder, gkeonprem_projects_locations_vmware_admin_clusters_enroll_task,
    gkeonprem_projects_locations_vmware_admin_clusters_patch_builder, gkeonprem_projects_locations_vmware_admin_clusters_patch_task,
    gkeonprem_projects_locations_vmware_admin_clusters_set_iam_policy_builder, gkeonprem_projects_locations_vmware_admin_clusters_set_iam_policy_task,
    gkeonprem_projects_locations_vmware_admin_clusters_test_iam_permissions_builder, gkeonprem_projects_locations_vmware_admin_clusters_test_iam_permissions_task,
    gkeonprem_projects_locations_vmware_admin_clusters_unenroll_builder, gkeonprem_projects_locations_vmware_admin_clusters_unenroll_task,
    gkeonprem_projects_locations_vmware_clusters_create_builder, gkeonprem_projects_locations_vmware_clusters_create_task,
    gkeonprem_projects_locations_vmware_clusters_delete_builder, gkeonprem_projects_locations_vmware_clusters_delete_task,
    gkeonprem_projects_locations_vmware_clusters_enroll_builder, gkeonprem_projects_locations_vmware_clusters_enroll_task,
    gkeonprem_projects_locations_vmware_clusters_patch_builder, gkeonprem_projects_locations_vmware_clusters_patch_task,
    gkeonprem_projects_locations_vmware_clusters_query_version_config_builder, gkeonprem_projects_locations_vmware_clusters_query_version_config_task,
    gkeonprem_projects_locations_vmware_clusters_set_iam_policy_builder, gkeonprem_projects_locations_vmware_clusters_set_iam_policy_task,
    gkeonprem_projects_locations_vmware_clusters_test_iam_permissions_builder, gkeonprem_projects_locations_vmware_clusters_test_iam_permissions_task,
    gkeonprem_projects_locations_vmware_clusters_unenroll_builder, gkeonprem_projects_locations_vmware_clusters_unenroll_task,
    gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_create_builder, gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_create_task,
    gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_delete_builder, gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_delete_task,
    gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_enroll_builder, gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_enroll_task,
    gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_patch_builder, gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_patch_task,
    gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_set_iam_policy_builder, gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_set_iam_policy_task,
    gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_test_iam_permissions_builder, gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_test_iam_permissions_task,
    gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_unenroll_builder, gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_unenroll_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::gkeonprem::Empty;
use crate::providers::gcp::clients::gkeonprem::Operation;
use crate::providers::gcp::clients::gkeonprem::Policy;
use crate::providers::gcp::clients::gkeonprem::QueryBareMetalAdminVersionConfigResponse;
use crate::providers::gcp::clients::gkeonprem::QueryBareMetalVersionConfigResponse;
use crate::providers::gcp::clients::gkeonprem::QueryVmwareVersionConfigResponse;
use crate::providers::gcp::clients::gkeonprem::TestIamPermissionsResponse;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalAdminClustersCreateArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalAdminClustersEnrollArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalAdminClustersPatchArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalAdminClustersQueryVersionConfigArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalAdminClustersSetIamPolicyArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalAdminClustersTestIamPermissionsArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalAdminClustersUnenrollArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalClustersBareMetalNodePoolsCreateArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalClustersBareMetalNodePoolsDeleteArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalClustersBareMetalNodePoolsEnrollArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalClustersBareMetalNodePoolsPatchArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalClustersBareMetalNodePoolsSetIamPolicyArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalClustersBareMetalNodePoolsTestIamPermissionsArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalClustersBareMetalNodePoolsUnenrollArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalClustersCreateArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalClustersDeleteArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalClustersEnrollArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalClustersPatchArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalClustersQueryVersionConfigArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalClustersSetIamPolicyArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalClustersTestIamPermissionsArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalClustersUnenrollArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareAdminClustersCreateArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareAdminClustersEnrollArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareAdminClustersPatchArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareAdminClustersSetIamPolicyArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareAdminClustersTestIamPermissionsArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareAdminClustersUnenrollArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareClustersCreateArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareClustersDeleteArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareClustersEnrollArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareClustersPatchArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareClustersQueryVersionConfigArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareClustersSetIamPolicyArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareClustersTestIamPermissionsArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareClustersUnenrollArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareClustersVmwareNodePoolsCreateArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareClustersVmwareNodePoolsDeleteArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareClustersVmwareNodePoolsEnrollArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareClustersVmwareNodePoolsPatchArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareClustersVmwareNodePoolsSetIamPolicyArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareClustersVmwareNodePoolsTestIamPermissionsArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareClustersVmwareNodePoolsUnenrollArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// GkeonpremProvider with automatic state tracking.
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
/// let provider = GkeonpremProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct GkeonpremProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> GkeonpremProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new GkeonpremProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Gkeonprem projects locations bare metal admin clusters create.
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
    pub fn gkeonprem_projects_locations_bare_metal_admin_clusters_create(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalAdminClustersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_admin_clusters_create_builder(
            &self.http_client,
            &args.parent,
            &args.allowPreflightFailure,
            &args.bareMetalAdminClusterId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_admin_clusters_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal admin clusters enroll.
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
    pub fn gkeonprem_projects_locations_bare_metal_admin_clusters_enroll(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalAdminClustersEnrollArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_admin_clusters_enroll_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_admin_clusters_enroll_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal admin clusters patch.
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
    pub fn gkeonprem_projects_locations_bare_metal_admin_clusters_patch(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalAdminClustersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_admin_clusters_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_admin_clusters_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal admin clusters query version config.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the QueryBareMetalAdminVersionConfigResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gkeonprem_projects_locations_bare_metal_admin_clusters_query_version_config(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalAdminClustersQueryVersionConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<QueryBareMetalAdminVersionConfigResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_admin_clusters_query_version_config_builder(
            &self.http_client,
            &args.parent,
            &args.upgradeConfig.clusterName,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_admin_clusters_query_version_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal admin clusters set iam policy.
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
    pub fn gkeonprem_projects_locations_bare_metal_admin_clusters_set_iam_policy(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalAdminClustersSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_admin_clusters_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_admin_clusters_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal admin clusters test iam permissions.
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
    pub fn gkeonprem_projects_locations_bare_metal_admin_clusters_test_iam_permissions(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalAdminClustersTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_admin_clusters_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_admin_clusters_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal admin clusters unenroll.
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
    pub fn gkeonprem_projects_locations_bare_metal_admin_clusters_unenroll(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalAdminClustersUnenrollArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_admin_clusters_unenroll_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.etag,
            &args.ignoreErrors,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_admin_clusters_unenroll_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal clusters create.
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
    pub fn gkeonprem_projects_locations_bare_metal_clusters_create(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalClustersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_clusters_create_builder(
            &self.http_client,
            &args.parent,
            &args.allowPreflightFailure,
            &args.bareMetalClusterId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_clusters_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal clusters delete.
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
    pub fn gkeonprem_projects_locations_bare_metal_clusters_delete(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalClustersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_clusters_delete_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.etag,
            &args.force,
            &args.ignoreErrors,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_clusters_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal clusters enroll.
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
    pub fn gkeonprem_projects_locations_bare_metal_clusters_enroll(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalClustersEnrollArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_clusters_enroll_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_clusters_enroll_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal clusters patch.
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
    pub fn gkeonprem_projects_locations_bare_metal_clusters_patch(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalClustersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_clusters_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_clusters_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal clusters query version config.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the QueryBareMetalVersionConfigResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gkeonprem_projects_locations_bare_metal_clusters_query_version_config(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalClustersQueryVersionConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<QueryBareMetalVersionConfigResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_clusters_query_version_config_builder(
            &self.http_client,
            &args.parent,
            &args.createConfig.adminClusterMembership,
            &args.createConfig.adminClusterName,
            &args.upgradeConfig.clusterName,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_clusters_query_version_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal clusters set iam policy.
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
    pub fn gkeonprem_projects_locations_bare_metal_clusters_set_iam_policy(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalClustersSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_clusters_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_clusters_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal clusters test iam permissions.
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
    pub fn gkeonprem_projects_locations_bare_metal_clusters_test_iam_permissions(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalClustersTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_clusters_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_clusters_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal clusters unenroll.
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
    pub fn gkeonprem_projects_locations_bare_metal_clusters_unenroll(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalClustersUnenrollArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_clusters_unenroll_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.etag,
            &args.force,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_clusters_unenroll_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal clusters bare metal node pools create.
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
    pub fn gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_create(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalClustersBareMetalNodePoolsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_create_builder(
            &self.http_client,
            &args.parent,
            &args.bareMetalNodePoolId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal clusters bare metal node pools delete.
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
    pub fn gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_delete(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalClustersBareMetalNodePoolsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_delete_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.etag,
            &args.ignoreErrors,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal clusters bare metal node pools enroll.
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
    pub fn gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_enroll(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalClustersBareMetalNodePoolsEnrollArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_enroll_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_enroll_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal clusters bare metal node pools patch.
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
    pub fn gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_patch(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalClustersBareMetalNodePoolsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal clusters bare metal node pools set iam policy.
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
    pub fn gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_set_iam_policy(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalClustersBareMetalNodePoolsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal clusters bare metal node pools test iam permissions.
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
    pub fn gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_test_iam_permissions(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalClustersBareMetalNodePoolsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal clusters bare metal node pools unenroll.
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
    pub fn gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_unenroll(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalClustersBareMetalNodePoolsUnenrollArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_unenroll_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.etag,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_unenroll_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations operations cancel.
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
    pub fn gkeonprem_projects_locations_operations_cancel(
        &self,
        args: &GkeonpremProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations operations delete.
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
    pub fn gkeonprem_projects_locations_operations_delete(
        &self,
        args: &GkeonpremProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware admin clusters create.
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
    pub fn gkeonprem_projects_locations_vmware_admin_clusters_create(
        &self,
        args: &GkeonpremProjectsLocationsVmwareAdminClustersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_admin_clusters_create_builder(
            &self.http_client,
            &args.parent,
            &args.allowPreflightFailure,
            &args.skipValidations,
            &args.validateOnly,
            &args.vmwareAdminClusterId,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_admin_clusters_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware admin clusters enroll.
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
    pub fn gkeonprem_projects_locations_vmware_admin_clusters_enroll(
        &self,
        args: &GkeonpremProjectsLocationsVmwareAdminClustersEnrollArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_admin_clusters_enroll_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_admin_clusters_enroll_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware admin clusters patch.
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
    pub fn gkeonprem_projects_locations_vmware_admin_clusters_patch(
        &self,
        args: &GkeonpremProjectsLocationsVmwareAdminClustersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_admin_clusters_patch_builder(
            &self.http_client,
            &args.name,
            &args.skipValidations,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_admin_clusters_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware admin clusters set iam policy.
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
    pub fn gkeonprem_projects_locations_vmware_admin_clusters_set_iam_policy(
        &self,
        args: &GkeonpremProjectsLocationsVmwareAdminClustersSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_admin_clusters_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_admin_clusters_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware admin clusters test iam permissions.
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
    pub fn gkeonprem_projects_locations_vmware_admin_clusters_test_iam_permissions(
        &self,
        args: &GkeonpremProjectsLocationsVmwareAdminClustersTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_admin_clusters_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_admin_clusters_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware admin clusters unenroll.
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
    pub fn gkeonprem_projects_locations_vmware_admin_clusters_unenroll(
        &self,
        args: &GkeonpremProjectsLocationsVmwareAdminClustersUnenrollArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_admin_clusters_unenroll_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.etag,
            &args.ignoreErrors,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_admin_clusters_unenroll_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware clusters create.
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
    pub fn gkeonprem_projects_locations_vmware_clusters_create(
        &self,
        args: &GkeonpremProjectsLocationsVmwareClustersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_clusters_create_builder(
            &self.http_client,
            &args.parent,
            &args.allowPreflightFailure,
            &args.skipValidations,
            &args.validateOnly,
            &args.vmwareClusterId,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_clusters_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware clusters delete.
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
    pub fn gkeonprem_projects_locations_vmware_clusters_delete(
        &self,
        args: &GkeonpremProjectsLocationsVmwareClustersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_clusters_delete_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.etag,
            &args.force,
            &args.ignoreErrors,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_clusters_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware clusters enroll.
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
    pub fn gkeonprem_projects_locations_vmware_clusters_enroll(
        &self,
        args: &GkeonpremProjectsLocationsVmwareClustersEnrollArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_clusters_enroll_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_clusters_enroll_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware clusters patch.
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
    pub fn gkeonprem_projects_locations_vmware_clusters_patch(
        &self,
        args: &GkeonpremProjectsLocationsVmwareClustersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_clusters_patch_builder(
            &self.http_client,
            &args.name,
            &args.skipValidations,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_clusters_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware clusters query version config.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the QueryVmwareVersionConfigResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gkeonprem_projects_locations_vmware_clusters_query_version_config(
        &self,
        args: &GkeonpremProjectsLocationsVmwareClustersQueryVersionConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<QueryVmwareVersionConfigResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_clusters_query_version_config_builder(
            &self.http_client,
            &args.parent,
            &args.createConfig.adminClusterMembership,
            &args.createConfig.adminClusterName,
            &args.upgradeConfig.clusterName,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_clusters_query_version_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware clusters set iam policy.
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
    pub fn gkeonprem_projects_locations_vmware_clusters_set_iam_policy(
        &self,
        args: &GkeonpremProjectsLocationsVmwareClustersSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_clusters_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_clusters_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware clusters test iam permissions.
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
    pub fn gkeonprem_projects_locations_vmware_clusters_test_iam_permissions(
        &self,
        args: &GkeonpremProjectsLocationsVmwareClustersTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_clusters_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_clusters_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware clusters unenroll.
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
    pub fn gkeonprem_projects_locations_vmware_clusters_unenroll(
        &self,
        args: &GkeonpremProjectsLocationsVmwareClustersUnenrollArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_clusters_unenroll_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.etag,
            &args.force,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_clusters_unenroll_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware clusters vmware node pools create.
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
    pub fn gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_create(
        &self,
        args: &GkeonpremProjectsLocationsVmwareClustersVmwareNodePoolsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_create_builder(
            &self.http_client,
            &args.parent,
            &args.validateOnly,
            &args.vmwareNodePoolId,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware clusters vmware node pools delete.
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
    pub fn gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_delete(
        &self,
        args: &GkeonpremProjectsLocationsVmwareClustersVmwareNodePoolsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_delete_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.etag,
            &args.ignoreErrors,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware clusters vmware node pools enroll.
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
    pub fn gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_enroll(
        &self,
        args: &GkeonpremProjectsLocationsVmwareClustersVmwareNodePoolsEnrollArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_enroll_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_enroll_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware clusters vmware node pools patch.
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
    pub fn gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_patch(
        &self,
        args: &GkeonpremProjectsLocationsVmwareClustersVmwareNodePoolsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware clusters vmware node pools set iam policy.
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
    pub fn gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_set_iam_policy(
        &self,
        args: &GkeonpremProjectsLocationsVmwareClustersVmwareNodePoolsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware clusters vmware node pools test iam permissions.
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
    pub fn gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_test_iam_permissions(
        &self,
        args: &GkeonpremProjectsLocationsVmwareClustersVmwareNodePoolsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware clusters vmware node pools unenroll.
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
    pub fn gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_unenroll(
        &self,
        args: &GkeonpremProjectsLocationsVmwareClustersVmwareNodePoolsUnenrollArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_unenroll_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.etag,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_unenroll_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
