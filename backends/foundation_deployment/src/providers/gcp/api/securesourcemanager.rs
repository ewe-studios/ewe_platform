//! SecuresourcemanagerProvider - State-aware securesourcemanager API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       securesourcemanager API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::securesourcemanager::{
    securesourcemanager_projects_locations_get_builder, securesourcemanager_projects_locations_get_task,
    securesourcemanager_projects_locations_list_builder, securesourcemanager_projects_locations_list_task,
    securesourcemanager_projects_locations_instances_create_builder, securesourcemanager_projects_locations_instances_create_task,
    securesourcemanager_projects_locations_instances_delete_builder, securesourcemanager_projects_locations_instances_delete_task,
    securesourcemanager_projects_locations_instances_get_builder, securesourcemanager_projects_locations_instances_get_task,
    securesourcemanager_projects_locations_instances_get_iam_policy_builder, securesourcemanager_projects_locations_instances_get_iam_policy_task,
    securesourcemanager_projects_locations_instances_list_builder, securesourcemanager_projects_locations_instances_list_task,
    securesourcemanager_projects_locations_instances_set_iam_policy_builder, securesourcemanager_projects_locations_instances_set_iam_policy_task,
    securesourcemanager_projects_locations_instances_test_iam_permissions_builder, securesourcemanager_projects_locations_instances_test_iam_permissions_task,
    securesourcemanager_projects_locations_operations_cancel_builder, securesourcemanager_projects_locations_operations_cancel_task,
    securesourcemanager_projects_locations_operations_delete_builder, securesourcemanager_projects_locations_operations_delete_task,
    securesourcemanager_projects_locations_operations_get_builder, securesourcemanager_projects_locations_operations_get_task,
    securesourcemanager_projects_locations_operations_list_builder, securesourcemanager_projects_locations_operations_list_task,
    securesourcemanager_projects_locations_repositories_create_builder, securesourcemanager_projects_locations_repositories_create_task,
    securesourcemanager_projects_locations_repositories_delete_builder, securesourcemanager_projects_locations_repositories_delete_task,
    securesourcemanager_projects_locations_repositories_fetch_blob_builder, securesourcemanager_projects_locations_repositories_fetch_blob_task,
    securesourcemanager_projects_locations_repositories_fetch_tree_builder, securesourcemanager_projects_locations_repositories_fetch_tree_task,
    securesourcemanager_projects_locations_repositories_get_builder, securesourcemanager_projects_locations_repositories_get_task,
    securesourcemanager_projects_locations_repositories_get_iam_policy_builder, securesourcemanager_projects_locations_repositories_get_iam_policy_task,
    securesourcemanager_projects_locations_repositories_list_builder, securesourcemanager_projects_locations_repositories_list_task,
    securesourcemanager_projects_locations_repositories_patch_builder, securesourcemanager_projects_locations_repositories_patch_task,
    securesourcemanager_projects_locations_repositories_set_iam_policy_builder, securesourcemanager_projects_locations_repositories_set_iam_policy_task,
    securesourcemanager_projects_locations_repositories_test_iam_permissions_builder, securesourcemanager_projects_locations_repositories_test_iam_permissions_task,
    securesourcemanager_projects_locations_repositories_branch_rules_create_builder, securesourcemanager_projects_locations_repositories_branch_rules_create_task,
    securesourcemanager_projects_locations_repositories_branch_rules_delete_builder, securesourcemanager_projects_locations_repositories_branch_rules_delete_task,
    securesourcemanager_projects_locations_repositories_branch_rules_get_builder, securesourcemanager_projects_locations_repositories_branch_rules_get_task,
    securesourcemanager_projects_locations_repositories_branch_rules_list_builder, securesourcemanager_projects_locations_repositories_branch_rules_list_task,
    securesourcemanager_projects_locations_repositories_branch_rules_patch_builder, securesourcemanager_projects_locations_repositories_branch_rules_patch_task,
    securesourcemanager_projects_locations_repositories_hooks_create_builder, securesourcemanager_projects_locations_repositories_hooks_create_task,
    securesourcemanager_projects_locations_repositories_hooks_delete_builder, securesourcemanager_projects_locations_repositories_hooks_delete_task,
    securesourcemanager_projects_locations_repositories_hooks_get_builder, securesourcemanager_projects_locations_repositories_hooks_get_task,
    securesourcemanager_projects_locations_repositories_hooks_list_builder, securesourcemanager_projects_locations_repositories_hooks_list_task,
    securesourcemanager_projects_locations_repositories_hooks_patch_builder, securesourcemanager_projects_locations_repositories_hooks_patch_task,
    securesourcemanager_projects_locations_repositories_issues_close_builder, securesourcemanager_projects_locations_repositories_issues_close_task,
    securesourcemanager_projects_locations_repositories_issues_create_builder, securesourcemanager_projects_locations_repositories_issues_create_task,
    securesourcemanager_projects_locations_repositories_issues_delete_builder, securesourcemanager_projects_locations_repositories_issues_delete_task,
    securesourcemanager_projects_locations_repositories_issues_get_builder, securesourcemanager_projects_locations_repositories_issues_get_task,
    securesourcemanager_projects_locations_repositories_issues_list_builder, securesourcemanager_projects_locations_repositories_issues_list_task,
    securesourcemanager_projects_locations_repositories_issues_open_builder, securesourcemanager_projects_locations_repositories_issues_open_task,
    securesourcemanager_projects_locations_repositories_issues_patch_builder, securesourcemanager_projects_locations_repositories_issues_patch_task,
    securesourcemanager_projects_locations_repositories_issues_issue_comments_create_builder, securesourcemanager_projects_locations_repositories_issues_issue_comments_create_task,
    securesourcemanager_projects_locations_repositories_issues_issue_comments_delete_builder, securesourcemanager_projects_locations_repositories_issues_issue_comments_delete_task,
    securesourcemanager_projects_locations_repositories_issues_issue_comments_get_builder, securesourcemanager_projects_locations_repositories_issues_issue_comments_get_task,
    securesourcemanager_projects_locations_repositories_issues_issue_comments_list_builder, securesourcemanager_projects_locations_repositories_issues_issue_comments_list_task,
    securesourcemanager_projects_locations_repositories_issues_issue_comments_patch_builder, securesourcemanager_projects_locations_repositories_issues_issue_comments_patch_task,
    securesourcemanager_projects_locations_repositories_pull_requests_close_builder, securesourcemanager_projects_locations_repositories_pull_requests_close_task,
    securesourcemanager_projects_locations_repositories_pull_requests_create_builder, securesourcemanager_projects_locations_repositories_pull_requests_create_task,
    securesourcemanager_projects_locations_repositories_pull_requests_get_builder, securesourcemanager_projects_locations_repositories_pull_requests_get_task,
    securesourcemanager_projects_locations_repositories_pull_requests_list_builder, securesourcemanager_projects_locations_repositories_pull_requests_list_task,
    securesourcemanager_projects_locations_repositories_pull_requests_list_file_diffs_builder, securesourcemanager_projects_locations_repositories_pull_requests_list_file_diffs_task,
    securesourcemanager_projects_locations_repositories_pull_requests_merge_builder, securesourcemanager_projects_locations_repositories_pull_requests_merge_task,
    securesourcemanager_projects_locations_repositories_pull_requests_open_builder, securesourcemanager_projects_locations_repositories_pull_requests_open_task,
    securesourcemanager_projects_locations_repositories_pull_requests_patch_builder, securesourcemanager_projects_locations_repositories_pull_requests_patch_task,
    securesourcemanager_projects_locations_repositories_pull_requests_pull_request_comments_batch_create_builder, securesourcemanager_projects_locations_repositories_pull_requests_pull_request_comments_batch_create_task,
    securesourcemanager_projects_locations_repositories_pull_requests_pull_request_comments_create_builder, securesourcemanager_projects_locations_repositories_pull_requests_pull_request_comments_create_task,
    securesourcemanager_projects_locations_repositories_pull_requests_pull_request_comments_delete_builder, securesourcemanager_projects_locations_repositories_pull_requests_pull_request_comments_delete_task,
    securesourcemanager_projects_locations_repositories_pull_requests_pull_request_comments_get_builder, securesourcemanager_projects_locations_repositories_pull_requests_pull_request_comments_get_task,
    securesourcemanager_projects_locations_repositories_pull_requests_pull_request_comments_list_builder, securesourcemanager_projects_locations_repositories_pull_requests_pull_request_comments_list_task,
    securesourcemanager_projects_locations_repositories_pull_requests_pull_request_comments_patch_builder, securesourcemanager_projects_locations_repositories_pull_requests_pull_request_comments_patch_task,
    securesourcemanager_projects_locations_repositories_pull_requests_pull_request_comments_resolve_builder, securesourcemanager_projects_locations_repositories_pull_requests_pull_request_comments_resolve_task,
    securesourcemanager_projects_locations_repositories_pull_requests_pull_request_comments_unresolve_builder, securesourcemanager_projects_locations_repositories_pull_requests_pull_request_comments_unresolve_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::securesourcemanager::BranchRule;
use crate::providers::gcp::clients::securesourcemanager::Empty;
use crate::providers::gcp::clients::securesourcemanager::FetchBlobResponse;
use crate::providers::gcp::clients::securesourcemanager::FetchTreeResponse;
use crate::providers::gcp::clients::securesourcemanager::Hook;
use crate::providers::gcp::clients::securesourcemanager::Instance;
use crate::providers::gcp::clients::securesourcemanager::Issue;
use crate::providers::gcp::clients::securesourcemanager::IssueComment;
use crate::providers::gcp::clients::securesourcemanager::ListBranchRulesResponse;
use crate::providers::gcp::clients::securesourcemanager::ListHooksResponse;
use crate::providers::gcp::clients::securesourcemanager::ListInstancesResponse;
use crate::providers::gcp::clients::securesourcemanager::ListIssueCommentsResponse;
use crate::providers::gcp::clients::securesourcemanager::ListIssuesResponse;
use crate::providers::gcp::clients::securesourcemanager::ListLocationsResponse;
use crate::providers::gcp::clients::securesourcemanager::ListOperationsResponse;
use crate::providers::gcp::clients::securesourcemanager::ListPullRequestCommentsResponse;
use crate::providers::gcp::clients::securesourcemanager::ListPullRequestFileDiffsResponse;
use crate::providers::gcp::clients::securesourcemanager::ListPullRequestsResponse;
use crate::providers::gcp::clients::securesourcemanager::ListRepositoriesResponse;
use crate::providers::gcp::clients::securesourcemanager::Location;
use crate::providers::gcp::clients::securesourcemanager::Operation;
use crate::providers::gcp::clients::securesourcemanager::Policy;
use crate::providers::gcp::clients::securesourcemanager::PullRequest;
use crate::providers::gcp::clients::securesourcemanager::PullRequestComment;
use crate::providers::gcp::clients::securesourcemanager::Repository;
use crate::providers::gcp::clients::securesourcemanager::TestIamPermissionsResponse;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsGetArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsInstancesCreateArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsInstancesDeleteArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsInstancesGetArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsInstancesGetIamPolicyArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsInstancesListArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsInstancesSetIamPolicyArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsInstancesTestIamPermissionsArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsListArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsOperationsListArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsRepositoriesBranchRulesCreateArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsRepositoriesBranchRulesDeleteArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsRepositoriesBranchRulesGetArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsRepositoriesBranchRulesListArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsRepositoriesBranchRulesPatchArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsRepositoriesCreateArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsRepositoriesDeleteArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsRepositoriesFetchBlobArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsRepositoriesFetchTreeArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsRepositoriesGetArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsRepositoriesGetIamPolicyArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsRepositoriesHooksCreateArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsRepositoriesHooksDeleteArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsRepositoriesHooksGetArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsRepositoriesHooksListArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsRepositoriesHooksPatchArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsRepositoriesIssuesCloseArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsRepositoriesIssuesCreateArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsRepositoriesIssuesDeleteArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsRepositoriesIssuesGetArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsRepositoriesIssuesIssueCommentsCreateArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsRepositoriesIssuesIssueCommentsDeleteArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsRepositoriesIssuesIssueCommentsGetArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsRepositoriesIssuesIssueCommentsListArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsRepositoriesIssuesIssueCommentsPatchArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsRepositoriesIssuesListArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsRepositoriesIssuesOpenArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsRepositoriesIssuesPatchArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsRepositoriesListArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsRepositoriesPatchArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsRepositoriesPullRequestsCloseArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsRepositoriesPullRequestsCreateArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsRepositoriesPullRequestsGetArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsRepositoriesPullRequestsListArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsRepositoriesPullRequestsListFileDiffsArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsRepositoriesPullRequestsMergeArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsRepositoriesPullRequestsOpenArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsRepositoriesPullRequestsPatchArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsRepositoriesPullRequestsPullRequestCommentsBatchCreateArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsRepositoriesPullRequestsPullRequestCommentsCreateArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsRepositoriesPullRequestsPullRequestCommentsDeleteArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsRepositoriesPullRequestsPullRequestCommentsGetArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsRepositoriesPullRequestsPullRequestCommentsListArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsRepositoriesPullRequestsPullRequestCommentsPatchArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsRepositoriesPullRequestsPullRequestCommentsResolveArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsRepositoriesPullRequestsPullRequestCommentsUnresolveArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsRepositoriesSetIamPolicyArgs;
use crate::providers::gcp::clients::securesourcemanager::SecuresourcemanagerProjectsLocationsRepositoriesTestIamPermissionsArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// SecuresourcemanagerProvider with automatic state tracking.
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
/// let provider = SecuresourcemanagerProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct SecuresourcemanagerProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> SecuresourcemanagerProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new SecuresourcemanagerProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new SecuresourcemanagerProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Securesourcemanager projects locations get.
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
    pub fn securesourcemanager_projects_locations_get(
        &self,
        args: &SecuresourcemanagerProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations list.
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
    pub fn securesourcemanager_projects_locations_list(
        &self,
        args: &SecuresourcemanagerProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations instances create.
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
    pub fn securesourcemanager_projects_locations_instances_create(
        &self,
        args: &SecuresourcemanagerProjectsLocationsInstancesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_instances_create_builder(
            &self.http_client,
            &args.parent,
            &args.instanceId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_instances_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations instances delete.
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
    pub fn securesourcemanager_projects_locations_instances_delete(
        &self,
        args: &SecuresourcemanagerProjectsLocationsInstancesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_instances_delete_builder(
            &self.http_client,
            &args.name,
            &args.force,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_instances_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations instances get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Instance result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn securesourcemanager_projects_locations_instances_get(
        &self,
        args: &SecuresourcemanagerProjectsLocationsInstancesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Instance, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_instances_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_instances_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations instances get iam policy.
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
    pub fn securesourcemanager_projects_locations_instances_get_iam_policy(
        &self,
        args: &SecuresourcemanagerProjectsLocationsInstancesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_instances_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options_requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_instances_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations instances list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListInstancesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn securesourcemanager_projects_locations_instances_list(
        &self,
        args: &SecuresourcemanagerProjectsLocationsInstancesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListInstancesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_instances_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_instances_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations instances set iam policy.
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
    pub fn securesourcemanager_projects_locations_instances_set_iam_policy(
        &self,
        args: &SecuresourcemanagerProjectsLocationsInstancesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_instances_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_instances_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations instances test iam permissions.
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
    pub fn securesourcemanager_projects_locations_instances_test_iam_permissions(
        &self,
        args: &SecuresourcemanagerProjectsLocationsInstancesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_instances_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_instances_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations operations cancel.
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
    pub fn securesourcemanager_projects_locations_operations_cancel(
        &self,
        args: &SecuresourcemanagerProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations operations delete.
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
    pub fn securesourcemanager_projects_locations_operations_delete(
        &self,
        args: &SecuresourcemanagerProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations operations get.
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
    pub fn securesourcemanager_projects_locations_operations_get(
        &self,
        args: &SecuresourcemanagerProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations operations list.
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
    pub fn securesourcemanager_projects_locations_operations_list(
        &self,
        args: &SecuresourcemanagerProjectsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations repositories create.
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
    pub fn securesourcemanager_projects_locations_repositories_create(
        &self,
        args: &SecuresourcemanagerProjectsLocationsRepositoriesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_repositories_create_builder(
            &self.http_client,
            &args.parent,
            &args.repositoryId,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_repositories_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations repositories delete.
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
    pub fn securesourcemanager_projects_locations_repositories_delete(
        &self,
        args: &SecuresourcemanagerProjectsLocationsRepositoriesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_repositories_delete_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_repositories_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations repositories fetch blob.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FetchBlobResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn securesourcemanager_projects_locations_repositories_fetch_blob(
        &self,
        args: &SecuresourcemanagerProjectsLocationsRepositoriesFetchBlobArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FetchBlobResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_repositories_fetch_blob_builder(
            &self.http_client,
            &args.repository,
            &args.sha,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_repositories_fetch_blob_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations repositories fetch tree.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FetchTreeResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn securesourcemanager_projects_locations_repositories_fetch_tree(
        &self,
        args: &SecuresourcemanagerProjectsLocationsRepositoriesFetchTreeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FetchTreeResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_repositories_fetch_tree_builder(
            &self.http_client,
            &args.repository,
            &args.pageSize,
            &args.pageToken,
            &args.recursive,
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_repositories_fetch_tree_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations repositories get.
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
    pub fn securesourcemanager_projects_locations_repositories_get(
        &self,
        args: &SecuresourcemanagerProjectsLocationsRepositoriesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Repository, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_repositories_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_repositories_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations repositories get iam policy.
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
    pub fn securesourcemanager_projects_locations_repositories_get_iam_policy(
        &self,
        args: &SecuresourcemanagerProjectsLocationsRepositoriesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_repositories_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options_requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_repositories_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations repositories list.
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
    pub fn securesourcemanager_projects_locations_repositories_list(
        &self,
        args: &SecuresourcemanagerProjectsLocationsRepositoriesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListRepositoriesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_repositories_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.instance,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_repositories_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations repositories patch.
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
    pub fn securesourcemanager_projects_locations_repositories_patch(
        &self,
        args: &SecuresourcemanagerProjectsLocationsRepositoriesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_repositories_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_repositories_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations repositories set iam policy.
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
    pub fn securesourcemanager_projects_locations_repositories_set_iam_policy(
        &self,
        args: &SecuresourcemanagerProjectsLocationsRepositoriesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_repositories_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_repositories_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations repositories test iam permissions.
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
    pub fn securesourcemanager_projects_locations_repositories_test_iam_permissions(
        &self,
        args: &SecuresourcemanagerProjectsLocationsRepositoriesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_repositories_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_repositories_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations repositories branch rules create.
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
    pub fn securesourcemanager_projects_locations_repositories_branch_rules_create(
        &self,
        args: &SecuresourcemanagerProjectsLocationsRepositoriesBranchRulesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_repositories_branch_rules_create_builder(
            &self.http_client,
            &args.parent,
            &args.branchRuleId,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_repositories_branch_rules_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations repositories branch rules delete.
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
    pub fn securesourcemanager_projects_locations_repositories_branch_rules_delete(
        &self,
        args: &SecuresourcemanagerProjectsLocationsRepositoriesBranchRulesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_repositories_branch_rules_delete_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_repositories_branch_rules_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations repositories branch rules get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BranchRule result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn securesourcemanager_projects_locations_repositories_branch_rules_get(
        &self,
        args: &SecuresourcemanagerProjectsLocationsRepositoriesBranchRulesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BranchRule, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_repositories_branch_rules_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_repositories_branch_rules_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations repositories branch rules list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListBranchRulesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn securesourcemanager_projects_locations_repositories_branch_rules_list(
        &self,
        args: &SecuresourcemanagerProjectsLocationsRepositoriesBranchRulesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBranchRulesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_repositories_branch_rules_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_repositories_branch_rules_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations repositories branch rules patch.
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
    pub fn securesourcemanager_projects_locations_repositories_branch_rules_patch(
        &self,
        args: &SecuresourcemanagerProjectsLocationsRepositoriesBranchRulesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_repositories_branch_rules_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_repositories_branch_rules_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations repositories hooks create.
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
    pub fn securesourcemanager_projects_locations_repositories_hooks_create(
        &self,
        args: &SecuresourcemanagerProjectsLocationsRepositoriesHooksCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_repositories_hooks_create_builder(
            &self.http_client,
            &args.parent,
            &args.hookId,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_repositories_hooks_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations repositories hooks delete.
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
    pub fn securesourcemanager_projects_locations_repositories_hooks_delete(
        &self,
        args: &SecuresourcemanagerProjectsLocationsRepositoriesHooksDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_repositories_hooks_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_repositories_hooks_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations repositories hooks get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Hook result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn securesourcemanager_projects_locations_repositories_hooks_get(
        &self,
        args: &SecuresourcemanagerProjectsLocationsRepositoriesHooksGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Hook, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_repositories_hooks_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_repositories_hooks_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations repositories hooks list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListHooksResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn securesourcemanager_projects_locations_repositories_hooks_list(
        &self,
        args: &SecuresourcemanagerProjectsLocationsRepositoriesHooksListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListHooksResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_repositories_hooks_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_repositories_hooks_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations repositories hooks patch.
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
    pub fn securesourcemanager_projects_locations_repositories_hooks_patch(
        &self,
        args: &SecuresourcemanagerProjectsLocationsRepositoriesHooksPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_repositories_hooks_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_repositories_hooks_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations repositories issues close.
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
    pub fn securesourcemanager_projects_locations_repositories_issues_close(
        &self,
        args: &SecuresourcemanagerProjectsLocationsRepositoriesIssuesCloseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_repositories_issues_close_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_repositories_issues_close_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations repositories issues create.
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
    pub fn securesourcemanager_projects_locations_repositories_issues_create(
        &self,
        args: &SecuresourcemanagerProjectsLocationsRepositoriesIssuesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_repositories_issues_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_repositories_issues_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations repositories issues delete.
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
    pub fn securesourcemanager_projects_locations_repositories_issues_delete(
        &self,
        args: &SecuresourcemanagerProjectsLocationsRepositoriesIssuesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_repositories_issues_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_repositories_issues_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations repositories issues get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Issue result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn securesourcemanager_projects_locations_repositories_issues_get(
        &self,
        args: &SecuresourcemanagerProjectsLocationsRepositoriesIssuesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Issue, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_repositories_issues_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_repositories_issues_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations repositories issues list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListIssuesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn securesourcemanager_projects_locations_repositories_issues_list(
        &self,
        args: &SecuresourcemanagerProjectsLocationsRepositoriesIssuesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListIssuesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_repositories_issues_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_repositories_issues_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations repositories issues open.
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
    pub fn securesourcemanager_projects_locations_repositories_issues_open(
        &self,
        args: &SecuresourcemanagerProjectsLocationsRepositoriesIssuesOpenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_repositories_issues_open_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_repositories_issues_open_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations repositories issues patch.
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
    pub fn securesourcemanager_projects_locations_repositories_issues_patch(
        &self,
        args: &SecuresourcemanagerProjectsLocationsRepositoriesIssuesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_repositories_issues_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_repositories_issues_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations repositories issues issue comments create.
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
    pub fn securesourcemanager_projects_locations_repositories_issues_issue_comments_create(
        &self,
        args: &SecuresourcemanagerProjectsLocationsRepositoriesIssuesIssueCommentsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_repositories_issues_issue_comments_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_repositories_issues_issue_comments_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations repositories issues issue comments delete.
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
    pub fn securesourcemanager_projects_locations_repositories_issues_issue_comments_delete(
        &self,
        args: &SecuresourcemanagerProjectsLocationsRepositoriesIssuesIssueCommentsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_repositories_issues_issue_comments_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_repositories_issues_issue_comments_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations repositories issues issue comments get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IssueComment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn securesourcemanager_projects_locations_repositories_issues_issue_comments_get(
        &self,
        args: &SecuresourcemanagerProjectsLocationsRepositoriesIssuesIssueCommentsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IssueComment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_repositories_issues_issue_comments_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_repositories_issues_issue_comments_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations repositories issues issue comments list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListIssueCommentsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn securesourcemanager_projects_locations_repositories_issues_issue_comments_list(
        &self,
        args: &SecuresourcemanagerProjectsLocationsRepositoriesIssuesIssueCommentsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListIssueCommentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_repositories_issues_issue_comments_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_repositories_issues_issue_comments_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations repositories issues issue comments patch.
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
    pub fn securesourcemanager_projects_locations_repositories_issues_issue_comments_patch(
        &self,
        args: &SecuresourcemanagerProjectsLocationsRepositoriesIssuesIssueCommentsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_repositories_issues_issue_comments_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_repositories_issues_issue_comments_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations repositories pull requests close.
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
    pub fn securesourcemanager_projects_locations_repositories_pull_requests_close(
        &self,
        args: &SecuresourcemanagerProjectsLocationsRepositoriesPullRequestsCloseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_repositories_pull_requests_close_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_repositories_pull_requests_close_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations repositories pull requests create.
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
    pub fn securesourcemanager_projects_locations_repositories_pull_requests_create(
        &self,
        args: &SecuresourcemanagerProjectsLocationsRepositoriesPullRequestsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_repositories_pull_requests_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_repositories_pull_requests_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations repositories pull requests get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PullRequest result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn securesourcemanager_projects_locations_repositories_pull_requests_get(
        &self,
        args: &SecuresourcemanagerProjectsLocationsRepositoriesPullRequestsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PullRequest, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_repositories_pull_requests_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_repositories_pull_requests_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations repositories pull requests list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListPullRequestsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn securesourcemanager_projects_locations_repositories_pull_requests_list(
        &self,
        args: &SecuresourcemanagerProjectsLocationsRepositoriesPullRequestsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListPullRequestsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_repositories_pull_requests_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_repositories_pull_requests_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations repositories pull requests list file diffs.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListPullRequestFileDiffsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn securesourcemanager_projects_locations_repositories_pull_requests_list_file_diffs(
        &self,
        args: &SecuresourcemanagerProjectsLocationsRepositoriesPullRequestsListFileDiffsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListPullRequestFileDiffsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_repositories_pull_requests_list_file_diffs_builder(
            &self.http_client,
            &args.name,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_repositories_pull_requests_list_file_diffs_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations repositories pull requests merge.
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
    pub fn securesourcemanager_projects_locations_repositories_pull_requests_merge(
        &self,
        args: &SecuresourcemanagerProjectsLocationsRepositoriesPullRequestsMergeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_repositories_pull_requests_merge_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_repositories_pull_requests_merge_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations repositories pull requests open.
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
    pub fn securesourcemanager_projects_locations_repositories_pull_requests_open(
        &self,
        args: &SecuresourcemanagerProjectsLocationsRepositoriesPullRequestsOpenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_repositories_pull_requests_open_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_repositories_pull_requests_open_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations repositories pull requests patch.
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
    pub fn securesourcemanager_projects_locations_repositories_pull_requests_patch(
        &self,
        args: &SecuresourcemanagerProjectsLocationsRepositoriesPullRequestsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_repositories_pull_requests_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_repositories_pull_requests_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations repositories pull requests pull request comments batch create.
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
    pub fn securesourcemanager_projects_locations_repositories_pull_requests_pull_request_comments_batch_create(
        &self,
        args: &SecuresourcemanagerProjectsLocationsRepositoriesPullRequestsPullRequestCommentsBatchCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_repositories_pull_requests_pull_request_comments_batch_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_repositories_pull_requests_pull_request_comments_batch_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations repositories pull requests pull request comments create.
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
    pub fn securesourcemanager_projects_locations_repositories_pull_requests_pull_request_comments_create(
        &self,
        args: &SecuresourcemanagerProjectsLocationsRepositoriesPullRequestsPullRequestCommentsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_repositories_pull_requests_pull_request_comments_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_repositories_pull_requests_pull_request_comments_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations repositories pull requests pull request comments delete.
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
    pub fn securesourcemanager_projects_locations_repositories_pull_requests_pull_request_comments_delete(
        &self,
        args: &SecuresourcemanagerProjectsLocationsRepositoriesPullRequestsPullRequestCommentsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_repositories_pull_requests_pull_request_comments_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_repositories_pull_requests_pull_request_comments_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations repositories pull requests pull request comments get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PullRequestComment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn securesourcemanager_projects_locations_repositories_pull_requests_pull_request_comments_get(
        &self,
        args: &SecuresourcemanagerProjectsLocationsRepositoriesPullRequestsPullRequestCommentsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PullRequestComment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_repositories_pull_requests_pull_request_comments_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_repositories_pull_requests_pull_request_comments_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations repositories pull requests pull request comments list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListPullRequestCommentsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn securesourcemanager_projects_locations_repositories_pull_requests_pull_request_comments_list(
        &self,
        args: &SecuresourcemanagerProjectsLocationsRepositoriesPullRequestsPullRequestCommentsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListPullRequestCommentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_repositories_pull_requests_pull_request_comments_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_repositories_pull_requests_pull_request_comments_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations repositories pull requests pull request comments patch.
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
    pub fn securesourcemanager_projects_locations_repositories_pull_requests_pull_request_comments_patch(
        &self,
        args: &SecuresourcemanagerProjectsLocationsRepositoriesPullRequestsPullRequestCommentsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_repositories_pull_requests_pull_request_comments_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_repositories_pull_requests_pull_request_comments_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations repositories pull requests pull request comments resolve.
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
    pub fn securesourcemanager_projects_locations_repositories_pull_requests_pull_request_comments_resolve(
        &self,
        args: &SecuresourcemanagerProjectsLocationsRepositoriesPullRequestsPullRequestCommentsResolveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_repositories_pull_requests_pull_request_comments_resolve_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_repositories_pull_requests_pull_request_comments_resolve_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securesourcemanager projects locations repositories pull requests pull request comments unresolve.
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
    pub fn securesourcemanager_projects_locations_repositories_pull_requests_pull_request_comments_unresolve(
        &self,
        args: &SecuresourcemanagerProjectsLocationsRepositoriesPullRequestsPullRequestCommentsUnresolveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securesourcemanager_projects_locations_repositories_pull_requests_pull_request_comments_unresolve_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = securesourcemanager_projects_locations_repositories_pull_requests_pull_request_comments_unresolve_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
