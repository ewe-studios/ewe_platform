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
    cloudbuild_projects_locations_connections_create_builder, cloudbuild_projects_locations_connections_create_task,
    cloudbuild_projects_locations_connections_delete_builder, cloudbuild_projects_locations_connections_delete_task,
    cloudbuild_projects_locations_connections_patch_builder, cloudbuild_projects_locations_connections_patch_task,
    cloudbuild_projects_locations_connections_process_webhook_builder, cloudbuild_projects_locations_connections_process_webhook_task,
    cloudbuild_projects_locations_connections_set_iam_policy_builder, cloudbuild_projects_locations_connections_set_iam_policy_task,
    cloudbuild_projects_locations_connections_test_iam_permissions_builder, cloudbuild_projects_locations_connections_test_iam_permissions_task,
    cloudbuild_projects_locations_connections_repositories_access_read_token_builder, cloudbuild_projects_locations_connections_repositories_access_read_token_task,
    cloudbuild_projects_locations_connections_repositories_access_read_write_token_builder, cloudbuild_projects_locations_connections_repositories_access_read_write_token_task,
    cloudbuild_projects_locations_connections_repositories_batch_create_builder, cloudbuild_projects_locations_connections_repositories_batch_create_task,
    cloudbuild_projects_locations_connections_repositories_create_builder, cloudbuild_projects_locations_connections_repositories_create_task,
    cloudbuild_projects_locations_connections_repositories_delete_builder, cloudbuild_projects_locations_connections_repositories_delete_task,
    cloudbuild_projects_locations_operations_cancel_builder, cloudbuild_projects_locations_operations_cancel_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::cloudbuild::Empty;
use crate::providers::gcp::clients::cloudbuild::FetchReadTokenResponse;
use crate::providers::gcp::clients::cloudbuild::FetchReadWriteTokenResponse;
use crate::providers::gcp::clients::cloudbuild::Operation;
use crate::providers::gcp::clients::cloudbuild::Policy;
use crate::providers::gcp::clients::cloudbuild::TestIamPermissionsResponse;
use crate::providers::gcp::clients::cloudbuild::CloudbuildProjectsLocationsConnectionsCreateArgs;
use crate::providers::gcp::clients::cloudbuild::CloudbuildProjectsLocationsConnectionsDeleteArgs;
use crate::providers::gcp::clients::cloudbuild::CloudbuildProjectsLocationsConnectionsPatchArgs;
use crate::providers::gcp::clients::cloudbuild::CloudbuildProjectsLocationsConnectionsProcessWebhookArgs;
use crate::providers::gcp::clients::cloudbuild::CloudbuildProjectsLocationsConnectionsRepositoriesAccessReadTokenArgs;
use crate::providers::gcp::clients::cloudbuild::CloudbuildProjectsLocationsConnectionsRepositoriesAccessReadWriteTokenArgs;
use crate::providers::gcp::clients::cloudbuild::CloudbuildProjectsLocationsConnectionsRepositoriesBatchCreateArgs;
use crate::providers::gcp::clients::cloudbuild::CloudbuildProjectsLocationsConnectionsRepositoriesCreateArgs;
use crate::providers::gcp::clients::cloudbuild::CloudbuildProjectsLocationsConnectionsRepositoriesDeleteArgs;
use crate::providers::gcp::clients::cloudbuild::CloudbuildProjectsLocationsConnectionsSetIamPolicyArgs;
use crate::providers::gcp::clients::cloudbuild::CloudbuildProjectsLocationsConnectionsTestIamPermissionsArgs;
use crate::providers::gcp::clients::cloudbuild::CloudbuildProjectsLocationsOperationsCancelArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// CloudbuildProvider with automatic state tracking.
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
/// let provider = CloudbuildProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct CloudbuildProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> CloudbuildProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new CloudbuildProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

}
