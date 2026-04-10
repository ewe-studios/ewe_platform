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
    developerconnect_projects_locations_account_connectors_create_builder, developerconnect_projects_locations_account_connectors_create_task,
    developerconnect_projects_locations_account_connectors_delete_builder, developerconnect_projects_locations_account_connectors_delete_task,
    developerconnect_projects_locations_account_connectors_patch_builder, developerconnect_projects_locations_account_connectors_patch_task,
    developerconnect_projects_locations_account_connectors_users_delete_builder, developerconnect_projects_locations_account_connectors_users_delete_task,
    developerconnect_projects_locations_account_connectors_users_delete_self_builder, developerconnect_projects_locations_account_connectors_users_delete_self_task,
    developerconnect_projects_locations_account_connectors_users_fetch_access_token_builder, developerconnect_projects_locations_account_connectors_users_fetch_access_token_task,
    developerconnect_projects_locations_connections_create_builder, developerconnect_projects_locations_connections_create_task,
    developerconnect_projects_locations_connections_delete_builder, developerconnect_projects_locations_connections_delete_task,
    developerconnect_projects_locations_connections_patch_builder, developerconnect_projects_locations_connections_patch_task,
    developerconnect_projects_locations_connections_process_git_hub_enterprise_webhook_builder, developerconnect_projects_locations_connections_process_git_hub_enterprise_webhook_task,
    developerconnect_projects_locations_connections_git_repository_links_create_builder, developerconnect_projects_locations_connections_git_repository_links_create_task,
    developerconnect_projects_locations_connections_git_repository_links_delete_builder, developerconnect_projects_locations_connections_git_repository_links_delete_task,
    developerconnect_projects_locations_connections_git_repository_links_fetch_read_token_builder, developerconnect_projects_locations_connections_git_repository_links_fetch_read_token_task,
    developerconnect_projects_locations_connections_git_repository_links_fetch_read_write_token_builder, developerconnect_projects_locations_connections_git_repository_links_fetch_read_write_token_task,
    developerconnect_projects_locations_connections_git_repository_links_process_bitbucket_cloud_webhook_builder, developerconnect_projects_locations_connections_git_repository_links_process_bitbucket_cloud_webhook_task,
    developerconnect_projects_locations_connections_git_repository_links_process_bitbucket_data_center_webhook_builder, developerconnect_projects_locations_connections_git_repository_links_process_bitbucket_data_center_webhook_task,
    developerconnect_projects_locations_connections_git_repository_links_process_git_lab_enterprise_webhook_builder, developerconnect_projects_locations_connections_git_repository_links_process_git_lab_enterprise_webhook_task,
    developerconnect_projects_locations_connections_git_repository_links_process_git_lab_webhook_builder, developerconnect_projects_locations_connections_git_repository_links_process_git_lab_webhook_task,
    developerconnect_projects_locations_insights_configs_create_builder, developerconnect_projects_locations_insights_configs_create_task,
    developerconnect_projects_locations_insights_configs_delete_builder, developerconnect_projects_locations_insights_configs_delete_task,
    developerconnect_projects_locations_insights_configs_patch_builder, developerconnect_projects_locations_insights_configs_patch_task,
    developerconnect_projects_locations_operations_cancel_builder, developerconnect_projects_locations_operations_cancel_task,
    developerconnect_projects_locations_operations_delete_builder, developerconnect_projects_locations_operations_delete_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::developerconnect::Empty;
use crate::providers::gcp::clients::developerconnect::FetchAccessTokenResponse;
use crate::providers::gcp::clients::developerconnect::FetchReadTokenResponse;
use crate::providers::gcp::clients::developerconnect::FetchReadWriteTokenResponse;
use crate::providers::gcp::clients::developerconnect::Operation;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsAccountConnectorsCreateArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsAccountConnectorsDeleteArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsAccountConnectorsPatchArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsAccountConnectorsUsersDeleteArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsAccountConnectorsUsersDeleteSelfArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsAccountConnectorsUsersFetchAccessTokenArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsConnectionsCreateArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsConnectionsDeleteArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsConnectionsGitRepositoryLinksCreateArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsConnectionsGitRepositoryLinksDeleteArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsConnectionsGitRepositoryLinksFetchReadTokenArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsConnectionsGitRepositoryLinksFetchReadWriteTokenArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsConnectionsGitRepositoryLinksProcessBitbucketCloudWebhookArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsConnectionsGitRepositoryLinksProcessBitbucketDataCenterWebhookArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsConnectionsGitRepositoryLinksProcessGitLabEnterpriseWebhookArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsConnectionsGitRepositoryLinksProcessGitLabWebhookArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsConnectionsPatchArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsConnectionsProcessGitHubEnterpriseWebhookArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsInsightsConfigsCreateArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsInsightsConfigsDeleteArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsInsightsConfigsPatchArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::developerconnect::DeveloperconnectProjectsLocationsOperationsDeleteArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// DeveloperconnectProvider with automatic state tracking.
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
/// let provider = DeveloperconnectProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct DeveloperconnectProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> DeveloperconnectProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new DeveloperconnectProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
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
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
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

    /// Developerconnect projects locations connections git repository links fetch read token.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Developerconnect projects locations connections git repository links fetch read write token.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

}
