//! DeveloperconnectProvider - State-aware developerconnect API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       developerconnect API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::developerconnect::{
    developerconnect_projects_locations_get_builder, developerconnect_projects_locations_get_task,
    developerconnect_projects_locations_list_builder, developerconnect_projects_locations_list_task,
    developerconnect_projects_locations_account_connectors_create_builder, developerconnect_projects_locations_account_connectors_create_task,
    developerconnect_projects_locations_account_connectors_delete_builder, developerconnect_projects_locations_account_connectors_delete_task,
    developerconnect_projects_locations_account_connectors_fetch_user_repositories_builder, developerconnect_projects_locations_account_connectors_fetch_user_repositories_task,
    developerconnect_projects_locations_account_connectors_get_builder, developerconnect_projects_locations_account_connectors_get_task,
    developerconnect_projects_locations_account_connectors_list_builder, developerconnect_projects_locations_account_connectors_list_task,
    developerconnect_projects_locations_account_connectors_patch_builder, developerconnect_projects_locations_account_connectors_patch_task,
    developerconnect_projects_locations_account_connectors_users_delete_builder, developerconnect_projects_locations_account_connectors_users_delete_task,
    developerconnect_projects_locations_account_connectors_users_delete_self_builder, developerconnect_projects_locations_account_connectors_users_delete_self_task,
    developerconnect_projects_locations_account_connectors_users_fetch_access_token_builder, developerconnect_projects_locations_account_connectors_users_fetch_access_token_task,
    developerconnect_projects_locations_account_connectors_users_fetch_self_builder, developerconnect_projects_locations_account_connectors_users_fetch_self_task,
    developerconnect_projects_locations_account_connectors_users_finish_o_auth_flow_builder, developerconnect_projects_locations_account_connectors_users_finish_o_auth_flow_task,
    developerconnect_projects_locations_account_connectors_users_list_builder, developerconnect_projects_locations_account_connectors_users_list_task,
    developerconnect_projects_locations_account_connectors_users_start_o_auth_flow_builder, developerconnect_projects_locations_account_connectors_users_start_o_auth_flow_task,
    developerconnect_projects_locations_connections_create_builder, developerconnect_projects_locations_connections_create_task,
    developerconnect_projects_locations_connections_delete_builder, developerconnect_projects_locations_connections_delete_task,
    developerconnect_projects_locations_connections_fetch_git_hub_installations_builder, developerconnect_projects_locations_connections_fetch_git_hub_installations_task,
    developerconnect_projects_locations_connections_fetch_linkable_git_repositories_builder, developerconnect_projects_locations_connections_fetch_linkable_git_repositories_task,
    developerconnect_projects_locations_connections_get_builder, developerconnect_projects_locations_connections_get_task,
    developerconnect_projects_locations_connections_list_builder, developerconnect_projects_locations_connections_list_task,
    developerconnect_projects_locations_connections_patch_builder, developerconnect_projects_locations_connections_patch_task,
    developerconnect_projects_locations_connections_process_git_hub_enterprise_webhook_builder, developerconnect_projects_locations_connections_process_git_hub_enterprise_webhook_task,
    developerconnect_projects_locations_connections_git_repository_links_create_builder, developerconnect_projects_locations_connections_git_repository_links_create_task,
    developerconnect_projects_locations_connections_git_repository_links_delete_builder, developerconnect_projects_locations_connections_git_repository_links_delete_task,
    developerconnect_projects_locations_connections_git_repository_links_fetch_git_refs_builder, developerconnect_projects_locations_connections_git_repository_links_fetch_git_refs_task,
    developerconnect_projects_locations_connections_git_repository_links_fetch_read_token_builder, developerconnect_projects_locations_connections_git_repository_links_fetch_read_token_task,
    developerconnect_projects_locations_connections_git_repository_links_fetch_read_write_token_builder, developerconnect_projects_locations_connections_git_repository_links_fetch_read_write_token_task,
    developerconnect_projects_locations_connections_git_repository_links_get_builder, developerconnect_projects_locations_connections_git_repository_links_get_task,
    developerconnect_projects_locations_connections_git_repository_links_list_builder, developerconnect_projects_locations_connections_git_repository_links_list_task,
    developerconnect_projects_locations_connections_git_repository_links_process_bitbucket_cloud_webhook_builder, developerconnect_projects_locations_connections_git_repository_links_process_bitbucket_cloud_webhook_task,
    developerconnect_projects_locations_connections_git_repository_links_process_bitbucket_data_center_webhook_builder, developerconnect_projects_locations_connections_git_repository_links_process_bitbucket_data_center_webhook_task,
    developerconnect_projects_locations_connections_git_repository_links_process_git_lab_enterprise_webhook_builder, developerconnect_projects_locations_connections_git_repository_links_process_git_lab_enterprise_webhook_task,
    developerconnect_projects_locations_connections_git_repository_links_process_git_lab_webhook_builder, developerconnect_projects_locations_connections_git_repository_links_process_git_lab_webhook_task,
    developerconnect_projects_locations_insights_configs_create_builder, developerconnect_projects_locations_insights_configs_create_task,
    developerconnect_projects_locations_insights_configs_delete_builder, developerconnect_projects_locations_insights_configs_delete_task,
    developerconnect_projects_locations_insights_configs_get_builder, developerconnect_projects_locations_insights_configs_get_task,
    developerconnect_projects_locations_insights_configs_list_builder, developerconnect_projects_locations_insights_configs_list_task,
    developerconnect_projects_locations_insights_configs_patch_builder, developerconnect_projects_locations_insights_configs_patch_task,
    developerconnect_projects_locations_insights_configs_deployment_events_get_builder, developerconnect_projects_locations_insights_configs_deployment_events_get_task,
    developerconnect_projects_locations_insights_configs_deployment_events_list_builder, developerconnect_projects_locations_insights_configs_deployment_events_list_task,
    developerconnect_projects_locations_operations_cancel_builder, developerconnect_projects_locations_operations_cancel_task,
    developerconnect_projects_locations_operations_delete_builder, developerconnect_projects_locations_operations_delete_task,
    developerconnect_projects_locations_operations_get_builder, developerconnect_projects_locations_operations_get_task,
    developerconnect_projects_locations_operations_list_builder, developerconnect_projects_locations_operations_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::developerconnect::AccountConnector;
use crate::providers::gcp::clients::developerconnect::Connection;
use crate::providers::gcp::clients::developerconnect::DeploymentEvent;
use crate::providers::gcp::clients::developerconnect::Empty;
use crate::providers::gcp::clients::developerconnect::FetchAccessTokenResponse;
use crate::providers::gcp::clients::developerconnect::FetchGitHubInstallationsResponse;
use crate::providers::gcp::clients::developerconnect::FetchGitRefsResponse;
use crate::providers::gcp::clients::developerconnect::FetchLinkableGitRepositoriesResponse;
use crate::providers::gcp::clients::developerconnect::FetchReadTokenResponse;
use crate::providers::gcp::clients::developerconnect::FetchReadWriteTokenResponse;
use crate::providers::gcp::clients::developerconnect::FetchUserRepositoriesResponse;
use crate::providers::gcp::clients::developerconnect::FinishOAuthResponse;
use crate::providers::gcp::clients::developerconnect::GitRepositoryLink;
use crate::providers::gcp::clients::developerconnect::InsightsConfig;
use crate::providers::gcp::clients::developerconnect::ListAccountConnectorsResponse;
use crate::providers::gcp::clients::developerconnect::ListConnectionsResponse;
use crate::providers::gcp::clients::developerconnect::ListDeploymentEventsResponse;
use crate::providers::gcp::clients::developerconnect::ListGitRepositoryLinksResponse;
use crate::providers::gcp::clients::developerconnect::ListInsightsConfigsResponse;
use crate::providers::gcp::clients::developerconnect::ListLocationsResponse;
use crate::providers::gcp::clients::developerconnect::ListOperationsResponse;
use crate::providers::gcp::clients::developerconnect::ListUsersResponse;
use crate::providers::gcp::clients::developerconnect::Location;
use crate::providers::gcp::clients::developerconnect::Operation;
use crate::providers::gcp::clients::developerconnect::StartOAuthResponse;
use crate::providers::gcp::clients::developerconnect::User;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsAccountConnectorsCreateArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsAccountConnectorsDeleteArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsAccountConnectorsFetchUserRepositoriesArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsAccountConnectorsGetArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsAccountConnectorsListArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsAccountConnectorsPatchArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsAccountConnectorsUsersDeleteArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsAccountConnectorsUsersDeleteSelfArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsAccountConnectorsUsersFetchAccessTokenArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsAccountConnectorsUsersFetchSelfArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsAccountConnectorsUsersFinishOAuthFlowArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsAccountConnectorsUsersListArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsAccountConnectorsUsersStartOAuthFlowArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsConnectionsCreateArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsConnectionsDeleteArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsConnectionsFetchGitHubInstallationsArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsConnectionsFetchLinkableGitRepositoriesArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsConnectionsGetArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsConnectionsGitRepositoryLinksCreateArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsConnectionsGitRepositoryLinksDeleteArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsConnectionsGitRepositoryLinksFetchGitRefsArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsConnectionsGitRepositoryLinksFetchReadTokenArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsConnectionsGitRepositoryLinksFetchReadWriteTokenArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsConnectionsGitRepositoryLinksGetArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsConnectionsGitRepositoryLinksListArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsConnectionsGitRepositoryLinksProcessBitbucketCloudWebhookArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsConnectionsGitRepositoryLinksProcessBitbucketDataCenterWebhookArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsConnectionsGitRepositoryLinksProcessGitLabEnterpriseWebhookArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsConnectionsGitRepositoryLinksProcessGitLabWebhookArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsConnectionsListArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsConnectionsPatchArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsConnectionsProcessGitHubEnterpriseWebhookArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsGetArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsInsightsConfigsCreateArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsInsightsConfigsDeleteArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsInsightsConfigsDeploymentEventsGetArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsInsightsConfigsDeploymentEventsListArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsInsightsConfigsGetArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsInsightsConfigsListArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsInsightsConfigsPatchArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsListArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsOperationsListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// DeveloperconnectProvider with automatic state tracking.
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
/// let provider = DeveloperconnectProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct DeveloperconnectProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> DeveloperconnectProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new DeveloperconnectProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new DeveloperconnectProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Developerconnect projects locations get.
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
    pub fn developerconnect_projects_locations_get(
        &self,
        args: &DeveloperconnectProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = developerconnect_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = developerconnect_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Developerconnect projects locations list.
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
    pub fn developerconnect_projects_locations_list(
        &self,
        args: &DeveloperconnectProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = developerconnect_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = developerconnect_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Developerconnect projects locations account connectors create.
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
    pub fn developerconnect_projects_locations_account_connectors_create(
        &self,
        args: &DeveloperconnectProjectsLocationsAccountConnectorsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = developerconnect_projects_locations_account_connectors_create_builder(
            &self.http_client,
            &args.parent,
            &args.accountConnectorId,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = developerconnect_projects_locations_account_connectors_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Developerconnect projects locations account connectors delete.
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
    pub fn developerconnect_projects_locations_account_connectors_delete(
        &self,
        args: &DeveloperconnectProjectsLocationsAccountConnectorsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = developerconnect_projects_locations_account_connectors_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.force,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = developerconnect_projects_locations_account_connectors_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Developerconnect projects locations account connectors fetch user repositories.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FetchUserRepositoriesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn developerconnect_projects_locations_account_connectors_fetch_user_repositories(
        &self,
        args: &DeveloperconnectProjectsLocationsAccountConnectorsFetchUserRepositoriesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FetchUserRepositoriesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = developerconnect_projects_locations_account_connectors_fetch_user_repositories_builder(
            &self.http_client,
            &args.accountConnector,
            &args.pageSize,
            &args.pageToken,
            &args.repository,
        )
        .map_err(ProviderError::Api)?;

        let task = developerconnect_projects_locations_account_connectors_fetch_user_repositories_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Developerconnect projects locations account connectors get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccountConnector result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn developerconnect_projects_locations_account_connectors_get(
        &self,
        args: &DeveloperconnectProjectsLocationsAccountConnectorsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccountConnector, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = developerconnect_projects_locations_account_connectors_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = developerconnect_projects_locations_account_connectors_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Developerconnect projects locations account connectors list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAccountConnectorsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn developerconnect_projects_locations_account_connectors_list(
        &self,
        args: &DeveloperconnectProjectsLocationsAccountConnectorsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAccountConnectorsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = developerconnect_projects_locations_account_connectors_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = developerconnect_projects_locations_account_connectors_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Developerconnect projects locations account connectors patch.
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
    pub fn developerconnect_projects_locations_account_connectors_patch(
        &self,
        args: &DeveloperconnectProjectsLocationsAccountConnectorsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = developerconnect_projects_locations_account_connectors_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.requestId,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = developerconnect_projects_locations_account_connectors_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Developerconnect projects locations account connectors users delete.
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
    pub fn developerconnect_projects_locations_account_connectors_users_delete(
        &self,
        args: &DeveloperconnectProjectsLocationsAccountConnectorsUsersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = developerconnect_projects_locations_account_connectors_users_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = developerconnect_projects_locations_account_connectors_users_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Developerconnect projects locations account connectors users delete self.
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
    pub fn developerconnect_projects_locations_account_connectors_users_delete_self(
        &self,
        args: &DeveloperconnectProjectsLocationsAccountConnectorsUsersDeleteSelfArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = developerconnect_projects_locations_account_connectors_users_delete_self_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = developerconnect_projects_locations_account_connectors_users_delete_self_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Developerconnect projects locations account connectors users fetch access token.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FetchAccessTokenResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn developerconnect_projects_locations_account_connectors_users_fetch_access_token(
        &self,
        args: &DeveloperconnectProjectsLocationsAccountConnectorsUsersFetchAccessTokenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FetchAccessTokenResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = developerconnect_projects_locations_account_connectors_users_fetch_access_token_builder(
            &self.http_client,
            &args.accountConnector,
        )
        .map_err(ProviderError::Api)?;

        let task = developerconnect_projects_locations_account_connectors_users_fetch_access_token_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Developerconnect projects locations account connectors users fetch self.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the User result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn developerconnect_projects_locations_account_connectors_users_fetch_self(
        &self,
        args: &DeveloperconnectProjectsLocationsAccountConnectorsUsersFetchSelfArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<User, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = developerconnect_projects_locations_account_connectors_users_fetch_self_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = developerconnect_projects_locations_account_connectors_users_fetch_self_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Developerconnect projects locations account connectors users finish o auth flow.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FinishOAuthResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn developerconnect_projects_locations_account_connectors_users_finish_o_auth_flow(
        &self,
        args: &DeveloperconnectProjectsLocationsAccountConnectorsUsersFinishOAuthFlowArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FinishOAuthResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = developerconnect_projects_locations_account_connectors_users_finish_o_auth_flow_builder(
            &self.http_client,
            &args.accountConnector,
            &args.googleOauthParams_scopes,
            &args.googleOauthParams_ticket,
            &args.googleOauthParams_versionInfo,
            &args.oauthParams_code,
            &args.oauthParams_ticket,
        )
        .map_err(ProviderError::Api)?;

        let task = developerconnect_projects_locations_account_connectors_users_finish_o_auth_flow_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Developerconnect projects locations account connectors users list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListUsersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn developerconnect_projects_locations_account_connectors_users_list(
        &self,
        args: &DeveloperconnectProjectsLocationsAccountConnectorsUsersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListUsersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = developerconnect_projects_locations_account_connectors_users_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = developerconnect_projects_locations_account_connectors_users_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Developerconnect projects locations account connectors users start o auth flow.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the StartOAuthResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn developerconnect_projects_locations_account_connectors_users_start_o_auth_flow(
        &self,
        args: &DeveloperconnectProjectsLocationsAccountConnectorsUsersStartOAuthFlowArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<StartOAuthResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = developerconnect_projects_locations_account_connectors_users_start_o_auth_flow_builder(
            &self.http_client,
            &args.accountConnector,
        )
        .map_err(ProviderError::Api)?;

        let task = developerconnect_projects_locations_account_connectors_users_start_o_auth_flow_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Developerconnect projects locations connections create.
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
    pub fn developerconnect_projects_locations_connections_create(
        &self,
        args: &DeveloperconnectProjectsLocationsConnectionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = developerconnect_projects_locations_connections_create_builder(
            &self.http_client,
            &args.parent,
            &args.connectionId,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = developerconnect_projects_locations_connections_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Developerconnect projects locations connections delete.
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
    pub fn developerconnect_projects_locations_connections_delete(
        &self,
        args: &DeveloperconnectProjectsLocationsConnectionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = developerconnect_projects_locations_connections_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = developerconnect_projects_locations_connections_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Developerconnect projects locations connections fetch git hub installations.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FetchGitHubInstallationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn developerconnect_projects_locations_connections_fetch_git_hub_installations(
        &self,
        args: &DeveloperconnectProjectsLocationsConnectionsFetchGitHubInstallationsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FetchGitHubInstallationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = developerconnect_projects_locations_connections_fetch_git_hub_installations_builder(
            &self.http_client,
            &args.connection,
        )
        .map_err(ProviderError::Api)?;

        let task = developerconnect_projects_locations_connections_fetch_git_hub_installations_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Developerconnect projects locations connections fetch linkable git repositories.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FetchLinkableGitRepositoriesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn developerconnect_projects_locations_connections_fetch_linkable_git_repositories(
        &self,
        args: &DeveloperconnectProjectsLocationsConnectionsFetchLinkableGitRepositoriesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FetchLinkableGitRepositoriesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = developerconnect_projects_locations_connections_fetch_linkable_git_repositories_builder(
            &self.http_client,
            &args.connection,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = developerconnect_projects_locations_connections_fetch_linkable_git_repositories_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Developerconnect projects locations connections get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Connection result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn developerconnect_projects_locations_connections_get(
        &self,
        args: &DeveloperconnectProjectsLocationsConnectionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Connection, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = developerconnect_projects_locations_connections_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = developerconnect_projects_locations_connections_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Developerconnect projects locations connections list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListConnectionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn developerconnect_projects_locations_connections_list(
        &self,
        args: &DeveloperconnectProjectsLocationsConnectionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListConnectionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = developerconnect_projects_locations_connections_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = developerconnect_projects_locations_connections_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Developerconnect projects locations connections patch.
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
    pub fn developerconnect_projects_locations_connections_patch(
        &self,
        args: &DeveloperconnectProjectsLocationsConnectionsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = developerconnect_projects_locations_connections_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.requestId,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = developerconnect_projects_locations_connections_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Developerconnect projects locations connections process git hub enterprise webhook.
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
    pub fn developerconnect_projects_locations_connections_process_git_hub_enterprise_webhook(
        &self,
        args: &DeveloperconnectProjectsLocationsConnectionsProcessGitHubEnterpriseWebhookArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = developerconnect_projects_locations_connections_process_git_hub_enterprise_webhook_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = developerconnect_projects_locations_connections_process_git_hub_enterprise_webhook_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Developerconnect projects locations connections git repository links create.
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
    pub fn developerconnect_projects_locations_connections_git_repository_links_create(
        &self,
        args: &DeveloperconnectProjectsLocationsConnectionsGitRepositoryLinksCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = developerconnect_projects_locations_connections_git_repository_links_create_builder(
            &self.http_client,
            &args.parent,
            &args.gitRepositoryLinkId,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = developerconnect_projects_locations_connections_git_repository_links_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Developerconnect projects locations connections git repository links delete.
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
    pub fn developerconnect_projects_locations_connections_git_repository_links_delete(
        &self,
        args: &DeveloperconnectProjectsLocationsConnectionsGitRepositoryLinksDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = developerconnect_projects_locations_connections_git_repository_links_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = developerconnect_projects_locations_connections_git_repository_links_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Developerconnect projects locations connections git repository links fetch git refs.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FetchGitRefsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn developerconnect_projects_locations_connections_git_repository_links_fetch_git_refs(
        &self,
        args: &DeveloperconnectProjectsLocationsConnectionsGitRepositoryLinksFetchGitRefsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FetchGitRefsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = developerconnect_projects_locations_connections_git_repository_links_fetch_git_refs_builder(
            &self.http_client,
            &args.gitRepositoryLink,
            &args.pageSize,
            &args.pageToken,
            &args.refType,
        )
        .map_err(ProviderError::Api)?;

        let task = developerconnect_projects_locations_connections_git_repository_links_fetch_git_refs_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Developerconnect projects locations connections git repository links fetch read token.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FetchReadTokenResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn developerconnect_projects_locations_connections_git_repository_links_fetch_read_token(
        &self,
        args: &DeveloperconnectProjectsLocationsConnectionsGitRepositoryLinksFetchReadTokenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FetchReadTokenResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = developerconnect_projects_locations_connections_git_repository_links_fetch_read_token_builder(
            &self.http_client,
            &args.gitRepositoryLink,
        )
        .map_err(ProviderError::Api)?;

        let task = developerconnect_projects_locations_connections_git_repository_links_fetch_read_token_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Developerconnect projects locations connections git repository links fetch read write token.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FetchReadWriteTokenResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn developerconnect_projects_locations_connections_git_repository_links_fetch_read_write_token(
        &self,
        args: &DeveloperconnectProjectsLocationsConnectionsGitRepositoryLinksFetchReadWriteTokenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FetchReadWriteTokenResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = developerconnect_projects_locations_connections_git_repository_links_fetch_read_write_token_builder(
            &self.http_client,
            &args.gitRepositoryLink,
        )
        .map_err(ProviderError::Api)?;

        let task = developerconnect_projects_locations_connections_git_repository_links_fetch_read_write_token_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Developerconnect projects locations connections git repository links get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GitRepositoryLink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn developerconnect_projects_locations_connections_git_repository_links_get(
        &self,
        args: &DeveloperconnectProjectsLocationsConnectionsGitRepositoryLinksGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GitRepositoryLink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = developerconnect_projects_locations_connections_git_repository_links_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = developerconnect_projects_locations_connections_git_repository_links_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Developerconnect projects locations connections git repository links list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListGitRepositoryLinksResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn developerconnect_projects_locations_connections_git_repository_links_list(
        &self,
        args: &DeveloperconnectProjectsLocationsConnectionsGitRepositoryLinksListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListGitRepositoryLinksResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = developerconnect_projects_locations_connections_git_repository_links_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = developerconnect_projects_locations_connections_git_repository_links_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Developerconnect projects locations connections git repository links process bitbucket cloud webhook.
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
    pub fn developerconnect_projects_locations_connections_git_repository_links_process_bitbucket_cloud_webhook(
        &self,
        args: &DeveloperconnectProjectsLocationsConnectionsGitRepositoryLinksProcessBitbucketCloudWebhookArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = developerconnect_projects_locations_connections_git_repository_links_process_bitbucket_cloud_webhook_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = developerconnect_projects_locations_connections_git_repository_links_process_bitbucket_cloud_webhook_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Developerconnect projects locations connections git repository links process bitbucket data center webhook.
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
    pub fn developerconnect_projects_locations_connections_git_repository_links_process_bitbucket_data_center_webhook(
        &self,
        args: &DeveloperconnectProjectsLocationsConnectionsGitRepositoryLinksProcessBitbucketDataCenterWebhookArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = developerconnect_projects_locations_connections_git_repository_links_process_bitbucket_data_center_webhook_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = developerconnect_projects_locations_connections_git_repository_links_process_bitbucket_data_center_webhook_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Developerconnect projects locations connections git repository links process git lab enterprise webhook.
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
    pub fn developerconnect_projects_locations_connections_git_repository_links_process_git_lab_enterprise_webhook(
        &self,
        args: &DeveloperconnectProjectsLocationsConnectionsGitRepositoryLinksProcessGitLabEnterpriseWebhookArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = developerconnect_projects_locations_connections_git_repository_links_process_git_lab_enterprise_webhook_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = developerconnect_projects_locations_connections_git_repository_links_process_git_lab_enterprise_webhook_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Developerconnect projects locations connections git repository links process git lab webhook.
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
    pub fn developerconnect_projects_locations_connections_git_repository_links_process_git_lab_webhook(
        &self,
        args: &DeveloperconnectProjectsLocationsConnectionsGitRepositoryLinksProcessGitLabWebhookArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = developerconnect_projects_locations_connections_git_repository_links_process_git_lab_webhook_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = developerconnect_projects_locations_connections_git_repository_links_process_git_lab_webhook_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Developerconnect projects locations insights configs create.
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
    pub fn developerconnect_projects_locations_insights_configs_create(
        &self,
        args: &DeveloperconnectProjectsLocationsInsightsConfigsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = developerconnect_projects_locations_insights_configs_create_builder(
            &self.http_client,
            &args.parent,
            &args.insightsConfigId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = developerconnect_projects_locations_insights_configs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Developerconnect projects locations insights configs delete.
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
    pub fn developerconnect_projects_locations_insights_configs_delete(
        &self,
        args: &DeveloperconnectProjectsLocationsInsightsConfigsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = developerconnect_projects_locations_insights_configs_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = developerconnect_projects_locations_insights_configs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Developerconnect projects locations insights configs get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the InsightsConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn developerconnect_projects_locations_insights_configs_get(
        &self,
        args: &DeveloperconnectProjectsLocationsInsightsConfigsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<InsightsConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = developerconnect_projects_locations_insights_configs_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = developerconnect_projects_locations_insights_configs_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Developerconnect projects locations insights configs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListInsightsConfigsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn developerconnect_projects_locations_insights_configs_list(
        &self,
        args: &DeveloperconnectProjectsLocationsInsightsConfigsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListInsightsConfigsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = developerconnect_projects_locations_insights_configs_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = developerconnect_projects_locations_insights_configs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Developerconnect projects locations insights configs patch.
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
    pub fn developerconnect_projects_locations_insights_configs_patch(
        &self,
        args: &DeveloperconnectProjectsLocationsInsightsConfigsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = developerconnect_projects_locations_insights_configs_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = developerconnect_projects_locations_insights_configs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Developerconnect projects locations insights configs deployment events get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeploymentEvent result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn developerconnect_projects_locations_insights_configs_deployment_events_get(
        &self,
        args: &DeveloperconnectProjectsLocationsInsightsConfigsDeploymentEventsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeploymentEvent, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = developerconnect_projects_locations_insights_configs_deployment_events_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = developerconnect_projects_locations_insights_configs_deployment_events_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Developerconnect projects locations insights configs deployment events list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDeploymentEventsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn developerconnect_projects_locations_insights_configs_deployment_events_list(
        &self,
        args: &DeveloperconnectProjectsLocationsInsightsConfigsDeploymentEventsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDeploymentEventsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = developerconnect_projects_locations_insights_configs_deployment_events_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = developerconnect_projects_locations_insights_configs_deployment_events_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Developerconnect projects locations operations cancel.
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
    pub fn developerconnect_projects_locations_operations_cancel(
        &self,
        args: &DeveloperconnectProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = developerconnect_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = developerconnect_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Developerconnect projects locations operations delete.
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
    pub fn developerconnect_projects_locations_operations_delete(
        &self,
        args: &DeveloperconnectProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = developerconnect_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = developerconnect_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Developerconnect projects locations operations get.
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
    pub fn developerconnect_projects_locations_operations_get(
        &self,
        args: &DeveloperconnectProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = developerconnect_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = developerconnect_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Developerconnect projects locations operations list.
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
    pub fn developerconnect_projects_locations_operations_list(
        &self,
        args: &DeveloperconnectProjectsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = developerconnect_projects_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = developerconnect_projects_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
