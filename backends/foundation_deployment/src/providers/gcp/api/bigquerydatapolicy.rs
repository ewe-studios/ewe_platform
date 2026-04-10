//! BigquerydatapolicyProvider - State-aware bigquerydatapolicy API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       bigquerydatapolicy API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::bigquerydatapolicy::{
    bigquerydatapolicy_projects_locations_data_policies_add_grantees_builder, bigquerydatapolicy_projects_locations_data_policies_add_grantees_task,
    bigquerydatapolicy_projects_locations_data_policies_create_builder, bigquerydatapolicy_projects_locations_data_policies_create_task,
    bigquerydatapolicy_projects_locations_data_policies_delete_builder, bigquerydatapolicy_projects_locations_data_policies_delete_task,
    bigquerydatapolicy_projects_locations_data_policies_get_iam_policy_builder, bigquerydatapolicy_projects_locations_data_policies_get_iam_policy_task,
    bigquerydatapolicy_projects_locations_data_policies_patch_builder, bigquerydatapolicy_projects_locations_data_policies_patch_task,
    bigquerydatapolicy_projects_locations_data_policies_remove_grantees_builder, bigquerydatapolicy_projects_locations_data_policies_remove_grantees_task,
    bigquerydatapolicy_projects_locations_data_policies_set_iam_policy_builder, bigquerydatapolicy_projects_locations_data_policies_set_iam_policy_task,
    bigquerydatapolicy_projects_locations_data_policies_test_iam_permissions_builder, bigquerydatapolicy_projects_locations_data_policies_test_iam_permissions_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::bigquerydatapolicy::DataPolicy;
use crate::providers::gcp::clients::bigquerydatapolicy::Empty;
use crate::providers::gcp::clients::bigquerydatapolicy::Policy;
use crate::providers::gcp::clients::bigquerydatapolicy::TestIamPermissionsResponse;
use crate::providers::gcp::clients::bigquerydatapolicy::BigquerydatapolicyProjectsLocationsDataPoliciesAddGranteesArgs;
use crate::providers::gcp::clients::bigquerydatapolicy::BigquerydatapolicyProjectsLocationsDataPoliciesCreateArgs;
use crate::providers::gcp::clients::bigquerydatapolicy::BigquerydatapolicyProjectsLocationsDataPoliciesDeleteArgs;
use crate::providers::gcp::clients::bigquerydatapolicy::BigquerydatapolicyProjectsLocationsDataPoliciesGetIamPolicyArgs;
use crate::providers::gcp::clients::bigquerydatapolicy::BigquerydatapolicyProjectsLocationsDataPoliciesPatchArgs;
use crate::providers::gcp::clients::bigquerydatapolicy::BigquerydatapolicyProjectsLocationsDataPoliciesRemoveGranteesArgs;
use crate::providers::gcp::clients::bigquerydatapolicy::BigquerydatapolicyProjectsLocationsDataPoliciesSetIamPolicyArgs;
use crate::providers::gcp::clients::bigquerydatapolicy::BigquerydatapolicyProjectsLocationsDataPoliciesTestIamPermissionsArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// BigquerydatapolicyProvider with automatic state tracking.
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
/// let provider = BigquerydatapolicyProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct BigquerydatapolicyProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> BigquerydatapolicyProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new BigquerydatapolicyProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Bigquerydatapolicy projects locations data policies add grantees.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DataPolicy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigquerydatapolicy_projects_locations_data_policies_add_grantees(
        &self,
        args: &BigquerydatapolicyProjectsLocationsDataPoliciesAddGranteesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DataPolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquerydatapolicy_projects_locations_data_policies_add_grantees_builder(
            &self.http_client,
            &args.dataPolicy,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquerydatapolicy_projects_locations_data_policies_add_grantees_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquerydatapolicy projects locations data policies create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DataPolicy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigquerydatapolicy_projects_locations_data_policies_create(
        &self,
        args: &BigquerydatapolicyProjectsLocationsDataPoliciesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DataPolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquerydatapolicy_projects_locations_data_policies_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquerydatapolicy_projects_locations_data_policies_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquerydatapolicy projects locations data policies delete.
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
    pub fn bigquerydatapolicy_projects_locations_data_policies_delete(
        &self,
        args: &BigquerydatapolicyProjectsLocationsDataPoliciesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquerydatapolicy_projects_locations_data_policies_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquerydatapolicy_projects_locations_data_policies_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquerydatapolicy projects locations data policies get iam policy.
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
    pub fn bigquerydatapolicy_projects_locations_data_policies_get_iam_policy(
        &self,
        args: &BigquerydatapolicyProjectsLocationsDataPoliciesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquerydatapolicy_projects_locations_data_policies_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquerydatapolicy_projects_locations_data_policies_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquerydatapolicy projects locations data policies patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DataPolicy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigquerydatapolicy_projects_locations_data_policies_patch(
        &self,
        args: &BigquerydatapolicyProjectsLocationsDataPoliciesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DataPolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquerydatapolicy_projects_locations_data_policies_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquerydatapolicy_projects_locations_data_policies_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquerydatapolicy projects locations data policies remove grantees.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DataPolicy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigquerydatapolicy_projects_locations_data_policies_remove_grantees(
        &self,
        args: &BigquerydatapolicyProjectsLocationsDataPoliciesRemoveGranteesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DataPolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquerydatapolicy_projects_locations_data_policies_remove_grantees_builder(
            &self.http_client,
            &args.dataPolicy,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquerydatapolicy_projects_locations_data_policies_remove_grantees_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquerydatapolicy projects locations data policies set iam policy.
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
    pub fn bigquerydatapolicy_projects_locations_data_policies_set_iam_policy(
        &self,
        args: &BigquerydatapolicyProjectsLocationsDataPoliciesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquerydatapolicy_projects_locations_data_policies_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquerydatapolicy_projects_locations_data_policies_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquerydatapolicy projects locations data policies test iam permissions.
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
    pub fn bigquerydatapolicy_projects_locations_data_policies_test_iam_permissions(
        &self,
        args: &BigquerydatapolicyProjectsLocationsDataPoliciesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquerydatapolicy_projects_locations_data_policies_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquerydatapolicy_projects_locations_data_policies_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
