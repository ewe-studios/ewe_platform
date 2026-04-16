//! CloudbuildProvider - State-aware cloudbuild API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       cloudbuild API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::cloudbuild::{
    cloudbuild_projects_locations_get_builder, cloudbuild_projects_locations_get_task,
    cloudbuild_projects_locations_list_builder, cloudbuild_projects_locations_list_task,
    cloudbuild_projects_locations_connections_create_builder, cloudbuild_projects_locations_connections_create_task,
    cloudbuild_projects_locations_connections_delete_builder, cloudbuild_projects_locations_connections_delete_task,
    cloudbuild_projects_locations_connections_fetch_linkable_repositories_builder, cloudbuild_projects_locations_connections_fetch_linkable_repositories_task,
    cloudbuild_projects_locations_connections_get_builder, cloudbuild_projects_locations_connections_get_task,
    cloudbuild_projects_locations_connections_get_iam_policy_builder, cloudbuild_projects_locations_connections_get_iam_policy_task,
    cloudbuild_projects_locations_connections_list_builder, cloudbuild_projects_locations_connections_list_task,
    cloudbuild_projects_locations_connections_patch_builder, cloudbuild_projects_locations_connections_patch_task,
    cloudbuild_projects_locations_connections_process_webhook_builder, cloudbuild_projects_locations_connections_process_webhook_task,
    cloudbuild_projects_locations_connections_set_iam_policy_builder, cloudbuild_projects_locations_connections_set_iam_policy_task,
    cloudbuild_projects_locations_connections_test_iam_permissions_builder, cloudbuild_projects_locations_connections_test_iam_permissions_task,
    cloudbuild_projects_locations_connections_repositories_access_read_token_builder, cloudbuild_projects_locations_connections_repositories_access_read_token_task,
    cloudbuild_projects_locations_connections_repositories_access_read_write_token_builder, cloudbuild_projects_locations_connections_repositories_access_read_write_token_task,
    cloudbuild_projects_locations_connections_repositories_batch_create_builder, cloudbuild_projects_locations_connections_repositories_batch_create_task,
    cloudbuild_projects_locations_connections_repositories_create_builder, cloudbuild_projects_locations_connections_repositories_create_task,
    cloudbuild_projects_locations_connections_repositories_delete_builder, cloudbuild_projects_locations_connections_repositories_delete_task,
    cloudbuild_projects_locations_connections_repositories_fetch_git_refs_builder, cloudbuild_projects_locations_connections_repositories_fetch_git_refs_task,
    cloudbuild_projects_locations_connections_repositories_get_builder, cloudbuild_projects_locations_connections_repositories_get_task,
    cloudbuild_projects_locations_connections_repositories_list_builder, cloudbuild_projects_locations_connections_repositories_list_task,
    cloudbuild_projects_locations_operations_cancel_builder, cloudbuild_projects_locations_operations_cancel_task,
    cloudbuild_projects_locations_operations_get_builder, cloudbuild_projects_locations_operations_get_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::cloudbuild::Connection;
use crate::providers::gcp::clients::cloudbuild::Empty;
use crate::providers::gcp::clients::cloudbuild::FetchGitRefsResponse;
use crate::providers::gcp::clients::cloudbuild::FetchLinkableRepositoriesResponse;
use crate::providers::gcp::clients::cloudbuild::FetchReadTokenResponse;
use crate::providers::gcp::clients::cloudbuild::FetchReadWriteTokenResponse;
use crate::providers::gcp::clients::cloudbuild::ListConnectionsResponse;
use crate::providers::gcp::clients::cloudbuild::ListLocationsResponse;
use crate::providers::gcp::clients::cloudbuild::ListRepositoriesResponse;
use crate::providers::gcp::clients::cloudbuild::Location;
use crate::providers::gcp::clients::cloudbuild::Operation;
use crate::providers::gcp::clients::cloudbuild::Policy;
use crate::providers::gcp::clients::cloudbuild::Repository;
use crate::providers::gcp::clients::cloudbuild::TestIamPermissionsResponse;
use crate::providers::gcp::clients::cloudbuild::CloudbuildProjectsLocationsConnectionsCreateArgs;
use crate::providers::gcp::clients::cloudbuild::CloudbuildProjectsLocationsConnectionsDeleteArgs;
use crate::providers::gcp::clients::cloudbuild::CloudbuildProjectsLocationsConnectionsFetchLinkableRepositoriesArgs;
use crate::providers::gcp::clients::cloudbuild::CloudbuildProjectsLocationsConnectionsGetArgs;
use crate::providers::gcp::clients::cloudbuild::CloudbuildProjectsLocationsConnectionsGetIamPolicyArgs;
use crate::providers::gcp::clients::cloudbuild::CloudbuildProjectsLocationsConnectionsListArgs;
use crate::providers::gcp::clients::cloudbuild::CloudbuildProjectsLocationsConnectionsPatchArgs;
use crate::providers::gcp::clients::cloudbuild::CloudbuildProjectsLocationsConnectionsProcessWebhookArgs;
use crate::providers::gcp::clients::cloudbuild::CloudbuildProjectsLocationsConnectionsRepositoriesAccessReadTokenArgs;
use crate::providers::gcp::clients::cloudbuild::CloudbuildProjectsLocationsConnectionsRepositoriesAccessReadWriteTokenArgs;
use crate::providers::gcp::clients::cloudbuild::CloudbuildProjectsLocationsConnectionsRepositoriesBatchCreateArgs;
use crate::providers::gcp::clients::cloudbuild::CloudbuildProjectsLocationsConnectionsRepositoriesCreateArgs;
use crate::providers::gcp::clients::cloudbuild::CloudbuildProjectsLocationsConnectionsRepositoriesDeleteArgs;
use crate::providers::gcp::clients::cloudbuild::CloudbuildProjectsLocationsConnectionsRepositoriesFetchGitRefsArgs;
use crate::providers::gcp::clients::cloudbuild::CloudbuildProjectsLocationsConnectionsRepositoriesGetArgs;
use crate::providers::gcp::clients::cloudbuild::CloudbuildProjectsLocationsConnectionsRepositoriesListArgs;
use crate::providers::gcp::clients::cloudbuild::CloudbuildProjectsLocationsConnectionsSetIamPolicyArgs;
use crate::providers::gcp::clients::cloudbuild::CloudbuildProjectsLocationsConnectionsTestIamPermissionsArgs;
use crate::providers::gcp::clients::cloudbuild::CloudbuildProjectsLocationsGetArgs;
use crate::providers::gcp::clients::cloudbuild::CloudbuildProjectsLocationsListArgs;
use crate::providers::gcp::clients::cloudbuild::CloudbuildProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::cloudbuild::CloudbuildProjectsLocationsOperationsGetArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// CloudbuildProvider with automatic state tracking.
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
/// let provider = CloudbuildProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct CloudbuildProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> CloudbuildProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new CloudbuildProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new CloudbuildProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Cloudbuild projects locations get.
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
    pub fn cloudbuild_projects_locations_get(
        &self,
        args: &CloudbuildProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudbuild_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudbuild_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudbuild projects locations list.
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
    pub fn cloudbuild_projects_locations_list(
        &self,
        args: &CloudbuildProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudbuild_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudbuild_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudbuild projects locations connections create.
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
    pub fn cloudbuild_projects_locations_connections_create(
        &self,
        args: &CloudbuildProjectsLocationsConnectionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudbuild_projects_locations_connections_create_builder(
            &self.http_client,
            &args.parent,
            &args.connectionId,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudbuild_projects_locations_connections_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudbuild projects locations connections delete.
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
    pub fn cloudbuild_projects_locations_connections_delete(
        &self,
        args: &CloudbuildProjectsLocationsConnectionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudbuild_projects_locations_connections_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudbuild_projects_locations_connections_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudbuild projects locations connections fetch linkable repositories.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FetchLinkableRepositoriesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudbuild_projects_locations_connections_fetch_linkable_repositories(
        &self,
        args: &CloudbuildProjectsLocationsConnectionsFetchLinkableRepositoriesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FetchLinkableRepositoriesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudbuild_projects_locations_connections_fetch_linkable_repositories_builder(
            &self.http_client,
            &args.connection,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudbuild_projects_locations_connections_fetch_linkable_repositories_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudbuild projects locations connections get.
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
    pub fn cloudbuild_projects_locations_connections_get(
        &self,
        args: &CloudbuildProjectsLocationsConnectionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Connection, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudbuild_projects_locations_connections_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudbuild_projects_locations_connections_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudbuild projects locations connections get iam policy.
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
    pub fn cloudbuild_projects_locations_connections_get_iam_policy(
        &self,
        args: &CloudbuildProjectsLocationsConnectionsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudbuild_projects_locations_connections_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options_requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudbuild_projects_locations_connections_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudbuild projects locations connections list.
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
    pub fn cloudbuild_projects_locations_connections_list(
        &self,
        args: &CloudbuildProjectsLocationsConnectionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListConnectionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudbuild_projects_locations_connections_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudbuild_projects_locations_connections_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudbuild projects locations connections patch.
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
    pub fn cloudbuild_projects_locations_connections_patch(
        &self,
        args: &CloudbuildProjectsLocationsConnectionsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudbuild_projects_locations_connections_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.etag,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudbuild_projects_locations_connections_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudbuild projects locations connections process webhook.
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
    pub fn cloudbuild_projects_locations_connections_process_webhook(
        &self,
        args: &CloudbuildProjectsLocationsConnectionsProcessWebhookArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudbuild_projects_locations_connections_process_webhook_builder(
            &self.http_client,
            &args.parent,
            &args.webhookKey,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudbuild_projects_locations_connections_process_webhook_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudbuild projects locations connections set iam policy.
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
    pub fn cloudbuild_projects_locations_connections_set_iam_policy(
        &self,
        args: &CloudbuildProjectsLocationsConnectionsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudbuild_projects_locations_connections_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudbuild_projects_locations_connections_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudbuild projects locations connections test iam permissions.
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
    pub fn cloudbuild_projects_locations_connections_test_iam_permissions(
        &self,
        args: &CloudbuildProjectsLocationsConnectionsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudbuild_projects_locations_connections_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudbuild_projects_locations_connections_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudbuild projects locations connections repositories access read token.
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
    pub fn cloudbuild_projects_locations_connections_repositories_access_read_token(
        &self,
        args: &CloudbuildProjectsLocationsConnectionsRepositoriesAccessReadTokenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FetchReadTokenResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudbuild_projects_locations_connections_repositories_access_read_token_builder(
            &self.http_client,
            &args.repository,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudbuild_projects_locations_connections_repositories_access_read_token_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudbuild projects locations connections repositories access read write token.
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
    pub fn cloudbuild_projects_locations_connections_repositories_access_read_write_token(
        &self,
        args: &CloudbuildProjectsLocationsConnectionsRepositoriesAccessReadWriteTokenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FetchReadWriteTokenResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudbuild_projects_locations_connections_repositories_access_read_write_token_builder(
            &self.http_client,
            &args.repository,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudbuild_projects_locations_connections_repositories_access_read_write_token_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudbuild projects locations connections repositories batch create.
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
    pub fn cloudbuild_projects_locations_connections_repositories_batch_create(
        &self,
        args: &CloudbuildProjectsLocationsConnectionsRepositoriesBatchCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudbuild_projects_locations_connections_repositories_batch_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudbuild_projects_locations_connections_repositories_batch_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudbuild projects locations connections repositories create.
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
    pub fn cloudbuild_projects_locations_connections_repositories_create(
        &self,
        args: &CloudbuildProjectsLocationsConnectionsRepositoriesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudbuild_projects_locations_connections_repositories_create_builder(
            &self.http_client,
            &args.parent,
            &args.repositoryId,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudbuild_projects_locations_connections_repositories_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudbuild projects locations connections repositories delete.
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
    pub fn cloudbuild_projects_locations_connections_repositories_delete(
        &self,
        args: &CloudbuildProjectsLocationsConnectionsRepositoriesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudbuild_projects_locations_connections_repositories_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudbuild_projects_locations_connections_repositories_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudbuild projects locations connections repositories fetch git refs.
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
    pub fn cloudbuild_projects_locations_connections_repositories_fetch_git_refs(
        &self,
        args: &CloudbuildProjectsLocationsConnectionsRepositoriesFetchGitRefsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FetchGitRefsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudbuild_projects_locations_connections_repositories_fetch_git_refs_builder(
            &self.http_client,
            &args.repository,
            &args.pageSize,
            &args.pageToken,
            &args.refType,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudbuild_projects_locations_connections_repositories_fetch_git_refs_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudbuild projects locations connections repositories get.
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
    pub fn cloudbuild_projects_locations_connections_repositories_get(
        &self,
        args: &CloudbuildProjectsLocationsConnectionsRepositoriesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Repository, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudbuild_projects_locations_connections_repositories_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudbuild_projects_locations_connections_repositories_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudbuild projects locations connections repositories list.
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
    pub fn cloudbuild_projects_locations_connections_repositories_list(
        &self,
        args: &CloudbuildProjectsLocationsConnectionsRepositoriesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListRepositoriesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudbuild_projects_locations_connections_repositories_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudbuild_projects_locations_connections_repositories_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudbuild projects locations operations cancel.
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
    pub fn cloudbuild_projects_locations_operations_cancel(
        &self,
        args: &CloudbuildProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudbuild_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudbuild_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudbuild projects locations operations get.
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
    pub fn cloudbuild_projects_locations_operations_get(
        &self,
        args: &CloudbuildProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudbuild_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudbuild_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
