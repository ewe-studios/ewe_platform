//! BigqueryconnectionProvider - State-aware bigqueryconnection API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       bigqueryconnection API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::bigqueryconnection::{
    bigqueryconnection_projects_locations_connections_create_builder, bigqueryconnection_projects_locations_connections_create_task,
    bigqueryconnection_projects_locations_connections_delete_builder, bigqueryconnection_projects_locations_connections_delete_task,
    bigqueryconnection_projects_locations_connections_get_iam_policy_builder, bigqueryconnection_projects_locations_connections_get_iam_policy_task,
    bigqueryconnection_projects_locations_connections_patch_builder, bigqueryconnection_projects_locations_connections_patch_task,
    bigqueryconnection_projects_locations_connections_set_iam_policy_builder, bigqueryconnection_projects_locations_connections_set_iam_policy_task,
    bigqueryconnection_projects_locations_connections_test_iam_permissions_builder, bigqueryconnection_projects_locations_connections_test_iam_permissions_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::bigqueryconnection::Connection;
use crate::providers::gcp::clients::bigqueryconnection::Empty;
use crate::providers::gcp::clients::bigqueryconnection::Policy;
use crate::providers::gcp::clients::bigqueryconnection::TestIamPermissionsResponse;
use crate::providers::gcp::clients::bigqueryconnection::BigqueryconnectionProjectsLocationsConnectionsCreateArgs;
use crate::providers::gcp::clients::bigqueryconnection::BigqueryconnectionProjectsLocationsConnectionsDeleteArgs;
use crate::providers::gcp::clients::bigqueryconnection::BigqueryconnectionProjectsLocationsConnectionsGetIamPolicyArgs;
use crate::providers::gcp::clients::bigqueryconnection::BigqueryconnectionProjectsLocationsConnectionsPatchArgs;
use crate::providers::gcp::clients::bigqueryconnection::BigqueryconnectionProjectsLocationsConnectionsSetIamPolicyArgs;
use crate::providers::gcp::clients::bigqueryconnection::BigqueryconnectionProjectsLocationsConnectionsTestIamPermissionsArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// BigqueryconnectionProvider with automatic state tracking.
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
/// let provider = BigqueryconnectionProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct BigqueryconnectionProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> BigqueryconnectionProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new BigqueryconnectionProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Bigqueryconnection projects locations connections create.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigqueryconnection_projects_locations_connections_create(
        &self,
        args: &BigqueryconnectionProjectsLocationsConnectionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Connection, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigqueryconnection_projects_locations_connections_create_builder(
            &self.http_client,
            &args.parent,
            &args.connectionId,
        )
        .map_err(ProviderError::Api)?;

        let task = bigqueryconnection_projects_locations_connections_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigqueryconnection projects locations connections delete.
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
    pub fn bigqueryconnection_projects_locations_connections_delete(
        &self,
        args: &BigqueryconnectionProjectsLocationsConnectionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigqueryconnection_projects_locations_connections_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = bigqueryconnection_projects_locations_connections_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigqueryconnection projects locations connections get iam policy.
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
    pub fn bigqueryconnection_projects_locations_connections_get_iam_policy(
        &self,
        args: &BigqueryconnectionProjectsLocationsConnectionsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigqueryconnection_projects_locations_connections_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = bigqueryconnection_projects_locations_connections_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigqueryconnection projects locations connections patch.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigqueryconnection_projects_locations_connections_patch(
        &self,
        args: &BigqueryconnectionProjectsLocationsConnectionsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Connection, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigqueryconnection_projects_locations_connections_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = bigqueryconnection_projects_locations_connections_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigqueryconnection projects locations connections set iam policy.
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
    pub fn bigqueryconnection_projects_locations_connections_set_iam_policy(
        &self,
        args: &BigqueryconnectionProjectsLocationsConnectionsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigqueryconnection_projects_locations_connections_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = bigqueryconnection_projects_locations_connections_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigqueryconnection projects locations connections test iam permissions.
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
    pub fn bigqueryconnection_projects_locations_connections_test_iam_permissions(
        &self,
        args: &BigqueryconnectionProjectsLocationsConnectionsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigqueryconnection_projects_locations_connections_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = bigqueryconnection_projects_locations_connections_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
