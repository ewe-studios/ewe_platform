//! DataformProvider - State-aware dataform API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       dataform API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::dataform::{
    dataform_projects_locations_get_builder, dataform_projects_locations_get_task,
    dataform_projects_locations_get_config_builder, dataform_projects_locations_get_config_task,
    dataform_projects_locations_list_builder, dataform_projects_locations_list_task,
    dataform_projects_locations_query_user_root_contents_builder, dataform_projects_locations_query_user_root_contents_task,
    dataform_projects_locations_update_config_builder, dataform_projects_locations_update_config_task,
    dataform_projects_locations_folders_create_builder, dataform_projects_locations_folders_create_task,
    dataform_projects_locations_folders_delete_builder, dataform_projects_locations_folders_delete_task,
    dataform_projects_locations_folders_delete_tree_builder, dataform_projects_locations_folders_delete_tree_task,
    dataform_projects_locations_folders_get_builder, dataform_projects_locations_folders_get_task,
    dataform_projects_locations_folders_get_iam_policy_builder, dataform_projects_locations_folders_get_iam_policy_task,
    dataform_projects_locations_folders_move_builder, dataform_projects_locations_folders_move_task,
    dataform_projects_locations_folders_patch_builder, dataform_projects_locations_folders_patch_task,
    dataform_projects_locations_folders_query_folder_contents_builder, dataform_projects_locations_folders_query_folder_contents_task,
    dataform_projects_locations_folders_set_iam_policy_builder, dataform_projects_locations_folders_set_iam_policy_task,
    dataform_projects_locations_folders_test_iam_permissions_builder, dataform_projects_locations_folders_test_iam_permissions_task,
    dataform_projects_locations_operations_cancel_builder, dataform_projects_locations_operations_cancel_task,
    dataform_projects_locations_operations_delete_builder, dataform_projects_locations_operations_delete_task,
    dataform_projects_locations_operations_get_builder, dataform_projects_locations_operations_get_task,
    dataform_projects_locations_operations_list_builder, dataform_projects_locations_operations_list_task,
    dataform_projects_locations_repositories_commit_builder, dataform_projects_locations_repositories_commit_task,
    dataform_projects_locations_repositories_compute_access_token_status_builder, dataform_projects_locations_repositories_compute_access_token_status_task,
    dataform_projects_locations_repositories_create_builder, dataform_projects_locations_repositories_create_task,
    dataform_projects_locations_repositories_delete_builder, dataform_projects_locations_repositories_delete_task,
    dataform_projects_locations_repositories_fetch_history_builder, dataform_projects_locations_repositories_fetch_history_task,
    dataform_projects_locations_repositories_fetch_remote_branches_builder, dataform_projects_locations_repositories_fetch_remote_branches_task,
    dataform_projects_locations_repositories_get_builder, dataform_projects_locations_repositories_get_task,
    dataform_projects_locations_repositories_get_iam_policy_builder, dataform_projects_locations_repositories_get_iam_policy_task,
    dataform_projects_locations_repositories_list_builder, dataform_projects_locations_repositories_list_task,
    dataform_projects_locations_repositories_move_builder, dataform_projects_locations_repositories_move_task,
    dataform_projects_locations_repositories_patch_builder, dataform_projects_locations_repositories_patch_task,
    dataform_projects_locations_repositories_query_directory_contents_builder, dataform_projects_locations_repositories_query_directory_contents_task,
    dataform_projects_locations_repositories_read_file_builder, dataform_projects_locations_repositories_read_file_task,
    dataform_projects_locations_repositories_set_iam_policy_builder, dataform_projects_locations_repositories_set_iam_policy_task,
    dataform_projects_locations_repositories_test_iam_permissions_builder, dataform_projects_locations_repositories_test_iam_permissions_task,
    dataform_projects_locations_repositories_compilation_results_create_builder, dataform_projects_locations_repositories_compilation_results_create_task,
    dataform_projects_locations_repositories_compilation_results_get_builder, dataform_projects_locations_repositories_compilation_results_get_task,
    dataform_projects_locations_repositories_compilation_results_list_builder, dataform_projects_locations_repositories_compilation_results_list_task,
    dataform_projects_locations_repositories_compilation_results_query_builder, dataform_projects_locations_repositories_compilation_results_query_task,
    dataform_projects_locations_repositories_release_configs_create_builder, dataform_projects_locations_repositories_release_configs_create_task,
    dataform_projects_locations_repositories_release_configs_delete_builder, dataform_projects_locations_repositories_release_configs_delete_task,
    dataform_projects_locations_repositories_release_configs_get_builder, dataform_projects_locations_repositories_release_configs_get_task,
    dataform_projects_locations_repositories_release_configs_list_builder, dataform_projects_locations_repositories_release_configs_list_task,
    dataform_projects_locations_repositories_release_configs_patch_builder, dataform_projects_locations_repositories_release_configs_patch_task,
    dataform_projects_locations_repositories_workflow_configs_create_builder, dataform_projects_locations_repositories_workflow_configs_create_task,
    dataform_projects_locations_repositories_workflow_configs_delete_builder, dataform_projects_locations_repositories_workflow_configs_delete_task,
    dataform_projects_locations_repositories_workflow_configs_get_builder, dataform_projects_locations_repositories_workflow_configs_get_task,
    dataform_projects_locations_repositories_workflow_configs_list_builder, dataform_projects_locations_repositories_workflow_configs_list_task,
    dataform_projects_locations_repositories_workflow_configs_patch_builder, dataform_projects_locations_repositories_workflow_configs_patch_task,
    dataform_projects_locations_repositories_workflow_invocations_cancel_builder, dataform_projects_locations_repositories_workflow_invocations_cancel_task,
    dataform_projects_locations_repositories_workflow_invocations_create_builder, dataform_projects_locations_repositories_workflow_invocations_create_task,
    dataform_projects_locations_repositories_workflow_invocations_delete_builder, dataform_projects_locations_repositories_workflow_invocations_delete_task,
    dataform_projects_locations_repositories_workflow_invocations_get_builder, dataform_projects_locations_repositories_workflow_invocations_get_task,
    dataform_projects_locations_repositories_workflow_invocations_list_builder, dataform_projects_locations_repositories_workflow_invocations_list_task,
    dataform_projects_locations_repositories_workflow_invocations_query_builder, dataform_projects_locations_repositories_workflow_invocations_query_task,
    dataform_projects_locations_repositories_workspaces_commit_builder, dataform_projects_locations_repositories_workspaces_commit_task,
    dataform_projects_locations_repositories_workspaces_create_builder, dataform_projects_locations_repositories_workspaces_create_task,
    dataform_projects_locations_repositories_workspaces_delete_builder, dataform_projects_locations_repositories_workspaces_delete_task,
    dataform_projects_locations_repositories_workspaces_fetch_file_diff_builder, dataform_projects_locations_repositories_workspaces_fetch_file_diff_task,
    dataform_projects_locations_repositories_workspaces_fetch_file_git_statuses_builder, dataform_projects_locations_repositories_workspaces_fetch_file_git_statuses_task,
    dataform_projects_locations_repositories_workspaces_fetch_git_ahead_behind_builder, dataform_projects_locations_repositories_workspaces_fetch_git_ahead_behind_task,
    dataform_projects_locations_repositories_workspaces_get_builder, dataform_projects_locations_repositories_workspaces_get_task,
    dataform_projects_locations_repositories_workspaces_get_iam_policy_builder, dataform_projects_locations_repositories_workspaces_get_iam_policy_task,
    dataform_projects_locations_repositories_workspaces_install_npm_packages_builder, dataform_projects_locations_repositories_workspaces_install_npm_packages_task,
    dataform_projects_locations_repositories_workspaces_list_builder, dataform_projects_locations_repositories_workspaces_list_task,
    dataform_projects_locations_repositories_workspaces_make_directory_builder, dataform_projects_locations_repositories_workspaces_make_directory_task,
    dataform_projects_locations_repositories_workspaces_move_directory_builder, dataform_projects_locations_repositories_workspaces_move_directory_task,
    dataform_projects_locations_repositories_workspaces_move_file_builder, dataform_projects_locations_repositories_workspaces_move_file_task,
    dataform_projects_locations_repositories_workspaces_pull_builder, dataform_projects_locations_repositories_workspaces_pull_task,
    dataform_projects_locations_repositories_workspaces_push_builder, dataform_projects_locations_repositories_workspaces_push_task,
    dataform_projects_locations_repositories_workspaces_query_directory_contents_builder, dataform_projects_locations_repositories_workspaces_query_directory_contents_task,
    dataform_projects_locations_repositories_workspaces_read_file_builder, dataform_projects_locations_repositories_workspaces_read_file_task,
    dataform_projects_locations_repositories_workspaces_remove_directory_builder, dataform_projects_locations_repositories_workspaces_remove_directory_task,
    dataform_projects_locations_repositories_workspaces_remove_file_builder, dataform_projects_locations_repositories_workspaces_remove_file_task,
    dataform_projects_locations_repositories_workspaces_reset_builder, dataform_projects_locations_repositories_workspaces_reset_task,
    dataform_projects_locations_repositories_workspaces_search_files_builder, dataform_projects_locations_repositories_workspaces_search_files_task,
    dataform_projects_locations_repositories_workspaces_set_iam_policy_builder, dataform_projects_locations_repositories_workspaces_set_iam_policy_task,
    dataform_projects_locations_repositories_workspaces_test_iam_permissions_builder, dataform_projects_locations_repositories_workspaces_test_iam_permissions_task,
    dataform_projects_locations_repositories_workspaces_write_file_builder, dataform_projects_locations_repositories_workspaces_write_file_task,
    dataform_projects_locations_team_folders_create_builder, dataform_projects_locations_team_folders_create_task,
    dataform_projects_locations_team_folders_delete_builder, dataform_projects_locations_team_folders_delete_task,
    dataform_projects_locations_team_folders_delete_tree_builder, dataform_projects_locations_team_folders_delete_tree_task,
    dataform_projects_locations_team_folders_get_builder, dataform_projects_locations_team_folders_get_task,
    dataform_projects_locations_team_folders_get_iam_policy_builder, dataform_projects_locations_team_folders_get_iam_policy_task,
    dataform_projects_locations_team_folders_patch_builder, dataform_projects_locations_team_folders_patch_task,
    dataform_projects_locations_team_folders_query_contents_builder, dataform_projects_locations_team_folders_query_contents_task,
    dataform_projects_locations_team_folders_search_builder, dataform_projects_locations_team_folders_search_task,
    dataform_projects_locations_team_folders_set_iam_policy_builder, dataform_projects_locations_team_folders_set_iam_policy_task,
    dataform_projects_locations_team_folders_test_iam_permissions_builder, dataform_projects_locations_team_folders_test_iam_permissions_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::dataform::CancelWorkflowInvocationResponse;
use crate::providers::gcp::clients::dataform::CommitRepositoryChangesResponse;
use crate::providers::gcp::clients::dataform::CommitWorkspaceChangesResponse;
use crate::providers::gcp::clients::dataform::CompilationResult;
use crate::providers::gcp::clients::dataform::ComputeRepositoryAccessTokenStatusResponse;
use crate::providers::gcp::clients::dataform::Config;
use crate::providers::gcp::clients::dataform::Empty;
use crate::providers::gcp::clients::dataform::FetchFileDiffResponse;
use crate::providers::gcp::clients::dataform::FetchFileGitStatusesResponse;
use crate::providers::gcp::clients::dataform::FetchGitAheadBehindResponse;
use crate::providers::gcp::clients::dataform::FetchRemoteBranchesResponse;
use crate::providers::gcp::clients::dataform::FetchRepositoryHistoryResponse;
use crate::providers::gcp::clients::dataform::Folder;
use crate::providers::gcp::clients::dataform::InstallNpmPackagesResponse;
use crate::providers::gcp::clients::dataform::ListCompilationResultsResponse;
use crate::providers::gcp::clients::dataform::ListLocationsResponse;
use crate::providers::gcp::clients::dataform::ListOperationsResponse;
use crate::providers::gcp::clients::dataform::ListReleaseConfigsResponse;
use crate::providers::gcp::clients::dataform::ListRepositoriesResponse;
use crate::providers::gcp::clients::dataform::ListWorkflowConfigsResponse;
use crate::providers::gcp::clients::dataform::ListWorkflowInvocationsResponse;
use crate::providers::gcp::clients::dataform::ListWorkspacesResponse;
use crate::providers::gcp::clients::dataform::Location;
use crate::providers::gcp::clients::dataform::MakeDirectoryResponse;
use crate::providers::gcp::clients::dataform::MoveDirectoryResponse;
use crate::providers::gcp::clients::dataform::MoveFileResponse;
use crate::providers::gcp::clients::dataform::Operation;
use crate::providers::gcp::clients::dataform::Policy;
use crate::providers::gcp::clients::dataform::PullGitCommitsResponse;
use crate::providers::gcp::clients::dataform::PushGitCommitsResponse;
use crate::providers::gcp::clients::dataform::QueryCompilationResultActionsResponse;
use crate::providers::gcp::clients::dataform::QueryDirectoryContentsResponse;
use crate::providers::gcp::clients::dataform::QueryFolderContentsResponse;
use crate::providers::gcp::clients::dataform::QueryRepositoryDirectoryContentsResponse;
use crate::providers::gcp::clients::dataform::QueryTeamFolderContentsResponse;
use crate::providers::gcp::clients::dataform::QueryUserRootContentsResponse;
use crate::providers::gcp::clients::dataform::QueryWorkflowInvocationActionsResponse;
use crate::providers::gcp::clients::dataform::ReadFileResponse;
use crate::providers::gcp::clients::dataform::ReadRepositoryFileResponse;
use crate::providers::gcp::clients::dataform::ReleaseConfig;
use crate::providers::gcp::clients::dataform::RemoveDirectoryResponse;
use crate::providers::gcp::clients::dataform::RemoveFileResponse;
use crate::providers::gcp::clients::dataform::Repository;
use crate::providers::gcp::clients::dataform::ResetWorkspaceChangesResponse;
use crate::providers::gcp::clients::dataform::SearchFilesResponse;
use crate::providers::gcp::clients::dataform::SearchTeamFoldersResponse;
use crate::providers::gcp::clients::dataform::TeamFolder;
use crate::providers::gcp::clients::dataform::TestIamPermissionsResponse;
use crate::providers::gcp::clients::dataform::WorkflowConfig;
use crate::providers::gcp::clients::dataform::WorkflowInvocation;
use crate::providers::gcp::clients::dataform::Workspace;
use crate::providers::gcp::clients::dataform::WriteFileResponse;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsFoldersCreateArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsFoldersDeleteArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsFoldersDeleteTreeArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsFoldersGetArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsFoldersGetIamPolicyArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsFoldersMoveArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsFoldersPatchArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsFoldersQueryFolderContentsArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsFoldersSetIamPolicyArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsFoldersTestIamPermissionsArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsGetArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsGetConfigArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsListArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsOperationsListArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsQueryUserRootContentsArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesCommitArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesCompilationResultsCreateArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesCompilationResultsGetArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesCompilationResultsListArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesCompilationResultsQueryArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesComputeAccessTokenStatusArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesCreateArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesDeleteArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesFetchHistoryArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesFetchRemoteBranchesArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesGetArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesGetIamPolicyArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesListArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesMoveArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesPatchArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesQueryDirectoryContentsArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesReadFileArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesReleaseConfigsCreateArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesReleaseConfigsDeleteArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesReleaseConfigsGetArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesReleaseConfigsListArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesReleaseConfigsPatchArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesSetIamPolicyArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesTestIamPermissionsArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesWorkflowConfigsCreateArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesWorkflowConfigsDeleteArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesWorkflowConfigsGetArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesWorkflowConfigsListArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesWorkflowConfigsPatchArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesWorkflowInvocationsCancelArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesWorkflowInvocationsCreateArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesWorkflowInvocationsDeleteArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesWorkflowInvocationsGetArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesWorkflowInvocationsListArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesWorkflowInvocationsQueryArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesWorkspacesCommitArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesWorkspacesCreateArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesWorkspacesDeleteArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesWorkspacesFetchFileDiffArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesWorkspacesFetchFileGitStatusesArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesWorkspacesFetchGitAheadBehindArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesWorkspacesGetArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesWorkspacesGetIamPolicyArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesWorkspacesInstallNpmPackagesArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesWorkspacesListArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesWorkspacesMakeDirectoryArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesWorkspacesMoveDirectoryArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesWorkspacesMoveFileArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesWorkspacesPullArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesWorkspacesPushArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesWorkspacesQueryDirectoryContentsArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesWorkspacesReadFileArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesWorkspacesRemoveDirectoryArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesWorkspacesRemoveFileArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesWorkspacesResetArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesWorkspacesSearchFilesArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesWorkspacesSetIamPolicyArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesWorkspacesTestIamPermissionsArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsRepositoriesWorkspacesWriteFileArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsTeamFoldersCreateArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsTeamFoldersDeleteArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsTeamFoldersDeleteTreeArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsTeamFoldersGetArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsTeamFoldersGetIamPolicyArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsTeamFoldersPatchArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsTeamFoldersQueryContentsArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsTeamFoldersSearchArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsTeamFoldersSetIamPolicyArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsTeamFoldersTestIamPermissionsArgs;
use crate::providers::gcp::clients::dataform::DataformProjectsLocationsUpdateConfigArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// DataformProvider with automatic state tracking.
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
/// let provider = DataformProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct DataformProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> DataformProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new DataformProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Dataform projects locations get.
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
    pub fn dataform_projects_locations_get(
        &self,
        args: &DataformProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations get config.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Config result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataform_projects_locations_get_config(
        &self,
        args: &DataformProjectsLocationsGetConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Config, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_get_config_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_get_config_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations list.
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
    pub fn dataform_projects_locations_list(
        &self,
        args: &DataformProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations query user root contents.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the QueryUserRootContentsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataform_projects_locations_query_user_root_contents(
        &self,
        args: &DataformProjectsLocationsQueryUserRootContentsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<QueryUserRootContentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_query_user_root_contents_builder(
            &self.http_client,
            &args.location,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_query_user_root_contents_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations update config.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Config result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataform_projects_locations_update_config(
        &self,
        args: &DataformProjectsLocationsUpdateConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Config, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_update_config_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_update_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations folders create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Folder result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataform_projects_locations_folders_create(
        &self,
        args: &DataformProjectsLocationsFoldersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Folder, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_folders_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_folders_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations folders delete.
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
    pub fn dataform_projects_locations_folders_delete(
        &self,
        args: &DataformProjectsLocationsFoldersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_folders_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_folders_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations folders delete tree.
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
    pub fn dataform_projects_locations_folders_delete_tree(
        &self,
        args: &DataformProjectsLocationsFoldersDeleteTreeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_folders_delete_tree_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_folders_delete_tree_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations folders get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Folder result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataform_projects_locations_folders_get(
        &self,
        args: &DataformProjectsLocationsFoldersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Folder, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_folders_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_folders_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations folders get iam policy.
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
    pub fn dataform_projects_locations_folders_get_iam_policy(
        &self,
        args: &DataformProjectsLocationsFoldersGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_folders_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_folders_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations folders move.
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
    pub fn dataform_projects_locations_folders_move(
        &self,
        args: &DataformProjectsLocationsFoldersMoveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_folders_move_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_folders_move_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations folders patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Folder result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataform_projects_locations_folders_patch(
        &self,
        args: &DataformProjectsLocationsFoldersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Folder, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_folders_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_folders_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations folders query folder contents.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the QueryFolderContentsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataform_projects_locations_folders_query_folder_contents(
        &self,
        args: &DataformProjectsLocationsFoldersQueryFolderContentsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<QueryFolderContentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_folders_query_folder_contents_builder(
            &self.http_client,
            &args.folder,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_folders_query_folder_contents_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations folders set iam policy.
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
    pub fn dataform_projects_locations_folders_set_iam_policy(
        &self,
        args: &DataformProjectsLocationsFoldersSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_folders_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_folders_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations folders test iam permissions.
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
    pub fn dataform_projects_locations_folders_test_iam_permissions(
        &self,
        args: &DataformProjectsLocationsFoldersTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_folders_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_folders_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations operations cancel.
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
    pub fn dataform_projects_locations_operations_cancel(
        &self,
        args: &DataformProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations operations delete.
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
    pub fn dataform_projects_locations_operations_delete(
        &self,
        args: &DataformProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations operations get.
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
    pub fn dataform_projects_locations_operations_get(
        &self,
        args: &DataformProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations operations list.
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
    pub fn dataform_projects_locations_operations_list(
        &self,
        args: &DataformProjectsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories commit.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CommitRepositoryChangesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataform_projects_locations_repositories_commit(
        &self,
        args: &DataformProjectsLocationsRepositoriesCommitArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CommitRepositoryChangesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_commit_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_commit_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories compute access token status.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ComputeRepositoryAccessTokenStatusResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataform_projects_locations_repositories_compute_access_token_status(
        &self,
        args: &DataformProjectsLocationsRepositoriesComputeAccessTokenStatusArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ComputeRepositoryAccessTokenStatusResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_compute_access_token_status_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_compute_access_token_status_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Repository result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataform_projects_locations_repositories_create(
        &self,
        args: &DataformProjectsLocationsRepositoriesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Repository, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_create_builder(
            &self.http_client,
            &args.parent,
            &args.repositoryId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories delete.
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
    pub fn dataform_projects_locations_repositories_delete(
        &self,
        args: &DataformProjectsLocationsRepositoriesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_delete_builder(
            &self.http_client,
            &args.name,
            &args.force,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories fetch history.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FetchRepositoryHistoryResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataform_projects_locations_repositories_fetch_history(
        &self,
        args: &DataformProjectsLocationsRepositoriesFetchHistoryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FetchRepositoryHistoryResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_fetch_history_builder(
            &self.http_client,
            &args.name,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_fetch_history_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories fetch remote branches.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FetchRemoteBranchesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataform_projects_locations_repositories_fetch_remote_branches(
        &self,
        args: &DataformProjectsLocationsRepositoriesFetchRemoteBranchesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FetchRemoteBranchesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_fetch_remote_branches_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_fetch_remote_branches_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Repository result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataform_projects_locations_repositories_get(
        &self,
        args: &DataformProjectsLocationsRepositoriesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Repository, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories get iam policy.
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
    pub fn dataform_projects_locations_repositories_get_iam_policy(
        &self,
        args: &DataformProjectsLocationsRepositoriesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListRepositoriesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataform_projects_locations_repositories_list(
        &self,
        args: &DataformProjectsLocationsRepositoriesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListRepositoriesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories move.
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
    pub fn dataform_projects_locations_repositories_move(
        &self,
        args: &DataformProjectsLocationsRepositoriesMoveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_move_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_move_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Repository result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataform_projects_locations_repositories_patch(
        &self,
        args: &DataformProjectsLocationsRepositoriesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Repository, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories query directory contents.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the QueryRepositoryDirectoryContentsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataform_projects_locations_repositories_query_directory_contents(
        &self,
        args: &DataformProjectsLocationsRepositoriesQueryDirectoryContentsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<QueryRepositoryDirectoryContentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_query_directory_contents_builder(
            &self.http_client,
            &args.name,
            &args.commitSha,
            &args.pageSize,
            &args.pageToken,
            &args.path,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_query_directory_contents_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories read file.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ReadRepositoryFileResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataform_projects_locations_repositories_read_file(
        &self,
        args: &DataformProjectsLocationsRepositoriesReadFileArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ReadRepositoryFileResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_read_file_builder(
            &self.http_client,
            &args.name,
            &args.commitSha,
            &args.path,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_read_file_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories set iam policy.
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
    pub fn dataform_projects_locations_repositories_set_iam_policy(
        &self,
        args: &DataformProjectsLocationsRepositoriesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories test iam permissions.
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
    pub fn dataform_projects_locations_repositories_test_iam_permissions(
        &self,
        args: &DataformProjectsLocationsRepositoriesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories compilation results create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CompilationResult result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataform_projects_locations_repositories_compilation_results_create(
        &self,
        args: &DataformProjectsLocationsRepositoriesCompilationResultsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CompilationResult, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_compilation_results_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_compilation_results_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories compilation results get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CompilationResult result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataform_projects_locations_repositories_compilation_results_get(
        &self,
        args: &DataformProjectsLocationsRepositoriesCompilationResultsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CompilationResult, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_compilation_results_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_compilation_results_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories compilation results list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCompilationResultsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataform_projects_locations_repositories_compilation_results_list(
        &self,
        args: &DataformProjectsLocationsRepositoriesCompilationResultsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCompilationResultsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_compilation_results_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_compilation_results_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories compilation results query.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the QueryCompilationResultActionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataform_projects_locations_repositories_compilation_results_query(
        &self,
        args: &DataformProjectsLocationsRepositoriesCompilationResultsQueryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<QueryCompilationResultActionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_compilation_results_query_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_compilation_results_query_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories release configs create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ReleaseConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataform_projects_locations_repositories_release_configs_create(
        &self,
        args: &DataformProjectsLocationsRepositoriesReleaseConfigsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ReleaseConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_release_configs_create_builder(
            &self.http_client,
            &args.parent,
            &args.releaseConfigId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_release_configs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories release configs delete.
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
    pub fn dataform_projects_locations_repositories_release_configs_delete(
        &self,
        args: &DataformProjectsLocationsRepositoriesReleaseConfigsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_release_configs_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_release_configs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories release configs get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ReleaseConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataform_projects_locations_repositories_release_configs_get(
        &self,
        args: &DataformProjectsLocationsRepositoriesReleaseConfigsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ReleaseConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_release_configs_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_release_configs_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories release configs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListReleaseConfigsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataform_projects_locations_repositories_release_configs_list(
        &self,
        args: &DataformProjectsLocationsRepositoriesReleaseConfigsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListReleaseConfigsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_release_configs_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_release_configs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories release configs patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ReleaseConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataform_projects_locations_repositories_release_configs_patch(
        &self,
        args: &DataformProjectsLocationsRepositoriesReleaseConfigsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ReleaseConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_release_configs_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_release_configs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories workflow configs create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WorkflowConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataform_projects_locations_repositories_workflow_configs_create(
        &self,
        args: &DataformProjectsLocationsRepositoriesWorkflowConfigsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WorkflowConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_workflow_configs_create_builder(
            &self.http_client,
            &args.parent,
            &args.workflowConfigId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_workflow_configs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories workflow configs delete.
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
    pub fn dataform_projects_locations_repositories_workflow_configs_delete(
        &self,
        args: &DataformProjectsLocationsRepositoriesWorkflowConfigsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_workflow_configs_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_workflow_configs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories workflow configs get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WorkflowConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataform_projects_locations_repositories_workflow_configs_get(
        &self,
        args: &DataformProjectsLocationsRepositoriesWorkflowConfigsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WorkflowConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_workflow_configs_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_workflow_configs_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories workflow configs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListWorkflowConfigsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataform_projects_locations_repositories_workflow_configs_list(
        &self,
        args: &DataformProjectsLocationsRepositoriesWorkflowConfigsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListWorkflowConfigsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_workflow_configs_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_workflow_configs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories workflow configs patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WorkflowConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataform_projects_locations_repositories_workflow_configs_patch(
        &self,
        args: &DataformProjectsLocationsRepositoriesWorkflowConfigsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WorkflowConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_workflow_configs_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_workflow_configs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories workflow invocations cancel.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CancelWorkflowInvocationResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataform_projects_locations_repositories_workflow_invocations_cancel(
        &self,
        args: &DataformProjectsLocationsRepositoriesWorkflowInvocationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CancelWorkflowInvocationResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_workflow_invocations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_workflow_invocations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories workflow invocations create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WorkflowInvocation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataform_projects_locations_repositories_workflow_invocations_create(
        &self,
        args: &DataformProjectsLocationsRepositoriesWorkflowInvocationsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WorkflowInvocation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_workflow_invocations_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_workflow_invocations_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories workflow invocations delete.
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
    pub fn dataform_projects_locations_repositories_workflow_invocations_delete(
        &self,
        args: &DataformProjectsLocationsRepositoriesWorkflowInvocationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_workflow_invocations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_workflow_invocations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories workflow invocations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WorkflowInvocation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataform_projects_locations_repositories_workflow_invocations_get(
        &self,
        args: &DataformProjectsLocationsRepositoriesWorkflowInvocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WorkflowInvocation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_workflow_invocations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_workflow_invocations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories workflow invocations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListWorkflowInvocationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataform_projects_locations_repositories_workflow_invocations_list(
        &self,
        args: &DataformProjectsLocationsRepositoriesWorkflowInvocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListWorkflowInvocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_workflow_invocations_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_workflow_invocations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories workflow invocations query.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the QueryWorkflowInvocationActionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataform_projects_locations_repositories_workflow_invocations_query(
        &self,
        args: &DataformProjectsLocationsRepositoriesWorkflowInvocationsQueryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<QueryWorkflowInvocationActionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_workflow_invocations_query_builder(
            &self.http_client,
            &args.name,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_workflow_invocations_query_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories workspaces commit.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CommitWorkspaceChangesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataform_projects_locations_repositories_workspaces_commit(
        &self,
        args: &DataformProjectsLocationsRepositoriesWorkspacesCommitArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CommitWorkspaceChangesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_workspaces_commit_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_workspaces_commit_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories workspaces create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Workspace result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataform_projects_locations_repositories_workspaces_create(
        &self,
        args: &DataformProjectsLocationsRepositoriesWorkspacesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Workspace, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_workspaces_create_builder(
            &self.http_client,
            &args.parent,
            &args.workspaceId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_workspaces_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories workspaces delete.
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
    pub fn dataform_projects_locations_repositories_workspaces_delete(
        &self,
        args: &DataformProjectsLocationsRepositoriesWorkspacesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_workspaces_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_workspaces_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories workspaces fetch file diff.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FetchFileDiffResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataform_projects_locations_repositories_workspaces_fetch_file_diff(
        &self,
        args: &DataformProjectsLocationsRepositoriesWorkspacesFetchFileDiffArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FetchFileDiffResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_workspaces_fetch_file_diff_builder(
            &self.http_client,
            &args.workspace,
            &args.path,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_workspaces_fetch_file_diff_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories workspaces fetch file git statuses.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FetchFileGitStatusesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataform_projects_locations_repositories_workspaces_fetch_file_git_statuses(
        &self,
        args: &DataformProjectsLocationsRepositoriesWorkspacesFetchFileGitStatusesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FetchFileGitStatusesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_workspaces_fetch_file_git_statuses_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_workspaces_fetch_file_git_statuses_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories workspaces fetch git ahead behind.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FetchGitAheadBehindResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataform_projects_locations_repositories_workspaces_fetch_git_ahead_behind(
        &self,
        args: &DataformProjectsLocationsRepositoriesWorkspacesFetchGitAheadBehindArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FetchGitAheadBehindResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_workspaces_fetch_git_ahead_behind_builder(
            &self.http_client,
            &args.name,
            &args.remoteBranch,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_workspaces_fetch_git_ahead_behind_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories workspaces get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Workspace result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataform_projects_locations_repositories_workspaces_get(
        &self,
        args: &DataformProjectsLocationsRepositoriesWorkspacesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Workspace, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_workspaces_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_workspaces_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories workspaces get iam policy.
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
    pub fn dataform_projects_locations_repositories_workspaces_get_iam_policy(
        &self,
        args: &DataformProjectsLocationsRepositoriesWorkspacesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_workspaces_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_workspaces_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories workspaces install npm packages.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the InstallNpmPackagesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataform_projects_locations_repositories_workspaces_install_npm_packages(
        &self,
        args: &DataformProjectsLocationsRepositoriesWorkspacesInstallNpmPackagesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<InstallNpmPackagesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_workspaces_install_npm_packages_builder(
            &self.http_client,
            &args.workspace,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_workspaces_install_npm_packages_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories workspaces list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListWorkspacesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataform_projects_locations_repositories_workspaces_list(
        &self,
        args: &DataformProjectsLocationsRepositoriesWorkspacesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListWorkspacesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_workspaces_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_workspaces_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories workspaces make directory.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the MakeDirectoryResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataform_projects_locations_repositories_workspaces_make_directory(
        &self,
        args: &DataformProjectsLocationsRepositoriesWorkspacesMakeDirectoryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<MakeDirectoryResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_workspaces_make_directory_builder(
            &self.http_client,
            &args.workspace,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_workspaces_make_directory_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories workspaces move directory.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the MoveDirectoryResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataform_projects_locations_repositories_workspaces_move_directory(
        &self,
        args: &DataformProjectsLocationsRepositoriesWorkspacesMoveDirectoryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<MoveDirectoryResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_workspaces_move_directory_builder(
            &self.http_client,
            &args.workspace,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_workspaces_move_directory_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories workspaces move file.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the MoveFileResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataform_projects_locations_repositories_workspaces_move_file(
        &self,
        args: &DataformProjectsLocationsRepositoriesWorkspacesMoveFileArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<MoveFileResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_workspaces_move_file_builder(
            &self.http_client,
            &args.workspace,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_workspaces_move_file_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories workspaces pull.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PullGitCommitsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataform_projects_locations_repositories_workspaces_pull(
        &self,
        args: &DataformProjectsLocationsRepositoriesWorkspacesPullArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PullGitCommitsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_workspaces_pull_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_workspaces_pull_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories workspaces push.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PushGitCommitsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataform_projects_locations_repositories_workspaces_push(
        &self,
        args: &DataformProjectsLocationsRepositoriesWorkspacesPushArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PushGitCommitsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_workspaces_push_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_workspaces_push_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories workspaces query directory contents.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the QueryDirectoryContentsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataform_projects_locations_repositories_workspaces_query_directory_contents(
        &self,
        args: &DataformProjectsLocationsRepositoriesWorkspacesQueryDirectoryContentsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<QueryDirectoryContentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_workspaces_query_directory_contents_builder(
            &self.http_client,
            &args.workspace,
            &args.pageSize,
            &args.pageToken,
            &args.path,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_workspaces_query_directory_contents_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories workspaces read file.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ReadFileResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataform_projects_locations_repositories_workspaces_read_file(
        &self,
        args: &DataformProjectsLocationsRepositoriesWorkspacesReadFileArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ReadFileResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_workspaces_read_file_builder(
            &self.http_client,
            &args.workspace,
            &args.path,
            &args.revision,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_workspaces_read_file_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories workspaces remove directory.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RemoveDirectoryResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataform_projects_locations_repositories_workspaces_remove_directory(
        &self,
        args: &DataformProjectsLocationsRepositoriesWorkspacesRemoveDirectoryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RemoveDirectoryResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_workspaces_remove_directory_builder(
            &self.http_client,
            &args.workspace,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_workspaces_remove_directory_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories workspaces remove file.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RemoveFileResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataform_projects_locations_repositories_workspaces_remove_file(
        &self,
        args: &DataformProjectsLocationsRepositoriesWorkspacesRemoveFileArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RemoveFileResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_workspaces_remove_file_builder(
            &self.http_client,
            &args.workspace,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_workspaces_remove_file_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories workspaces reset.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ResetWorkspaceChangesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataform_projects_locations_repositories_workspaces_reset(
        &self,
        args: &DataformProjectsLocationsRepositoriesWorkspacesResetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ResetWorkspaceChangesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_workspaces_reset_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_workspaces_reset_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories workspaces search files.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchFilesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataform_projects_locations_repositories_workspaces_search_files(
        &self,
        args: &DataformProjectsLocationsRepositoriesWorkspacesSearchFilesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchFilesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_workspaces_search_files_builder(
            &self.http_client,
            &args.workspace,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_workspaces_search_files_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories workspaces set iam policy.
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
    pub fn dataform_projects_locations_repositories_workspaces_set_iam_policy(
        &self,
        args: &DataformProjectsLocationsRepositoriesWorkspacesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_workspaces_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_workspaces_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories workspaces test iam permissions.
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
    pub fn dataform_projects_locations_repositories_workspaces_test_iam_permissions(
        &self,
        args: &DataformProjectsLocationsRepositoriesWorkspacesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_workspaces_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_workspaces_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations repositories workspaces write file.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WriteFileResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataform_projects_locations_repositories_workspaces_write_file(
        &self,
        args: &DataformProjectsLocationsRepositoriesWorkspacesWriteFileArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WriteFileResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_repositories_workspaces_write_file_builder(
            &self.http_client,
            &args.workspace,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_repositories_workspaces_write_file_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations team folders create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TeamFolder result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataform_projects_locations_team_folders_create(
        &self,
        args: &DataformProjectsLocationsTeamFoldersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TeamFolder, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_team_folders_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_team_folders_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations team folders delete.
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
    pub fn dataform_projects_locations_team_folders_delete(
        &self,
        args: &DataformProjectsLocationsTeamFoldersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_team_folders_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_team_folders_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations team folders delete tree.
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
    pub fn dataform_projects_locations_team_folders_delete_tree(
        &self,
        args: &DataformProjectsLocationsTeamFoldersDeleteTreeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_team_folders_delete_tree_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_team_folders_delete_tree_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations team folders get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TeamFolder result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataform_projects_locations_team_folders_get(
        &self,
        args: &DataformProjectsLocationsTeamFoldersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TeamFolder, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_team_folders_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_team_folders_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations team folders get iam policy.
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
    pub fn dataform_projects_locations_team_folders_get_iam_policy(
        &self,
        args: &DataformProjectsLocationsTeamFoldersGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_team_folders_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_team_folders_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations team folders patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TeamFolder result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataform_projects_locations_team_folders_patch(
        &self,
        args: &DataformProjectsLocationsTeamFoldersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TeamFolder, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_team_folders_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_team_folders_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations team folders query contents.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the QueryTeamFolderContentsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataform_projects_locations_team_folders_query_contents(
        &self,
        args: &DataformProjectsLocationsTeamFoldersQueryContentsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<QueryTeamFolderContentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_team_folders_query_contents_builder(
            &self.http_client,
            &args.teamFolder,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_team_folders_query_contents_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations team folders search.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchTeamFoldersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataform_projects_locations_team_folders_search(
        &self,
        args: &DataformProjectsLocationsTeamFoldersSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchTeamFoldersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_team_folders_search_builder(
            &self.http_client,
            &args.location,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_team_folders_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations team folders set iam policy.
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
    pub fn dataform_projects_locations_team_folders_set_iam_policy(
        &self,
        args: &DataformProjectsLocationsTeamFoldersSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_team_folders_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_team_folders_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataform projects locations team folders test iam permissions.
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
    pub fn dataform_projects_locations_team_folders_test_iam_permissions(
        &self,
        args: &DataformProjectsLocationsTeamFoldersTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataform_projects_locations_team_folders_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataform_projects_locations_team_folders_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
