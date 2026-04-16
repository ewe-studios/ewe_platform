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
    bigquerydatapolicy_projects_locations_data_policies_get_builder, bigquerydatapolicy_projects_locations_data_policies_get_task,
    bigquerydatapolicy_projects_locations_data_policies_get_iam_policy_builder, bigquerydatapolicy_projects_locations_data_policies_get_iam_policy_task,
    bigquerydatapolicy_projects_locations_data_policies_list_builder, bigquerydatapolicy_projects_locations_data_policies_list_task,
    bigquerydatapolicy_projects_locations_data_policies_patch_builder, bigquerydatapolicy_projects_locations_data_policies_patch_task,
    bigquerydatapolicy_projects_locations_data_policies_remove_grantees_builder, bigquerydatapolicy_projects_locations_data_policies_remove_grantees_task,
    bigquerydatapolicy_projects_locations_data_policies_set_iam_policy_builder, bigquerydatapolicy_projects_locations_data_policies_set_iam_policy_task,
    bigquerydatapolicy_projects_locations_data_policies_test_iam_permissions_builder, bigquerydatapolicy_projects_locations_data_policies_test_iam_permissions_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::bigquerydatapolicy::DataPolicy;
use crate::providers::gcp::clients::bigquerydatapolicy::Empty;
use crate::providers::gcp::clients::bigquerydatapolicy::ListDataPoliciesResponse;
use crate::providers::gcp::clients::bigquerydatapolicy::Policy;
use crate::providers::gcp::clients::bigquerydatapolicy::TestIamPermissionsResponse;
use crate::providers::gcp::clients::bigquerydatapolicy::BigquerydatapolicyProjectsLocationsDataPoliciesAddGranteesArgs;
use crate::providers::gcp::clients::bigquerydatapolicy::BigquerydatapolicyProjectsLocationsDataPoliciesCreateArgs;
use crate::providers::gcp::clients::bigquerydatapolicy::BigquerydatapolicyProjectsLocationsDataPoliciesDeleteArgs;
use crate::providers::gcp::clients::bigquerydatapolicy::BigquerydatapolicyProjectsLocationsDataPoliciesGetArgs;
use crate::providers::gcp::clients::bigquerydatapolicy::BigquerydatapolicyProjectsLocationsDataPoliciesGetIamPolicyArgs;
use crate::providers::gcp::clients::bigquerydatapolicy::BigquerydatapolicyProjectsLocationsDataPoliciesListArgs;
use crate::providers::gcp::clients::bigquerydatapolicy::BigquerydatapolicyProjectsLocationsDataPoliciesPatchArgs;
use crate::providers::gcp::clients::bigquerydatapolicy::BigquerydatapolicyProjectsLocationsDataPoliciesRemoveGranteesArgs;
use crate::providers::gcp::clients::bigquerydatapolicy::BigquerydatapolicyProjectsLocationsDataPoliciesSetIamPolicyArgs;
use crate::providers::gcp::clients::bigquerydatapolicy::BigquerydatapolicyProjectsLocationsDataPoliciesTestIamPermissionsArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// BigquerydatapolicyProvider with automatic state tracking.
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
/// let provider = BigquerydatapolicyProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct BigquerydatapolicyProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> BigquerydatapolicyProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new BigquerydatapolicyProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new BigquerydatapolicyProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
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
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquerydatapolicy projects locations data policies get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn bigquerydatapolicy_projects_locations_data_policies_get(
        &self,
        args: &BigquerydatapolicyProjectsLocationsDataPoliciesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DataPolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquerydatapolicy_projects_locations_data_policies_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquerydatapolicy_projects_locations_data_policies_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquerydatapolicy projects locations data policies get iam policy.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquerydatapolicy projects locations data policies list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDataPoliciesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigquerydatapolicy_projects_locations_data_policies_list(
        &self,
        args: &BigquerydatapolicyProjectsLocationsDataPoliciesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDataPoliciesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquerydatapolicy_projects_locations_data_policies_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquerydatapolicy_projects_locations_data_policies_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquerydatapolicy projects locations data policies patch.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquerydatapolicy projects locations data policies remove grantees.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquerydatapolicy projects locations data policies set iam policy.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquerydatapolicy projects locations data policies test iam permissions.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
