//! SecretmanagerProvider - State-aware secretmanager API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       secretmanager API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::secretmanager::{
    secretmanager_projects_locations_get_builder, secretmanager_projects_locations_get_task,
    secretmanager_projects_locations_list_builder, secretmanager_projects_locations_list_task,
    secretmanager_projects_locations_secrets_add_version_builder, secretmanager_projects_locations_secrets_add_version_task,
    secretmanager_projects_locations_secrets_create_builder, secretmanager_projects_locations_secrets_create_task,
    secretmanager_projects_locations_secrets_delete_builder, secretmanager_projects_locations_secrets_delete_task,
    secretmanager_projects_locations_secrets_get_builder, secretmanager_projects_locations_secrets_get_task,
    secretmanager_projects_locations_secrets_get_iam_policy_builder, secretmanager_projects_locations_secrets_get_iam_policy_task,
    secretmanager_projects_locations_secrets_list_builder, secretmanager_projects_locations_secrets_list_task,
    secretmanager_projects_locations_secrets_patch_builder, secretmanager_projects_locations_secrets_patch_task,
    secretmanager_projects_locations_secrets_set_iam_policy_builder, secretmanager_projects_locations_secrets_set_iam_policy_task,
    secretmanager_projects_locations_secrets_test_iam_permissions_builder, secretmanager_projects_locations_secrets_test_iam_permissions_task,
    secretmanager_projects_locations_secrets_versions_access_builder, secretmanager_projects_locations_secrets_versions_access_task,
    secretmanager_projects_locations_secrets_versions_destroy_builder, secretmanager_projects_locations_secrets_versions_destroy_task,
    secretmanager_projects_locations_secrets_versions_disable_builder, secretmanager_projects_locations_secrets_versions_disable_task,
    secretmanager_projects_locations_secrets_versions_enable_builder, secretmanager_projects_locations_secrets_versions_enable_task,
    secretmanager_projects_locations_secrets_versions_get_builder, secretmanager_projects_locations_secrets_versions_get_task,
    secretmanager_projects_locations_secrets_versions_list_builder, secretmanager_projects_locations_secrets_versions_list_task,
    secretmanager_projects_secrets_add_version_builder, secretmanager_projects_secrets_add_version_task,
    secretmanager_projects_secrets_create_builder, secretmanager_projects_secrets_create_task,
    secretmanager_projects_secrets_delete_builder, secretmanager_projects_secrets_delete_task,
    secretmanager_projects_secrets_get_builder, secretmanager_projects_secrets_get_task,
    secretmanager_projects_secrets_get_iam_policy_builder, secretmanager_projects_secrets_get_iam_policy_task,
    secretmanager_projects_secrets_list_builder, secretmanager_projects_secrets_list_task,
    secretmanager_projects_secrets_patch_builder, secretmanager_projects_secrets_patch_task,
    secretmanager_projects_secrets_set_iam_policy_builder, secretmanager_projects_secrets_set_iam_policy_task,
    secretmanager_projects_secrets_test_iam_permissions_builder, secretmanager_projects_secrets_test_iam_permissions_task,
    secretmanager_projects_secrets_versions_access_builder, secretmanager_projects_secrets_versions_access_task,
    secretmanager_projects_secrets_versions_destroy_builder, secretmanager_projects_secrets_versions_destroy_task,
    secretmanager_projects_secrets_versions_disable_builder, secretmanager_projects_secrets_versions_disable_task,
    secretmanager_projects_secrets_versions_enable_builder, secretmanager_projects_secrets_versions_enable_task,
    secretmanager_projects_secrets_versions_get_builder, secretmanager_projects_secrets_versions_get_task,
    secretmanager_projects_secrets_versions_list_builder, secretmanager_projects_secrets_versions_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::secretmanager::AccessSecretVersionResponse;
use crate::providers::gcp::clients::secretmanager::Empty;
use crate::providers::gcp::clients::secretmanager::ListLocationsResponse;
use crate::providers::gcp::clients::secretmanager::ListSecretVersionsResponse;
use crate::providers::gcp::clients::secretmanager::ListSecretsResponse;
use crate::providers::gcp::clients::secretmanager::Location;
use crate::providers::gcp::clients::secretmanager::Policy;
use crate::providers::gcp::clients::secretmanager::Secret;
use crate::providers::gcp::clients::secretmanager::SecretVersion;
use crate::providers::gcp::clients::secretmanager::TestIamPermissionsResponse;
use crate::providers::gcp::clients::secretmanager::SecretmanagerProjectsLocationsGetArgs;
use crate::providers::gcp::clients::secretmanager::SecretmanagerProjectsLocationsListArgs;
use crate::providers::gcp::clients::secretmanager::SecretmanagerProjectsLocationsSecretsAddVersionArgs;
use crate::providers::gcp::clients::secretmanager::SecretmanagerProjectsLocationsSecretsCreateArgs;
use crate::providers::gcp::clients::secretmanager::SecretmanagerProjectsLocationsSecretsDeleteArgs;
use crate::providers::gcp::clients::secretmanager::SecretmanagerProjectsLocationsSecretsGetArgs;
use crate::providers::gcp::clients::secretmanager::SecretmanagerProjectsLocationsSecretsGetIamPolicyArgs;
use crate::providers::gcp::clients::secretmanager::SecretmanagerProjectsLocationsSecretsListArgs;
use crate::providers::gcp::clients::secretmanager::SecretmanagerProjectsLocationsSecretsPatchArgs;
use crate::providers::gcp::clients::secretmanager::SecretmanagerProjectsLocationsSecretsSetIamPolicyArgs;
use crate::providers::gcp::clients::secretmanager::SecretmanagerProjectsLocationsSecretsTestIamPermissionsArgs;
use crate::providers::gcp::clients::secretmanager::SecretmanagerProjectsLocationsSecretsVersionsAccessArgs;
use crate::providers::gcp::clients::secretmanager::SecretmanagerProjectsLocationsSecretsVersionsDestroyArgs;
use crate::providers::gcp::clients::secretmanager::SecretmanagerProjectsLocationsSecretsVersionsDisableArgs;
use crate::providers::gcp::clients::secretmanager::SecretmanagerProjectsLocationsSecretsVersionsEnableArgs;
use crate::providers::gcp::clients::secretmanager::SecretmanagerProjectsLocationsSecretsVersionsGetArgs;
use crate::providers::gcp::clients::secretmanager::SecretmanagerProjectsLocationsSecretsVersionsListArgs;
use crate::providers::gcp::clients::secretmanager::SecretmanagerProjectsSecretsAddVersionArgs;
use crate::providers::gcp::clients::secretmanager::SecretmanagerProjectsSecretsCreateArgs;
use crate::providers::gcp::clients::secretmanager::SecretmanagerProjectsSecretsDeleteArgs;
use crate::providers::gcp::clients::secretmanager::SecretmanagerProjectsSecretsGetArgs;
use crate::providers::gcp::clients::secretmanager::SecretmanagerProjectsSecretsGetIamPolicyArgs;
use crate::providers::gcp::clients::secretmanager::SecretmanagerProjectsSecretsListArgs;
use crate::providers::gcp::clients::secretmanager::SecretmanagerProjectsSecretsPatchArgs;
use crate::providers::gcp::clients::secretmanager::SecretmanagerProjectsSecretsSetIamPolicyArgs;
use crate::providers::gcp::clients::secretmanager::SecretmanagerProjectsSecretsTestIamPermissionsArgs;
use crate::providers::gcp::clients::secretmanager::SecretmanagerProjectsSecretsVersionsAccessArgs;
use crate::providers::gcp::clients::secretmanager::SecretmanagerProjectsSecretsVersionsDestroyArgs;
use crate::providers::gcp::clients::secretmanager::SecretmanagerProjectsSecretsVersionsDisableArgs;
use crate::providers::gcp::clients::secretmanager::SecretmanagerProjectsSecretsVersionsEnableArgs;
use crate::providers::gcp::clients::secretmanager::SecretmanagerProjectsSecretsVersionsGetArgs;
use crate::providers::gcp::clients::secretmanager::SecretmanagerProjectsSecretsVersionsListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// SecretmanagerProvider with automatic state tracking.
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
/// let provider = SecretmanagerProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct SecretmanagerProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> SecretmanagerProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new SecretmanagerProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Secretmanager projects locations get.
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
    pub fn secretmanager_projects_locations_get(
        &self,
        args: &SecretmanagerProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = secretmanager_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = secretmanager_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Secretmanager projects locations list.
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
    pub fn secretmanager_projects_locations_list(
        &self,
        args: &SecretmanagerProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = secretmanager_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = secretmanager_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Secretmanager projects locations secrets add version.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SecretVersion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn secretmanager_projects_locations_secrets_add_version(
        &self,
        args: &SecretmanagerProjectsLocationsSecretsAddVersionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SecretVersion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = secretmanager_projects_locations_secrets_add_version_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = secretmanager_projects_locations_secrets_add_version_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Secretmanager projects locations secrets create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Secret result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn secretmanager_projects_locations_secrets_create(
        &self,
        args: &SecretmanagerProjectsLocationsSecretsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Secret, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = secretmanager_projects_locations_secrets_create_builder(
            &self.http_client,
            &args.parent,
            &args.secretId,
        )
        .map_err(ProviderError::Api)?;

        let task = secretmanager_projects_locations_secrets_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Secretmanager projects locations secrets delete.
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
    pub fn secretmanager_projects_locations_secrets_delete(
        &self,
        args: &SecretmanagerProjectsLocationsSecretsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = secretmanager_projects_locations_secrets_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
        )
        .map_err(ProviderError::Api)?;

        let task = secretmanager_projects_locations_secrets_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Secretmanager projects locations secrets get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Secret result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn secretmanager_projects_locations_secrets_get(
        &self,
        args: &SecretmanagerProjectsLocationsSecretsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Secret, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = secretmanager_projects_locations_secrets_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = secretmanager_projects_locations_secrets_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Secretmanager projects locations secrets get iam policy.
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
    pub fn secretmanager_projects_locations_secrets_get_iam_policy(
        &self,
        args: &SecretmanagerProjectsLocationsSecretsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = secretmanager_projects_locations_secrets_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = secretmanager_projects_locations_secrets_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Secretmanager projects locations secrets list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListSecretsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn secretmanager_projects_locations_secrets_list(
        &self,
        args: &SecretmanagerProjectsLocationsSecretsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSecretsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = secretmanager_projects_locations_secrets_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = secretmanager_projects_locations_secrets_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Secretmanager projects locations secrets patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Secret result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn secretmanager_projects_locations_secrets_patch(
        &self,
        args: &SecretmanagerProjectsLocationsSecretsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Secret, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = secretmanager_projects_locations_secrets_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = secretmanager_projects_locations_secrets_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Secretmanager projects locations secrets set iam policy.
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
    pub fn secretmanager_projects_locations_secrets_set_iam_policy(
        &self,
        args: &SecretmanagerProjectsLocationsSecretsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = secretmanager_projects_locations_secrets_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = secretmanager_projects_locations_secrets_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Secretmanager projects locations secrets test iam permissions.
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
    pub fn secretmanager_projects_locations_secrets_test_iam_permissions(
        &self,
        args: &SecretmanagerProjectsLocationsSecretsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = secretmanager_projects_locations_secrets_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = secretmanager_projects_locations_secrets_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Secretmanager projects locations secrets versions access.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccessSecretVersionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn secretmanager_projects_locations_secrets_versions_access(
        &self,
        args: &SecretmanagerProjectsLocationsSecretsVersionsAccessArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccessSecretVersionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = secretmanager_projects_locations_secrets_versions_access_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = secretmanager_projects_locations_secrets_versions_access_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Secretmanager projects locations secrets versions destroy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SecretVersion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn secretmanager_projects_locations_secrets_versions_destroy(
        &self,
        args: &SecretmanagerProjectsLocationsSecretsVersionsDestroyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SecretVersion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = secretmanager_projects_locations_secrets_versions_destroy_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = secretmanager_projects_locations_secrets_versions_destroy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Secretmanager projects locations secrets versions disable.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SecretVersion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn secretmanager_projects_locations_secrets_versions_disable(
        &self,
        args: &SecretmanagerProjectsLocationsSecretsVersionsDisableArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SecretVersion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = secretmanager_projects_locations_secrets_versions_disable_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = secretmanager_projects_locations_secrets_versions_disable_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Secretmanager projects locations secrets versions enable.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SecretVersion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn secretmanager_projects_locations_secrets_versions_enable(
        &self,
        args: &SecretmanagerProjectsLocationsSecretsVersionsEnableArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SecretVersion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = secretmanager_projects_locations_secrets_versions_enable_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = secretmanager_projects_locations_secrets_versions_enable_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Secretmanager projects locations secrets versions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SecretVersion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn secretmanager_projects_locations_secrets_versions_get(
        &self,
        args: &SecretmanagerProjectsLocationsSecretsVersionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SecretVersion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = secretmanager_projects_locations_secrets_versions_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = secretmanager_projects_locations_secrets_versions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Secretmanager projects locations secrets versions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListSecretVersionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn secretmanager_projects_locations_secrets_versions_list(
        &self,
        args: &SecretmanagerProjectsLocationsSecretsVersionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSecretVersionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = secretmanager_projects_locations_secrets_versions_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = secretmanager_projects_locations_secrets_versions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Secretmanager projects secrets add version.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SecretVersion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn secretmanager_projects_secrets_add_version(
        &self,
        args: &SecretmanagerProjectsSecretsAddVersionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SecretVersion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = secretmanager_projects_secrets_add_version_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = secretmanager_projects_secrets_add_version_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Secretmanager projects secrets create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Secret result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn secretmanager_projects_secrets_create(
        &self,
        args: &SecretmanagerProjectsSecretsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Secret, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = secretmanager_projects_secrets_create_builder(
            &self.http_client,
            &args.parent,
            &args.secretId,
        )
        .map_err(ProviderError::Api)?;

        let task = secretmanager_projects_secrets_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Secretmanager projects secrets delete.
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
    pub fn secretmanager_projects_secrets_delete(
        &self,
        args: &SecretmanagerProjectsSecretsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = secretmanager_projects_secrets_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
        )
        .map_err(ProviderError::Api)?;

        let task = secretmanager_projects_secrets_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Secretmanager projects secrets get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Secret result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn secretmanager_projects_secrets_get(
        &self,
        args: &SecretmanagerProjectsSecretsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Secret, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = secretmanager_projects_secrets_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = secretmanager_projects_secrets_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Secretmanager projects secrets get iam policy.
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
    pub fn secretmanager_projects_secrets_get_iam_policy(
        &self,
        args: &SecretmanagerProjectsSecretsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = secretmanager_projects_secrets_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = secretmanager_projects_secrets_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Secretmanager projects secrets list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListSecretsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn secretmanager_projects_secrets_list(
        &self,
        args: &SecretmanagerProjectsSecretsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSecretsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = secretmanager_projects_secrets_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = secretmanager_projects_secrets_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Secretmanager projects secrets patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Secret result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn secretmanager_projects_secrets_patch(
        &self,
        args: &SecretmanagerProjectsSecretsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Secret, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = secretmanager_projects_secrets_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = secretmanager_projects_secrets_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Secretmanager projects secrets set iam policy.
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
    pub fn secretmanager_projects_secrets_set_iam_policy(
        &self,
        args: &SecretmanagerProjectsSecretsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = secretmanager_projects_secrets_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = secretmanager_projects_secrets_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Secretmanager projects secrets test iam permissions.
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
    pub fn secretmanager_projects_secrets_test_iam_permissions(
        &self,
        args: &SecretmanagerProjectsSecretsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = secretmanager_projects_secrets_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = secretmanager_projects_secrets_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Secretmanager projects secrets versions access.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccessSecretVersionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn secretmanager_projects_secrets_versions_access(
        &self,
        args: &SecretmanagerProjectsSecretsVersionsAccessArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccessSecretVersionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = secretmanager_projects_secrets_versions_access_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = secretmanager_projects_secrets_versions_access_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Secretmanager projects secrets versions destroy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SecretVersion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn secretmanager_projects_secrets_versions_destroy(
        &self,
        args: &SecretmanagerProjectsSecretsVersionsDestroyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SecretVersion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = secretmanager_projects_secrets_versions_destroy_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = secretmanager_projects_secrets_versions_destroy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Secretmanager projects secrets versions disable.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SecretVersion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn secretmanager_projects_secrets_versions_disable(
        &self,
        args: &SecretmanagerProjectsSecretsVersionsDisableArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SecretVersion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = secretmanager_projects_secrets_versions_disable_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = secretmanager_projects_secrets_versions_disable_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Secretmanager projects secrets versions enable.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SecretVersion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn secretmanager_projects_secrets_versions_enable(
        &self,
        args: &SecretmanagerProjectsSecretsVersionsEnableArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SecretVersion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = secretmanager_projects_secrets_versions_enable_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = secretmanager_projects_secrets_versions_enable_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Secretmanager projects secrets versions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SecretVersion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn secretmanager_projects_secrets_versions_get(
        &self,
        args: &SecretmanagerProjectsSecretsVersionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SecretVersion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = secretmanager_projects_secrets_versions_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = secretmanager_projects_secrets_versions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Secretmanager projects secrets versions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListSecretVersionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn secretmanager_projects_secrets_versions_list(
        &self,
        args: &SecretmanagerProjectsSecretsVersionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSecretVersionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = secretmanager_projects_secrets_versions_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = secretmanager_projects_secrets_versions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
