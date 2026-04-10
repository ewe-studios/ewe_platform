//! DatamigrationProvider - State-aware datamigration API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       datamigration API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::datamigration::{
    datamigration_projects_locations_fetch_static_ips_builder, datamigration_projects_locations_fetch_static_ips_task,
    datamigration_projects_locations_get_builder, datamigration_projects_locations_get_task,
    datamigration_projects_locations_list_builder, datamigration_projects_locations_list_task,
    datamigration_projects_locations_connection_profiles_create_builder, datamigration_projects_locations_connection_profiles_create_task,
    datamigration_projects_locations_connection_profiles_delete_builder, datamigration_projects_locations_connection_profiles_delete_task,
    datamigration_projects_locations_connection_profiles_get_builder, datamigration_projects_locations_connection_profiles_get_task,
    datamigration_projects_locations_connection_profiles_get_iam_policy_builder, datamigration_projects_locations_connection_profiles_get_iam_policy_task,
    datamigration_projects_locations_connection_profiles_list_builder, datamigration_projects_locations_connection_profiles_list_task,
    datamigration_projects_locations_connection_profiles_patch_builder, datamigration_projects_locations_connection_profiles_patch_task,
    datamigration_projects_locations_connection_profiles_set_iam_policy_builder, datamigration_projects_locations_connection_profiles_set_iam_policy_task,
    datamigration_projects_locations_connection_profiles_test_iam_permissions_builder, datamigration_projects_locations_connection_profiles_test_iam_permissions_task,
    datamigration_projects_locations_conversion_workspaces_apply_builder, datamigration_projects_locations_conversion_workspaces_apply_task,
    datamigration_projects_locations_conversion_workspaces_commit_builder, datamigration_projects_locations_conversion_workspaces_commit_task,
    datamigration_projects_locations_conversion_workspaces_convert_builder, datamigration_projects_locations_conversion_workspaces_convert_task,
    datamigration_projects_locations_conversion_workspaces_create_builder, datamigration_projects_locations_conversion_workspaces_create_task,
    datamigration_projects_locations_conversion_workspaces_delete_builder, datamigration_projects_locations_conversion_workspaces_delete_task,
    datamigration_projects_locations_conversion_workspaces_describe_conversion_workspace_revisions_builder, datamigration_projects_locations_conversion_workspaces_describe_conversion_workspace_revisions_task,
    datamigration_projects_locations_conversion_workspaces_describe_database_entities_builder, datamigration_projects_locations_conversion_workspaces_describe_database_entities_task,
    datamigration_projects_locations_conversion_workspaces_get_builder, datamigration_projects_locations_conversion_workspaces_get_task,
    datamigration_projects_locations_conversion_workspaces_get_iam_policy_builder, datamigration_projects_locations_conversion_workspaces_get_iam_policy_task,
    datamigration_projects_locations_conversion_workspaces_list_builder, datamigration_projects_locations_conversion_workspaces_list_task,
    datamigration_projects_locations_conversion_workspaces_patch_builder, datamigration_projects_locations_conversion_workspaces_patch_task,
    datamigration_projects_locations_conversion_workspaces_rollback_builder, datamigration_projects_locations_conversion_workspaces_rollback_task,
    datamigration_projects_locations_conversion_workspaces_search_background_jobs_builder, datamigration_projects_locations_conversion_workspaces_search_background_jobs_task,
    datamigration_projects_locations_conversion_workspaces_seed_builder, datamigration_projects_locations_conversion_workspaces_seed_task,
    datamigration_projects_locations_conversion_workspaces_set_iam_policy_builder, datamigration_projects_locations_conversion_workspaces_set_iam_policy_task,
    datamigration_projects_locations_conversion_workspaces_test_iam_permissions_builder, datamigration_projects_locations_conversion_workspaces_test_iam_permissions_task,
    datamigration_projects_locations_conversion_workspaces_mapping_rules_create_builder, datamigration_projects_locations_conversion_workspaces_mapping_rules_create_task,
    datamigration_projects_locations_conversion_workspaces_mapping_rules_delete_builder, datamigration_projects_locations_conversion_workspaces_mapping_rules_delete_task,
    datamigration_projects_locations_conversion_workspaces_mapping_rules_get_builder, datamigration_projects_locations_conversion_workspaces_mapping_rules_get_task,
    datamigration_projects_locations_conversion_workspaces_mapping_rules_import_builder, datamigration_projects_locations_conversion_workspaces_mapping_rules_import_task,
    datamigration_projects_locations_conversion_workspaces_mapping_rules_list_builder, datamigration_projects_locations_conversion_workspaces_mapping_rules_list_task,
    datamigration_projects_locations_migration_jobs_create_builder, datamigration_projects_locations_migration_jobs_create_task,
    datamigration_projects_locations_migration_jobs_delete_builder, datamigration_projects_locations_migration_jobs_delete_task,
    datamigration_projects_locations_migration_jobs_demote_destination_builder, datamigration_projects_locations_migration_jobs_demote_destination_task,
    datamigration_projects_locations_migration_jobs_fetch_source_objects_builder, datamigration_projects_locations_migration_jobs_fetch_source_objects_task,
    datamigration_projects_locations_migration_jobs_generate_ssh_script_builder, datamigration_projects_locations_migration_jobs_generate_ssh_script_task,
    datamigration_projects_locations_migration_jobs_generate_tcp_proxy_script_builder, datamigration_projects_locations_migration_jobs_generate_tcp_proxy_script_task,
    datamigration_projects_locations_migration_jobs_get_builder, datamigration_projects_locations_migration_jobs_get_task,
    datamigration_projects_locations_migration_jobs_get_iam_policy_builder, datamigration_projects_locations_migration_jobs_get_iam_policy_task,
    datamigration_projects_locations_migration_jobs_list_builder, datamigration_projects_locations_migration_jobs_list_task,
    datamigration_projects_locations_migration_jobs_patch_builder, datamigration_projects_locations_migration_jobs_patch_task,
    datamigration_projects_locations_migration_jobs_promote_builder, datamigration_projects_locations_migration_jobs_promote_task,
    datamigration_projects_locations_migration_jobs_restart_builder, datamigration_projects_locations_migration_jobs_restart_task,
    datamigration_projects_locations_migration_jobs_resume_builder, datamigration_projects_locations_migration_jobs_resume_task,
    datamigration_projects_locations_migration_jobs_set_iam_policy_builder, datamigration_projects_locations_migration_jobs_set_iam_policy_task,
    datamigration_projects_locations_migration_jobs_start_builder, datamigration_projects_locations_migration_jobs_start_task,
    datamigration_projects_locations_migration_jobs_stop_builder, datamigration_projects_locations_migration_jobs_stop_task,
    datamigration_projects_locations_migration_jobs_test_iam_permissions_builder, datamigration_projects_locations_migration_jobs_test_iam_permissions_task,
    datamigration_projects_locations_migration_jobs_verify_builder, datamigration_projects_locations_migration_jobs_verify_task,
    datamigration_projects_locations_migration_jobs_objects_get_builder, datamigration_projects_locations_migration_jobs_objects_get_task,
    datamigration_projects_locations_migration_jobs_objects_get_iam_policy_builder, datamigration_projects_locations_migration_jobs_objects_get_iam_policy_task,
    datamigration_projects_locations_migration_jobs_objects_list_builder, datamigration_projects_locations_migration_jobs_objects_list_task,
    datamigration_projects_locations_migration_jobs_objects_lookup_builder, datamigration_projects_locations_migration_jobs_objects_lookup_task,
    datamigration_projects_locations_migration_jobs_objects_set_iam_policy_builder, datamigration_projects_locations_migration_jobs_objects_set_iam_policy_task,
    datamigration_projects_locations_migration_jobs_objects_test_iam_permissions_builder, datamigration_projects_locations_migration_jobs_objects_test_iam_permissions_task,
    datamigration_projects_locations_operations_cancel_builder, datamigration_projects_locations_operations_cancel_task,
    datamigration_projects_locations_operations_delete_builder, datamigration_projects_locations_operations_delete_task,
    datamigration_projects_locations_operations_get_builder, datamigration_projects_locations_operations_get_task,
    datamigration_projects_locations_operations_list_builder, datamigration_projects_locations_operations_list_task,
    datamigration_projects_locations_private_connections_create_builder, datamigration_projects_locations_private_connections_create_task,
    datamigration_projects_locations_private_connections_delete_builder, datamigration_projects_locations_private_connections_delete_task,
    datamigration_projects_locations_private_connections_get_builder, datamigration_projects_locations_private_connections_get_task,
    datamigration_projects_locations_private_connections_get_iam_policy_builder, datamigration_projects_locations_private_connections_get_iam_policy_task,
    datamigration_projects_locations_private_connections_list_builder, datamigration_projects_locations_private_connections_list_task,
    datamigration_projects_locations_private_connections_set_iam_policy_builder, datamigration_projects_locations_private_connections_set_iam_policy_task,
    datamigration_projects_locations_private_connections_test_iam_permissions_builder, datamigration_projects_locations_private_connections_test_iam_permissions_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::datamigration::ConnectionProfile;
use crate::providers::gcp::clients::datamigration::ConversionWorkspace;
use crate::providers::gcp::clients::datamigration::DescribeConversionWorkspaceRevisionsResponse;
use crate::providers::gcp::clients::datamigration::DescribeDatabaseEntitiesResponse;
use crate::providers::gcp::clients::datamigration::Empty;
use crate::providers::gcp::clients::datamigration::FetchStaticIpsResponse;
use crate::providers::gcp::clients::datamigration::ListConnectionProfilesResponse;
use crate::providers::gcp::clients::datamigration::ListConversionWorkspacesResponse;
use crate::providers::gcp::clients::datamigration::ListLocationsResponse;
use crate::providers::gcp::clients::datamigration::ListMappingRulesResponse;
use crate::providers::gcp::clients::datamigration::ListMigrationJobObjectsResponse;
use crate::providers::gcp::clients::datamigration::ListMigrationJobsResponse;
use crate::providers::gcp::clients::datamigration::ListOperationsResponse;
use crate::providers::gcp::clients::datamigration::ListPrivateConnectionsResponse;
use crate::providers::gcp::clients::datamigration::Location;
use crate::providers::gcp::clients::datamigration::MappingRule;
use crate::providers::gcp::clients::datamigration::MigrationJob;
use crate::providers::gcp::clients::datamigration::MigrationJobObject;
use crate::providers::gcp::clients::datamigration::Operation;
use crate::providers::gcp::clients::datamigration::Policy;
use crate::providers::gcp::clients::datamigration::PrivateConnection;
use crate::providers::gcp::clients::datamigration::SearchBackgroundJobsResponse;
use crate::providers::gcp::clients::datamigration::SshScript;
use crate::providers::gcp::clients::datamigration::TcpProxyScript;
use crate::providers::gcp::clients::datamigration::TestIamPermissionsResponse;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsConnectionProfilesCreateArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsConnectionProfilesDeleteArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsConnectionProfilesGetArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsConnectionProfilesGetIamPolicyArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsConnectionProfilesListArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsConnectionProfilesPatchArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsConnectionProfilesSetIamPolicyArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsConnectionProfilesTestIamPermissionsArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsConversionWorkspacesApplyArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsConversionWorkspacesCommitArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsConversionWorkspacesConvertArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsConversionWorkspacesCreateArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsConversionWorkspacesDeleteArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsConversionWorkspacesDescribeConversionWorkspaceRevisionsArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsConversionWorkspacesDescribeDatabaseEntitiesArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsConversionWorkspacesGetArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsConversionWorkspacesGetIamPolicyArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsConversionWorkspacesListArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsConversionWorkspacesMappingRulesCreateArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsConversionWorkspacesMappingRulesDeleteArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsConversionWorkspacesMappingRulesGetArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsConversionWorkspacesMappingRulesImportArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsConversionWorkspacesMappingRulesListArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsConversionWorkspacesPatchArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsConversionWorkspacesRollbackArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsConversionWorkspacesSearchBackgroundJobsArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsConversionWorkspacesSeedArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsConversionWorkspacesSetIamPolicyArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsConversionWorkspacesTestIamPermissionsArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsFetchStaticIpsArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsGetArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsListArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsMigrationJobsCreateArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsMigrationJobsDeleteArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsMigrationJobsDemoteDestinationArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsMigrationJobsFetchSourceObjectsArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsMigrationJobsGenerateSshScriptArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsMigrationJobsGenerateTcpProxyScriptArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsMigrationJobsGetArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsMigrationJobsGetIamPolicyArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsMigrationJobsListArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsMigrationJobsObjectsGetArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsMigrationJobsObjectsGetIamPolicyArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsMigrationJobsObjectsListArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsMigrationJobsObjectsLookupArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsMigrationJobsObjectsSetIamPolicyArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsMigrationJobsObjectsTestIamPermissionsArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsMigrationJobsPatchArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsMigrationJobsPromoteArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsMigrationJobsRestartArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsMigrationJobsResumeArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsMigrationJobsSetIamPolicyArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsMigrationJobsStartArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsMigrationJobsStopArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsMigrationJobsTestIamPermissionsArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsMigrationJobsVerifyArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsOperationsListArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsPrivateConnectionsCreateArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsPrivateConnectionsDeleteArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsPrivateConnectionsGetArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsPrivateConnectionsGetIamPolicyArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsPrivateConnectionsListArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsPrivateConnectionsSetIamPolicyArgs;
use crate::providers::gcp::clients::datamigration::DatamigrationProjectsLocationsPrivateConnectionsTestIamPermissionsArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// DatamigrationProvider with automatic state tracking.
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
/// let provider = DatamigrationProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct DatamigrationProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> DatamigrationProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new DatamigrationProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Datamigration projects locations fetch static ips.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FetchStaticIpsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn datamigration_projects_locations_fetch_static_ips(
        &self,
        args: &DatamigrationProjectsLocationsFetchStaticIpsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FetchStaticIpsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_fetch_static_ips_builder(
            &self.http_client,
            &args.name,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_fetch_static_ips_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations get.
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
    pub fn datamigration_projects_locations_get(
        &self,
        args: &DatamigrationProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations list.
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
    pub fn datamigration_projects_locations_list(
        &self,
        args: &DatamigrationProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations connection profiles create.
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
    pub fn datamigration_projects_locations_connection_profiles_create(
        &self,
        args: &DatamigrationProjectsLocationsConnectionProfilesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_connection_profiles_create_builder(
            &self.http_client,
            &args.parent,
            &args.connectionProfileId,
            &args.requestId,
            &args.skipValidation,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_connection_profiles_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations connection profiles delete.
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
    pub fn datamigration_projects_locations_connection_profiles_delete(
        &self,
        args: &DatamigrationProjectsLocationsConnectionProfilesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_connection_profiles_delete_builder(
            &self.http_client,
            &args.name,
            &args.force,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_connection_profiles_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations connection profiles get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ConnectionProfile result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn datamigration_projects_locations_connection_profiles_get(
        &self,
        args: &DatamigrationProjectsLocationsConnectionProfilesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ConnectionProfile, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_connection_profiles_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_connection_profiles_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations connection profiles get iam policy.
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
    pub fn datamigration_projects_locations_connection_profiles_get_iam_policy(
        &self,
        args: &DatamigrationProjectsLocationsConnectionProfilesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_connection_profiles_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_connection_profiles_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations connection profiles list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListConnectionProfilesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn datamigration_projects_locations_connection_profiles_list(
        &self,
        args: &DatamigrationProjectsLocationsConnectionProfilesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListConnectionProfilesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_connection_profiles_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_connection_profiles_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations connection profiles patch.
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
    pub fn datamigration_projects_locations_connection_profiles_patch(
        &self,
        args: &DatamigrationProjectsLocationsConnectionProfilesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_connection_profiles_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.skipValidation,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_connection_profiles_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations connection profiles set iam policy.
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
    pub fn datamigration_projects_locations_connection_profiles_set_iam_policy(
        &self,
        args: &DatamigrationProjectsLocationsConnectionProfilesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_connection_profiles_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_connection_profiles_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations connection profiles test iam permissions.
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
    pub fn datamigration_projects_locations_connection_profiles_test_iam_permissions(
        &self,
        args: &DatamigrationProjectsLocationsConnectionProfilesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_connection_profiles_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_connection_profiles_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations conversion workspaces apply.
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
    pub fn datamigration_projects_locations_conversion_workspaces_apply(
        &self,
        args: &DatamigrationProjectsLocationsConversionWorkspacesApplyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_conversion_workspaces_apply_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_conversion_workspaces_apply_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations conversion workspaces commit.
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
    pub fn datamigration_projects_locations_conversion_workspaces_commit(
        &self,
        args: &DatamigrationProjectsLocationsConversionWorkspacesCommitArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_conversion_workspaces_commit_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_conversion_workspaces_commit_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations conversion workspaces convert.
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
    pub fn datamigration_projects_locations_conversion_workspaces_convert(
        &self,
        args: &DatamigrationProjectsLocationsConversionWorkspacesConvertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_conversion_workspaces_convert_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_conversion_workspaces_convert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations conversion workspaces create.
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
    pub fn datamigration_projects_locations_conversion_workspaces_create(
        &self,
        args: &DatamigrationProjectsLocationsConversionWorkspacesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_conversion_workspaces_create_builder(
            &self.http_client,
            &args.parent,
            &args.conversionWorkspaceId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_conversion_workspaces_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations conversion workspaces delete.
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
    pub fn datamigration_projects_locations_conversion_workspaces_delete(
        &self,
        args: &DatamigrationProjectsLocationsConversionWorkspacesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_conversion_workspaces_delete_builder(
            &self.http_client,
            &args.name,
            &args.force,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_conversion_workspaces_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations conversion workspaces describe conversion workspace revisions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DescribeConversionWorkspaceRevisionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn datamigration_projects_locations_conversion_workspaces_describe_conversion_workspace_revisions(
        &self,
        args: &DatamigrationProjectsLocationsConversionWorkspacesDescribeConversionWorkspaceRevisionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DescribeConversionWorkspaceRevisionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_conversion_workspaces_describe_conversion_workspace_revisions_builder(
            &self.http_client,
            &args.conversionWorkspace,
            &args.commitId,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_conversion_workspaces_describe_conversion_workspace_revisions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations conversion workspaces describe database entities.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DescribeDatabaseEntitiesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn datamigration_projects_locations_conversion_workspaces_describe_database_entities(
        &self,
        args: &DatamigrationProjectsLocationsConversionWorkspacesDescribeDatabaseEntitiesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DescribeDatabaseEntitiesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_conversion_workspaces_describe_database_entities_builder(
            &self.http_client,
            &args.conversionWorkspace,
            &args.commitId,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.tree,
            &args.uncommitted,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_conversion_workspaces_describe_database_entities_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations conversion workspaces get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ConversionWorkspace result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn datamigration_projects_locations_conversion_workspaces_get(
        &self,
        args: &DatamigrationProjectsLocationsConversionWorkspacesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ConversionWorkspace, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_conversion_workspaces_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_conversion_workspaces_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations conversion workspaces get iam policy.
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
    pub fn datamigration_projects_locations_conversion_workspaces_get_iam_policy(
        &self,
        args: &DatamigrationProjectsLocationsConversionWorkspacesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_conversion_workspaces_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_conversion_workspaces_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations conversion workspaces list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListConversionWorkspacesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn datamigration_projects_locations_conversion_workspaces_list(
        &self,
        args: &DatamigrationProjectsLocationsConversionWorkspacesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListConversionWorkspacesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_conversion_workspaces_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_conversion_workspaces_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations conversion workspaces patch.
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
    pub fn datamigration_projects_locations_conversion_workspaces_patch(
        &self,
        args: &DatamigrationProjectsLocationsConversionWorkspacesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_conversion_workspaces_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_conversion_workspaces_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations conversion workspaces rollback.
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
    pub fn datamigration_projects_locations_conversion_workspaces_rollback(
        &self,
        args: &DatamigrationProjectsLocationsConversionWorkspacesRollbackArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_conversion_workspaces_rollback_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_conversion_workspaces_rollback_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations conversion workspaces search background jobs.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchBackgroundJobsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn datamigration_projects_locations_conversion_workspaces_search_background_jobs(
        &self,
        args: &DatamigrationProjectsLocationsConversionWorkspacesSearchBackgroundJobsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchBackgroundJobsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_conversion_workspaces_search_background_jobs_builder(
            &self.http_client,
            &args.conversionWorkspace,
            &args.completedUntilTime,
            &args.maxSize,
            &args.returnMostRecentPerJobType,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_conversion_workspaces_search_background_jobs_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations conversion workspaces seed.
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
    pub fn datamigration_projects_locations_conversion_workspaces_seed(
        &self,
        args: &DatamigrationProjectsLocationsConversionWorkspacesSeedArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_conversion_workspaces_seed_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_conversion_workspaces_seed_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations conversion workspaces set iam policy.
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
    pub fn datamigration_projects_locations_conversion_workspaces_set_iam_policy(
        &self,
        args: &DatamigrationProjectsLocationsConversionWorkspacesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_conversion_workspaces_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_conversion_workspaces_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations conversion workspaces test iam permissions.
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
    pub fn datamigration_projects_locations_conversion_workspaces_test_iam_permissions(
        &self,
        args: &DatamigrationProjectsLocationsConversionWorkspacesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_conversion_workspaces_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_conversion_workspaces_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations conversion workspaces mapping rules create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the MappingRule result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datamigration_projects_locations_conversion_workspaces_mapping_rules_create(
        &self,
        args: &DatamigrationProjectsLocationsConversionWorkspacesMappingRulesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<MappingRule, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_conversion_workspaces_mapping_rules_create_builder(
            &self.http_client,
            &args.parent,
            &args.mappingRuleId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_conversion_workspaces_mapping_rules_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations conversion workspaces mapping rules delete.
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
    pub fn datamigration_projects_locations_conversion_workspaces_mapping_rules_delete(
        &self,
        args: &DatamigrationProjectsLocationsConversionWorkspacesMappingRulesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_conversion_workspaces_mapping_rules_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_conversion_workspaces_mapping_rules_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations conversion workspaces mapping rules get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the MappingRule result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn datamigration_projects_locations_conversion_workspaces_mapping_rules_get(
        &self,
        args: &DatamigrationProjectsLocationsConversionWorkspacesMappingRulesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<MappingRule, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_conversion_workspaces_mapping_rules_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_conversion_workspaces_mapping_rules_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations conversion workspaces mapping rules import.
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
    pub fn datamigration_projects_locations_conversion_workspaces_mapping_rules_import(
        &self,
        args: &DatamigrationProjectsLocationsConversionWorkspacesMappingRulesImportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_conversion_workspaces_mapping_rules_import_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_conversion_workspaces_mapping_rules_import_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations conversion workspaces mapping rules list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListMappingRulesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn datamigration_projects_locations_conversion_workspaces_mapping_rules_list(
        &self,
        args: &DatamigrationProjectsLocationsConversionWorkspacesMappingRulesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListMappingRulesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_conversion_workspaces_mapping_rules_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_conversion_workspaces_mapping_rules_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations migration jobs create.
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
    pub fn datamigration_projects_locations_migration_jobs_create(
        &self,
        args: &DatamigrationProjectsLocationsMigrationJobsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_migration_jobs_create_builder(
            &self.http_client,
            &args.parent,
            &args.migrationJobId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_migration_jobs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations migration jobs delete.
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
    pub fn datamigration_projects_locations_migration_jobs_delete(
        &self,
        args: &DatamigrationProjectsLocationsMigrationJobsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_migration_jobs_delete_builder(
            &self.http_client,
            &args.name,
            &args.force,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_migration_jobs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations migration jobs demote destination.
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
    pub fn datamigration_projects_locations_migration_jobs_demote_destination(
        &self,
        args: &DatamigrationProjectsLocationsMigrationJobsDemoteDestinationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_migration_jobs_demote_destination_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_migration_jobs_demote_destination_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations migration jobs fetch source objects.
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
    pub fn datamigration_projects_locations_migration_jobs_fetch_source_objects(
        &self,
        args: &DatamigrationProjectsLocationsMigrationJobsFetchSourceObjectsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_migration_jobs_fetch_source_objects_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_migration_jobs_fetch_source_objects_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations migration jobs generate ssh script.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SshScript result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datamigration_projects_locations_migration_jobs_generate_ssh_script(
        &self,
        args: &DatamigrationProjectsLocationsMigrationJobsGenerateSshScriptArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SshScript, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_migration_jobs_generate_ssh_script_builder(
            &self.http_client,
            &args.migrationJob,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_migration_jobs_generate_ssh_script_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations migration jobs generate tcp proxy script.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TcpProxyScript result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datamigration_projects_locations_migration_jobs_generate_tcp_proxy_script(
        &self,
        args: &DatamigrationProjectsLocationsMigrationJobsGenerateTcpProxyScriptArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TcpProxyScript, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_migration_jobs_generate_tcp_proxy_script_builder(
            &self.http_client,
            &args.migrationJob,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_migration_jobs_generate_tcp_proxy_script_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations migration jobs get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the MigrationJob result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn datamigration_projects_locations_migration_jobs_get(
        &self,
        args: &DatamigrationProjectsLocationsMigrationJobsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<MigrationJob, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_migration_jobs_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_migration_jobs_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations migration jobs get iam policy.
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
    pub fn datamigration_projects_locations_migration_jobs_get_iam_policy(
        &self,
        args: &DatamigrationProjectsLocationsMigrationJobsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_migration_jobs_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_migration_jobs_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations migration jobs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListMigrationJobsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn datamigration_projects_locations_migration_jobs_list(
        &self,
        args: &DatamigrationProjectsLocationsMigrationJobsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListMigrationJobsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_migration_jobs_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_migration_jobs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations migration jobs patch.
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
    pub fn datamigration_projects_locations_migration_jobs_patch(
        &self,
        args: &DatamigrationProjectsLocationsMigrationJobsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_migration_jobs_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_migration_jobs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations migration jobs promote.
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
    pub fn datamigration_projects_locations_migration_jobs_promote(
        &self,
        args: &DatamigrationProjectsLocationsMigrationJobsPromoteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_migration_jobs_promote_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_migration_jobs_promote_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations migration jobs restart.
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
    pub fn datamigration_projects_locations_migration_jobs_restart(
        &self,
        args: &DatamigrationProjectsLocationsMigrationJobsRestartArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_migration_jobs_restart_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_migration_jobs_restart_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations migration jobs resume.
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
    pub fn datamigration_projects_locations_migration_jobs_resume(
        &self,
        args: &DatamigrationProjectsLocationsMigrationJobsResumeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_migration_jobs_resume_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_migration_jobs_resume_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations migration jobs set iam policy.
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
    pub fn datamigration_projects_locations_migration_jobs_set_iam_policy(
        &self,
        args: &DatamigrationProjectsLocationsMigrationJobsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_migration_jobs_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_migration_jobs_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations migration jobs start.
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
    pub fn datamigration_projects_locations_migration_jobs_start(
        &self,
        args: &DatamigrationProjectsLocationsMigrationJobsStartArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_migration_jobs_start_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_migration_jobs_start_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations migration jobs stop.
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
    pub fn datamigration_projects_locations_migration_jobs_stop(
        &self,
        args: &DatamigrationProjectsLocationsMigrationJobsStopArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_migration_jobs_stop_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_migration_jobs_stop_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations migration jobs test iam permissions.
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
    pub fn datamigration_projects_locations_migration_jobs_test_iam_permissions(
        &self,
        args: &DatamigrationProjectsLocationsMigrationJobsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_migration_jobs_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_migration_jobs_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations migration jobs verify.
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
    pub fn datamigration_projects_locations_migration_jobs_verify(
        &self,
        args: &DatamigrationProjectsLocationsMigrationJobsVerifyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_migration_jobs_verify_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_migration_jobs_verify_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations migration jobs objects get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the MigrationJobObject result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn datamigration_projects_locations_migration_jobs_objects_get(
        &self,
        args: &DatamigrationProjectsLocationsMigrationJobsObjectsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<MigrationJobObject, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_migration_jobs_objects_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_migration_jobs_objects_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations migration jobs objects get iam policy.
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
    pub fn datamigration_projects_locations_migration_jobs_objects_get_iam_policy(
        &self,
        args: &DatamigrationProjectsLocationsMigrationJobsObjectsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_migration_jobs_objects_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_migration_jobs_objects_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations migration jobs objects list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListMigrationJobObjectsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn datamigration_projects_locations_migration_jobs_objects_list(
        &self,
        args: &DatamigrationProjectsLocationsMigrationJobsObjectsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListMigrationJobObjectsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_migration_jobs_objects_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_migration_jobs_objects_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations migration jobs objects lookup.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the MigrationJobObject result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datamigration_projects_locations_migration_jobs_objects_lookup(
        &self,
        args: &DatamigrationProjectsLocationsMigrationJobsObjectsLookupArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<MigrationJobObject, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_migration_jobs_objects_lookup_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_migration_jobs_objects_lookup_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations migration jobs objects set iam policy.
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
    pub fn datamigration_projects_locations_migration_jobs_objects_set_iam_policy(
        &self,
        args: &DatamigrationProjectsLocationsMigrationJobsObjectsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_migration_jobs_objects_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_migration_jobs_objects_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations migration jobs objects test iam permissions.
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
    pub fn datamigration_projects_locations_migration_jobs_objects_test_iam_permissions(
        &self,
        args: &DatamigrationProjectsLocationsMigrationJobsObjectsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_migration_jobs_objects_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_migration_jobs_objects_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations operations cancel.
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
    pub fn datamigration_projects_locations_operations_cancel(
        &self,
        args: &DatamigrationProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations operations delete.
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
    pub fn datamigration_projects_locations_operations_delete(
        &self,
        args: &DatamigrationProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations operations get.
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
    pub fn datamigration_projects_locations_operations_get(
        &self,
        args: &DatamigrationProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations operations list.
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
    pub fn datamigration_projects_locations_operations_list(
        &self,
        args: &DatamigrationProjectsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations private connections create.
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
    pub fn datamigration_projects_locations_private_connections_create(
        &self,
        args: &DatamigrationProjectsLocationsPrivateConnectionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_private_connections_create_builder(
            &self.http_client,
            &args.parent,
            &args.privateConnectionId,
            &args.requestId,
            &args.skipValidation,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_private_connections_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations private connections delete.
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
    pub fn datamigration_projects_locations_private_connections_delete(
        &self,
        args: &DatamigrationProjectsLocationsPrivateConnectionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_private_connections_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_private_connections_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations private connections get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PrivateConnection result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn datamigration_projects_locations_private_connections_get(
        &self,
        args: &DatamigrationProjectsLocationsPrivateConnectionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PrivateConnection, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_private_connections_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_private_connections_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations private connections get iam policy.
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
    pub fn datamigration_projects_locations_private_connections_get_iam_policy(
        &self,
        args: &DatamigrationProjectsLocationsPrivateConnectionsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_private_connections_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_private_connections_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations private connections list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListPrivateConnectionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn datamigration_projects_locations_private_connections_list(
        &self,
        args: &DatamigrationProjectsLocationsPrivateConnectionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListPrivateConnectionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_private_connections_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_private_connections_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations private connections set iam policy.
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
    pub fn datamigration_projects_locations_private_connections_set_iam_policy(
        &self,
        args: &DatamigrationProjectsLocationsPrivateConnectionsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_private_connections_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_private_connections_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamigration projects locations private connections test iam permissions.
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
    pub fn datamigration_projects_locations_private_connections_test_iam_permissions(
        &self,
        args: &DatamigrationProjectsLocationsPrivateConnectionsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamigration_projects_locations_private_connections_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = datamigration_projects_locations_private_connections_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
