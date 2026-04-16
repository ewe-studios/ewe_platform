//! ConfigProvider - State-aware config API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       config API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::config::{
    config_projects_locations_get_builder, config_projects_locations_get_task,
    config_projects_locations_get_auto_migration_config_builder, config_projects_locations_get_auto_migration_config_task,
    config_projects_locations_list_builder, config_projects_locations_list_task,
    config_projects_locations_update_auto_migration_config_builder, config_projects_locations_update_auto_migration_config_task,
    config_projects_locations_deployment_groups_create_builder, config_projects_locations_deployment_groups_create_task,
    config_projects_locations_deployment_groups_delete_builder, config_projects_locations_deployment_groups_delete_task,
    config_projects_locations_deployment_groups_deprovision_builder, config_projects_locations_deployment_groups_deprovision_task,
    config_projects_locations_deployment_groups_get_builder, config_projects_locations_deployment_groups_get_task,
    config_projects_locations_deployment_groups_list_builder, config_projects_locations_deployment_groups_list_task,
    config_projects_locations_deployment_groups_patch_builder, config_projects_locations_deployment_groups_patch_task,
    config_projects_locations_deployment_groups_provision_builder, config_projects_locations_deployment_groups_provision_task,
    config_projects_locations_deployment_groups_revisions_get_builder, config_projects_locations_deployment_groups_revisions_get_task,
    config_projects_locations_deployment_groups_revisions_list_builder, config_projects_locations_deployment_groups_revisions_list_task,
    config_projects_locations_deployments_create_builder, config_projects_locations_deployments_create_task,
    config_projects_locations_deployments_delete_builder, config_projects_locations_deployments_delete_task,
    config_projects_locations_deployments_delete_state_builder, config_projects_locations_deployments_delete_state_task,
    config_projects_locations_deployments_export_lock_builder, config_projects_locations_deployments_export_lock_task,
    config_projects_locations_deployments_export_state_builder, config_projects_locations_deployments_export_state_task,
    config_projects_locations_deployments_get_builder, config_projects_locations_deployments_get_task,
    config_projects_locations_deployments_get_iam_policy_builder, config_projects_locations_deployments_get_iam_policy_task,
    config_projects_locations_deployments_import_state_builder, config_projects_locations_deployments_import_state_task,
    config_projects_locations_deployments_list_builder, config_projects_locations_deployments_list_task,
    config_projects_locations_deployments_lock_builder, config_projects_locations_deployments_lock_task,
    config_projects_locations_deployments_patch_builder, config_projects_locations_deployments_patch_task,
    config_projects_locations_deployments_set_iam_policy_builder, config_projects_locations_deployments_set_iam_policy_task,
    config_projects_locations_deployments_test_iam_permissions_builder, config_projects_locations_deployments_test_iam_permissions_task,
    config_projects_locations_deployments_unlock_builder, config_projects_locations_deployments_unlock_task,
    config_projects_locations_deployments_revisions_export_state_builder, config_projects_locations_deployments_revisions_export_state_task,
    config_projects_locations_deployments_revisions_get_builder, config_projects_locations_deployments_revisions_get_task,
    config_projects_locations_deployments_revisions_list_builder, config_projects_locations_deployments_revisions_list_task,
    config_projects_locations_deployments_revisions_resources_get_builder, config_projects_locations_deployments_revisions_resources_get_task,
    config_projects_locations_deployments_revisions_resources_list_builder, config_projects_locations_deployments_revisions_resources_list_task,
    config_projects_locations_operations_cancel_builder, config_projects_locations_operations_cancel_task,
    config_projects_locations_operations_delete_builder, config_projects_locations_operations_delete_task,
    config_projects_locations_operations_get_builder, config_projects_locations_operations_get_task,
    config_projects_locations_operations_list_builder, config_projects_locations_operations_list_task,
    config_projects_locations_previews_create_builder, config_projects_locations_previews_create_task,
    config_projects_locations_previews_delete_builder, config_projects_locations_previews_delete_task,
    config_projects_locations_previews_export_builder, config_projects_locations_previews_export_task,
    config_projects_locations_previews_get_builder, config_projects_locations_previews_get_task,
    config_projects_locations_previews_list_builder, config_projects_locations_previews_list_task,
    config_projects_locations_previews_resource_changes_get_builder, config_projects_locations_previews_resource_changes_get_task,
    config_projects_locations_previews_resource_changes_list_builder, config_projects_locations_previews_resource_changes_list_task,
    config_projects_locations_previews_resource_drifts_get_builder, config_projects_locations_previews_resource_drifts_get_task,
    config_projects_locations_previews_resource_drifts_list_builder, config_projects_locations_previews_resource_drifts_list_task,
    config_projects_locations_terraform_versions_get_builder, config_projects_locations_terraform_versions_get_task,
    config_projects_locations_terraform_versions_list_builder, config_projects_locations_terraform_versions_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::config::AutoMigrationConfig;
use crate::providers::gcp::clients::config::Deployment;
use crate::providers::gcp::clients::config::DeploymentGroup;
use crate::providers::gcp::clients::config::DeploymentGroupRevision;
use crate::providers::gcp::clients::config::Empty;
use crate::providers::gcp::clients::config::ExportPreviewResultResponse;
use crate::providers::gcp::clients::config::ListDeploymentGroupRevisionsResponse;
use crate::providers::gcp::clients::config::ListDeploymentGroupsResponse;
use crate::providers::gcp::clients::config::ListDeploymentsResponse;
use crate::providers::gcp::clients::config::ListLocationsResponse;
use crate::providers::gcp::clients::config::ListOperationsResponse;
use crate::providers::gcp::clients::config::ListPreviewsResponse;
use crate::providers::gcp::clients::config::ListResourceChangesResponse;
use crate::providers::gcp::clients::config::ListResourceDriftsResponse;
use crate::providers::gcp::clients::config::ListResourcesResponse;
use crate::providers::gcp::clients::config::ListRevisionsResponse;
use crate::providers::gcp::clients::config::ListTerraformVersionsResponse;
use crate::providers::gcp::clients::config::Location;
use crate::providers::gcp::clients::config::LockInfo;
use crate::providers::gcp::clients::config::Operation;
use crate::providers::gcp::clients::config::Policy;
use crate::providers::gcp::clients::config::Preview;
use crate::providers::gcp::clients::config::Resource;
use crate::providers::gcp::clients::config::ResourceChange;
use crate::providers::gcp::clients::config::ResourceDrift;
use crate::providers::gcp::clients::config::Revision;
use crate::providers::gcp::clients::config::Statefile;
use crate::providers::gcp::clients::config::TerraformVersion;
use crate::providers::gcp::clients::config::TestIamPermissionsResponse;
use crate::providers::gcp::clients::config::ConfigProjectsLocationsDeploymentGroupsCreateArgs;
use crate::providers::gcp::clients::config::ConfigProjectsLocationsDeploymentGroupsDeleteArgs;
use crate::providers::gcp::clients::config::ConfigProjectsLocationsDeploymentGroupsDeprovisionArgs;
use crate::providers::gcp::clients::config::ConfigProjectsLocationsDeploymentGroupsGetArgs;
use crate::providers::gcp::clients::config::ConfigProjectsLocationsDeploymentGroupsListArgs;
use crate::providers::gcp::clients::config::ConfigProjectsLocationsDeploymentGroupsPatchArgs;
use crate::providers::gcp::clients::config::ConfigProjectsLocationsDeploymentGroupsProvisionArgs;
use crate::providers::gcp::clients::config::ConfigProjectsLocationsDeploymentGroupsRevisionsGetArgs;
use crate::providers::gcp::clients::config::ConfigProjectsLocationsDeploymentGroupsRevisionsListArgs;
use crate::providers::gcp::clients::config::ConfigProjectsLocationsDeploymentsCreateArgs;
use crate::providers::gcp::clients::config::ConfigProjectsLocationsDeploymentsDeleteArgs;
use crate::providers::gcp::clients::config::ConfigProjectsLocationsDeploymentsDeleteStateArgs;
use crate::providers::gcp::clients::config::ConfigProjectsLocationsDeploymentsExportLockArgs;
use crate::providers::gcp::clients::config::ConfigProjectsLocationsDeploymentsExportStateArgs;
use crate::providers::gcp::clients::config::ConfigProjectsLocationsDeploymentsGetArgs;
use crate::providers::gcp::clients::config::ConfigProjectsLocationsDeploymentsGetIamPolicyArgs;
use crate::providers::gcp::clients::config::ConfigProjectsLocationsDeploymentsImportStateArgs;
use crate::providers::gcp::clients::config::ConfigProjectsLocationsDeploymentsListArgs;
use crate::providers::gcp::clients::config::ConfigProjectsLocationsDeploymentsLockArgs;
use crate::providers::gcp::clients::config::ConfigProjectsLocationsDeploymentsPatchArgs;
use crate::providers::gcp::clients::config::ConfigProjectsLocationsDeploymentsRevisionsExportStateArgs;
use crate::providers::gcp::clients::config::ConfigProjectsLocationsDeploymentsRevisionsGetArgs;
use crate::providers::gcp::clients::config::ConfigProjectsLocationsDeploymentsRevisionsListArgs;
use crate::providers::gcp::clients::config::ConfigProjectsLocationsDeploymentsRevisionsResourcesGetArgs;
use crate::providers::gcp::clients::config::ConfigProjectsLocationsDeploymentsRevisionsResourcesListArgs;
use crate::providers::gcp::clients::config::ConfigProjectsLocationsDeploymentsSetIamPolicyArgs;
use crate::providers::gcp::clients::config::ConfigProjectsLocationsDeploymentsTestIamPermissionsArgs;
use crate::providers::gcp::clients::config::ConfigProjectsLocationsDeploymentsUnlockArgs;
use crate::providers::gcp::clients::config::ConfigProjectsLocationsGetArgs;
use crate::providers::gcp::clients::config::ConfigProjectsLocationsGetAutoMigrationConfigArgs;
use crate::providers::gcp::clients::config::ConfigProjectsLocationsListArgs;
use crate::providers::gcp::clients::config::ConfigProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::config::ConfigProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::config::ConfigProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::config::ConfigProjectsLocationsOperationsListArgs;
use crate::providers::gcp::clients::config::ConfigProjectsLocationsPreviewsCreateArgs;
use crate::providers::gcp::clients::config::ConfigProjectsLocationsPreviewsDeleteArgs;
use crate::providers::gcp::clients::config::ConfigProjectsLocationsPreviewsExportArgs;
use crate::providers::gcp::clients::config::ConfigProjectsLocationsPreviewsGetArgs;
use crate::providers::gcp::clients::config::ConfigProjectsLocationsPreviewsListArgs;
use crate::providers::gcp::clients::config::ConfigProjectsLocationsPreviewsResourceChangesGetArgs;
use crate::providers::gcp::clients::config::ConfigProjectsLocationsPreviewsResourceChangesListArgs;
use crate::providers::gcp::clients::config::ConfigProjectsLocationsPreviewsResourceDriftsGetArgs;
use crate::providers::gcp::clients::config::ConfigProjectsLocationsPreviewsResourceDriftsListArgs;
use crate::providers::gcp::clients::config::ConfigProjectsLocationsTerraformVersionsGetArgs;
use crate::providers::gcp::clients::config::ConfigProjectsLocationsTerraformVersionsListArgs;
use crate::providers::gcp::clients::config::ConfigProjectsLocationsUpdateAutoMigrationConfigArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// ConfigProvider with automatic state tracking.
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
/// let provider = ConfigProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct ConfigProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> ConfigProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new ConfigProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new ConfigProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Config projects locations get.
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
    pub fn config_projects_locations_get(
        &self,
        args: &ConfigProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = config_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = config_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Config projects locations get auto migration config.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AutoMigrationConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn config_projects_locations_get_auto_migration_config(
        &self,
        args: &ConfigProjectsLocationsGetAutoMigrationConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AutoMigrationConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = config_projects_locations_get_auto_migration_config_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = config_projects_locations_get_auto_migration_config_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Config projects locations list.
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
    pub fn config_projects_locations_list(
        &self,
        args: &ConfigProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = config_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = config_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Config projects locations update auto migration config.
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
    pub fn config_projects_locations_update_auto_migration_config(
        &self,
        args: &ConfigProjectsLocationsUpdateAutoMigrationConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = config_projects_locations_update_auto_migration_config_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = config_projects_locations_update_auto_migration_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Config projects locations deployment groups create.
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
    pub fn config_projects_locations_deployment_groups_create(
        &self,
        args: &ConfigProjectsLocationsDeploymentGroupsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = config_projects_locations_deployment_groups_create_builder(
            &self.http_client,
            &args.parent,
            &args.deploymentGroupId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = config_projects_locations_deployment_groups_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Config projects locations deployment groups delete.
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
    pub fn config_projects_locations_deployment_groups_delete(
        &self,
        args: &ConfigProjectsLocationsDeploymentGroupsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = config_projects_locations_deployment_groups_delete_builder(
            &self.http_client,
            &args.name,
            &args.deploymentReferencePolicy,
            &args.force,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = config_projects_locations_deployment_groups_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Config projects locations deployment groups deprovision.
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
    pub fn config_projects_locations_deployment_groups_deprovision(
        &self,
        args: &ConfigProjectsLocationsDeploymentGroupsDeprovisionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = config_projects_locations_deployment_groups_deprovision_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = config_projects_locations_deployment_groups_deprovision_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Config projects locations deployment groups get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeploymentGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn config_projects_locations_deployment_groups_get(
        &self,
        args: &ConfigProjectsLocationsDeploymentGroupsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeploymentGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = config_projects_locations_deployment_groups_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = config_projects_locations_deployment_groups_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Config projects locations deployment groups list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDeploymentGroupsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn config_projects_locations_deployment_groups_list(
        &self,
        args: &ConfigProjectsLocationsDeploymentGroupsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDeploymentGroupsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = config_projects_locations_deployment_groups_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = config_projects_locations_deployment_groups_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Config projects locations deployment groups patch.
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
    pub fn config_projects_locations_deployment_groups_patch(
        &self,
        args: &ConfigProjectsLocationsDeploymentGroupsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = config_projects_locations_deployment_groups_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = config_projects_locations_deployment_groups_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Config projects locations deployment groups provision.
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
    pub fn config_projects_locations_deployment_groups_provision(
        &self,
        args: &ConfigProjectsLocationsDeploymentGroupsProvisionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = config_projects_locations_deployment_groups_provision_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = config_projects_locations_deployment_groups_provision_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Config projects locations deployment groups revisions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeploymentGroupRevision result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn config_projects_locations_deployment_groups_revisions_get(
        &self,
        args: &ConfigProjectsLocationsDeploymentGroupsRevisionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeploymentGroupRevision, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = config_projects_locations_deployment_groups_revisions_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = config_projects_locations_deployment_groups_revisions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Config projects locations deployment groups revisions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDeploymentGroupRevisionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn config_projects_locations_deployment_groups_revisions_list(
        &self,
        args: &ConfigProjectsLocationsDeploymentGroupsRevisionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDeploymentGroupRevisionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = config_projects_locations_deployment_groups_revisions_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = config_projects_locations_deployment_groups_revisions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Config projects locations deployments create.
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
    pub fn config_projects_locations_deployments_create(
        &self,
        args: &ConfigProjectsLocationsDeploymentsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = config_projects_locations_deployments_create_builder(
            &self.http_client,
            &args.parent,
            &args.deploymentId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = config_projects_locations_deployments_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Config projects locations deployments delete.
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
    pub fn config_projects_locations_deployments_delete(
        &self,
        args: &ConfigProjectsLocationsDeploymentsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = config_projects_locations_deployments_delete_builder(
            &self.http_client,
            &args.name,
            &args.deletePolicy,
            &args.force,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = config_projects_locations_deployments_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Config projects locations deployments delete state.
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
    pub fn config_projects_locations_deployments_delete_state(
        &self,
        args: &ConfigProjectsLocationsDeploymentsDeleteStateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = config_projects_locations_deployments_delete_state_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = config_projects_locations_deployments_delete_state_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Config projects locations deployments export lock.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LockInfo result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn config_projects_locations_deployments_export_lock(
        &self,
        args: &ConfigProjectsLocationsDeploymentsExportLockArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LockInfo, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = config_projects_locations_deployments_export_lock_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = config_projects_locations_deployments_export_lock_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Config projects locations deployments export state.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Statefile result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn config_projects_locations_deployments_export_state(
        &self,
        args: &ConfigProjectsLocationsDeploymentsExportStateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Statefile, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = config_projects_locations_deployments_export_state_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = config_projects_locations_deployments_export_state_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Config projects locations deployments get.
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
    pub fn config_projects_locations_deployments_get(
        &self,
        args: &ConfigProjectsLocationsDeploymentsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Deployment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = config_projects_locations_deployments_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = config_projects_locations_deployments_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Config projects locations deployments get iam policy.
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
    pub fn config_projects_locations_deployments_get_iam_policy(
        &self,
        args: &ConfigProjectsLocationsDeploymentsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = config_projects_locations_deployments_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options_requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = config_projects_locations_deployments_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Config projects locations deployments import state.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Statefile result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn config_projects_locations_deployments_import_state(
        &self,
        args: &ConfigProjectsLocationsDeploymentsImportStateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Statefile, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = config_projects_locations_deployments_import_state_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = config_projects_locations_deployments_import_state_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Config projects locations deployments list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDeploymentsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn config_projects_locations_deployments_list(
        &self,
        args: &ConfigProjectsLocationsDeploymentsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDeploymentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = config_projects_locations_deployments_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = config_projects_locations_deployments_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Config projects locations deployments lock.
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
    pub fn config_projects_locations_deployments_lock(
        &self,
        args: &ConfigProjectsLocationsDeploymentsLockArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = config_projects_locations_deployments_lock_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = config_projects_locations_deployments_lock_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Config projects locations deployments patch.
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
    pub fn config_projects_locations_deployments_patch(
        &self,
        args: &ConfigProjectsLocationsDeploymentsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = config_projects_locations_deployments_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = config_projects_locations_deployments_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Config projects locations deployments set iam policy.
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
    pub fn config_projects_locations_deployments_set_iam_policy(
        &self,
        args: &ConfigProjectsLocationsDeploymentsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = config_projects_locations_deployments_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = config_projects_locations_deployments_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Config projects locations deployments test iam permissions.
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
    pub fn config_projects_locations_deployments_test_iam_permissions(
        &self,
        args: &ConfigProjectsLocationsDeploymentsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = config_projects_locations_deployments_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = config_projects_locations_deployments_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Config projects locations deployments unlock.
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
    pub fn config_projects_locations_deployments_unlock(
        &self,
        args: &ConfigProjectsLocationsDeploymentsUnlockArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = config_projects_locations_deployments_unlock_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = config_projects_locations_deployments_unlock_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Config projects locations deployments revisions export state.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Statefile result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn config_projects_locations_deployments_revisions_export_state(
        &self,
        args: &ConfigProjectsLocationsDeploymentsRevisionsExportStateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Statefile, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = config_projects_locations_deployments_revisions_export_state_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = config_projects_locations_deployments_revisions_export_state_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Config projects locations deployments revisions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Revision result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn config_projects_locations_deployments_revisions_get(
        &self,
        args: &ConfigProjectsLocationsDeploymentsRevisionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Revision, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = config_projects_locations_deployments_revisions_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = config_projects_locations_deployments_revisions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Config projects locations deployments revisions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListRevisionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn config_projects_locations_deployments_revisions_list(
        &self,
        args: &ConfigProjectsLocationsDeploymentsRevisionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListRevisionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = config_projects_locations_deployments_revisions_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = config_projects_locations_deployments_revisions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Config projects locations deployments revisions resources get.
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
    pub fn config_projects_locations_deployments_revisions_resources_get(
        &self,
        args: &ConfigProjectsLocationsDeploymentsRevisionsResourcesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Resource, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = config_projects_locations_deployments_revisions_resources_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = config_projects_locations_deployments_revisions_resources_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Config projects locations deployments revisions resources list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListResourcesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn config_projects_locations_deployments_revisions_resources_list(
        &self,
        args: &ConfigProjectsLocationsDeploymentsRevisionsResourcesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListResourcesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = config_projects_locations_deployments_revisions_resources_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = config_projects_locations_deployments_revisions_resources_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Config projects locations operations cancel.
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
    pub fn config_projects_locations_operations_cancel(
        &self,
        args: &ConfigProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = config_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = config_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Config projects locations operations delete.
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
    pub fn config_projects_locations_operations_delete(
        &self,
        args: &ConfigProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = config_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = config_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Config projects locations operations get.
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
    pub fn config_projects_locations_operations_get(
        &self,
        args: &ConfigProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = config_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = config_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Config projects locations operations list.
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
    pub fn config_projects_locations_operations_list(
        &self,
        args: &ConfigProjectsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = config_projects_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = config_projects_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Config projects locations previews create.
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
    pub fn config_projects_locations_previews_create(
        &self,
        args: &ConfigProjectsLocationsPreviewsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = config_projects_locations_previews_create_builder(
            &self.http_client,
            &args.parent,
            &args.previewId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = config_projects_locations_previews_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Config projects locations previews delete.
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
    pub fn config_projects_locations_previews_delete(
        &self,
        args: &ConfigProjectsLocationsPreviewsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = config_projects_locations_previews_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = config_projects_locations_previews_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Config projects locations previews export.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ExportPreviewResultResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn config_projects_locations_previews_export(
        &self,
        args: &ConfigProjectsLocationsPreviewsExportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ExportPreviewResultResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = config_projects_locations_previews_export_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = config_projects_locations_previews_export_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Config projects locations previews get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Preview result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn config_projects_locations_previews_get(
        &self,
        args: &ConfigProjectsLocationsPreviewsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Preview, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = config_projects_locations_previews_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = config_projects_locations_previews_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Config projects locations previews list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListPreviewsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn config_projects_locations_previews_list(
        &self,
        args: &ConfigProjectsLocationsPreviewsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListPreviewsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = config_projects_locations_previews_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = config_projects_locations_previews_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Config projects locations previews resource changes get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ResourceChange result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn config_projects_locations_previews_resource_changes_get(
        &self,
        args: &ConfigProjectsLocationsPreviewsResourceChangesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ResourceChange, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = config_projects_locations_previews_resource_changes_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = config_projects_locations_previews_resource_changes_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Config projects locations previews resource changes list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListResourceChangesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn config_projects_locations_previews_resource_changes_list(
        &self,
        args: &ConfigProjectsLocationsPreviewsResourceChangesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListResourceChangesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = config_projects_locations_previews_resource_changes_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = config_projects_locations_previews_resource_changes_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Config projects locations previews resource drifts get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ResourceDrift result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn config_projects_locations_previews_resource_drifts_get(
        &self,
        args: &ConfigProjectsLocationsPreviewsResourceDriftsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ResourceDrift, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = config_projects_locations_previews_resource_drifts_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = config_projects_locations_previews_resource_drifts_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Config projects locations previews resource drifts list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListResourceDriftsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn config_projects_locations_previews_resource_drifts_list(
        &self,
        args: &ConfigProjectsLocationsPreviewsResourceDriftsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListResourceDriftsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = config_projects_locations_previews_resource_drifts_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = config_projects_locations_previews_resource_drifts_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Config projects locations terraform versions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TerraformVersion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn config_projects_locations_terraform_versions_get(
        &self,
        args: &ConfigProjectsLocationsTerraformVersionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TerraformVersion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = config_projects_locations_terraform_versions_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = config_projects_locations_terraform_versions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Config projects locations terraform versions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListTerraformVersionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn config_projects_locations_terraform_versions_list(
        &self,
        args: &ConfigProjectsLocationsTerraformVersionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListTerraformVersionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = config_projects_locations_terraform_versions_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = config_projects_locations_terraform_versions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
