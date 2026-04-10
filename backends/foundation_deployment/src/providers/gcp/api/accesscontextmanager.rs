//! AccesscontextmanagerProvider - State-aware accesscontextmanager API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       accesscontextmanager API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::accesscontextmanager::{
    accesscontextmanager_access_policies_create_builder, accesscontextmanager_access_policies_create_task,
    accesscontextmanager_access_policies_delete_builder, accesscontextmanager_access_policies_delete_task,
    accesscontextmanager_access_policies_get_iam_policy_builder, accesscontextmanager_access_policies_get_iam_policy_task,
    accesscontextmanager_access_policies_patch_builder, accesscontextmanager_access_policies_patch_task,
    accesscontextmanager_access_policies_set_iam_policy_builder, accesscontextmanager_access_policies_set_iam_policy_task,
    accesscontextmanager_access_policies_test_iam_permissions_builder, accesscontextmanager_access_policies_test_iam_permissions_task,
    accesscontextmanager_access_policies_access_levels_create_builder, accesscontextmanager_access_policies_access_levels_create_task,
    accesscontextmanager_access_policies_access_levels_delete_builder, accesscontextmanager_access_policies_access_levels_delete_task,
    accesscontextmanager_access_policies_access_levels_patch_builder, accesscontextmanager_access_policies_access_levels_patch_task,
    accesscontextmanager_access_policies_access_levels_replace_all_builder, accesscontextmanager_access_policies_access_levels_replace_all_task,
    accesscontextmanager_access_policies_access_levels_test_iam_permissions_builder, accesscontextmanager_access_policies_access_levels_test_iam_permissions_task,
    accesscontextmanager_access_policies_authorized_orgs_descs_create_builder, accesscontextmanager_access_policies_authorized_orgs_descs_create_task,
    accesscontextmanager_access_policies_authorized_orgs_descs_delete_builder, accesscontextmanager_access_policies_authorized_orgs_descs_delete_task,
    accesscontextmanager_access_policies_authorized_orgs_descs_patch_builder, accesscontextmanager_access_policies_authorized_orgs_descs_patch_task,
    accesscontextmanager_access_policies_service_perimeters_commit_builder, accesscontextmanager_access_policies_service_perimeters_commit_task,
    accesscontextmanager_access_policies_service_perimeters_create_builder, accesscontextmanager_access_policies_service_perimeters_create_task,
    accesscontextmanager_access_policies_service_perimeters_delete_builder, accesscontextmanager_access_policies_service_perimeters_delete_task,
    accesscontextmanager_access_policies_service_perimeters_patch_builder, accesscontextmanager_access_policies_service_perimeters_patch_task,
    accesscontextmanager_access_policies_service_perimeters_replace_all_builder, accesscontextmanager_access_policies_service_perimeters_replace_all_task,
    accesscontextmanager_access_policies_service_perimeters_test_iam_permissions_builder, accesscontextmanager_access_policies_service_perimeters_test_iam_permissions_task,
    accesscontextmanager_operations_cancel_builder, accesscontextmanager_operations_cancel_task,
    accesscontextmanager_operations_delete_builder, accesscontextmanager_operations_delete_task,
    accesscontextmanager_organizations_gcp_user_access_bindings_create_builder, accesscontextmanager_organizations_gcp_user_access_bindings_create_task,
    accesscontextmanager_organizations_gcp_user_access_bindings_delete_builder, accesscontextmanager_organizations_gcp_user_access_bindings_delete_task,
    accesscontextmanager_organizations_gcp_user_access_bindings_patch_builder, accesscontextmanager_organizations_gcp_user_access_bindings_patch_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::accesscontextmanager::Empty;
use crate::providers::gcp::clients::accesscontextmanager::Operation;
use crate::providers::gcp::clients::accesscontextmanager::Policy;
use crate::providers::gcp::clients::accesscontextmanager::TestIamPermissionsResponse;
use crate::providers::gcp::clients::accesscontextmanager::AccesscontextmanagerAccessPoliciesAccessLevelsCreateArgs;
use crate::providers::gcp::clients::accesscontextmanager::AccesscontextmanagerAccessPoliciesAccessLevelsDeleteArgs;
use crate::providers::gcp::clients::accesscontextmanager::AccesscontextmanagerAccessPoliciesAccessLevelsPatchArgs;
use crate::providers::gcp::clients::accesscontextmanager::AccesscontextmanagerAccessPoliciesAccessLevelsReplaceAllArgs;
use crate::providers::gcp::clients::accesscontextmanager::AccesscontextmanagerAccessPoliciesAccessLevelsTestIamPermissionsArgs;
use crate::providers::gcp::clients::accesscontextmanager::AccesscontextmanagerAccessPoliciesAuthorizedOrgsDescsCreateArgs;
use crate::providers::gcp::clients::accesscontextmanager::AccesscontextmanagerAccessPoliciesAuthorizedOrgsDescsDeleteArgs;
use crate::providers::gcp::clients::accesscontextmanager::AccesscontextmanagerAccessPoliciesAuthorizedOrgsDescsPatchArgs;
use crate::providers::gcp::clients::accesscontextmanager::AccesscontextmanagerAccessPoliciesCreateArgs;
use crate::providers::gcp::clients::accesscontextmanager::AccesscontextmanagerAccessPoliciesDeleteArgs;
use crate::providers::gcp::clients::accesscontextmanager::AccesscontextmanagerAccessPoliciesGetIamPolicyArgs;
use crate::providers::gcp::clients::accesscontextmanager::AccesscontextmanagerAccessPoliciesPatchArgs;
use crate::providers::gcp::clients::accesscontextmanager::AccesscontextmanagerAccessPoliciesServicePerimetersCommitArgs;
use crate::providers::gcp::clients::accesscontextmanager::AccesscontextmanagerAccessPoliciesServicePerimetersCreateArgs;
use crate::providers::gcp::clients::accesscontextmanager::AccesscontextmanagerAccessPoliciesServicePerimetersDeleteArgs;
use crate::providers::gcp::clients::accesscontextmanager::AccesscontextmanagerAccessPoliciesServicePerimetersPatchArgs;
use crate::providers::gcp::clients::accesscontextmanager::AccesscontextmanagerAccessPoliciesServicePerimetersReplaceAllArgs;
use crate::providers::gcp::clients::accesscontextmanager::AccesscontextmanagerAccessPoliciesServicePerimetersTestIamPermissionsArgs;
use crate::providers::gcp::clients::accesscontextmanager::AccesscontextmanagerAccessPoliciesSetIamPolicyArgs;
use crate::providers::gcp::clients::accesscontextmanager::AccesscontextmanagerAccessPoliciesTestIamPermissionsArgs;
use crate::providers::gcp::clients::accesscontextmanager::AccesscontextmanagerOperationsCancelArgs;
use crate::providers::gcp::clients::accesscontextmanager::AccesscontextmanagerOperationsDeleteArgs;
use crate::providers::gcp::clients::accesscontextmanager::AccesscontextmanagerOrganizationsGcpUserAccessBindingsCreateArgs;
use crate::providers::gcp::clients::accesscontextmanager::AccesscontextmanagerOrganizationsGcpUserAccessBindingsDeleteArgs;
use crate::providers::gcp::clients::accesscontextmanager::AccesscontextmanagerOrganizationsGcpUserAccessBindingsPatchArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// AccesscontextmanagerProvider with automatic state tracking.
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
/// let provider = AccesscontextmanagerProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct AccesscontextmanagerProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> AccesscontextmanagerProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new AccesscontextmanagerProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Accesscontextmanager access policies create.
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
    pub fn accesscontextmanager_access_policies_create(
        &self,
        args: &AccesscontextmanagerAccessPoliciesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accesscontextmanager_access_policies_create_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = accesscontextmanager_access_policies_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accesscontextmanager access policies delete.
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
    pub fn accesscontextmanager_access_policies_delete(
        &self,
        args: &AccesscontextmanagerAccessPoliciesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accesscontextmanager_access_policies_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = accesscontextmanager_access_policies_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accesscontextmanager access policies get iam policy.
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
    pub fn accesscontextmanager_access_policies_get_iam_policy(
        &self,
        args: &AccesscontextmanagerAccessPoliciesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accesscontextmanager_access_policies_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = accesscontextmanager_access_policies_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accesscontextmanager access policies patch.
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
    pub fn accesscontextmanager_access_policies_patch(
        &self,
        args: &AccesscontextmanagerAccessPoliciesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accesscontextmanager_access_policies_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = accesscontextmanager_access_policies_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accesscontextmanager access policies set iam policy.
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
    pub fn accesscontextmanager_access_policies_set_iam_policy(
        &self,
        args: &AccesscontextmanagerAccessPoliciesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accesscontextmanager_access_policies_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = accesscontextmanager_access_policies_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accesscontextmanager access policies test iam permissions.
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
    pub fn accesscontextmanager_access_policies_test_iam_permissions(
        &self,
        args: &AccesscontextmanagerAccessPoliciesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accesscontextmanager_access_policies_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = accesscontextmanager_access_policies_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accesscontextmanager access policies access levels create.
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
    pub fn accesscontextmanager_access_policies_access_levels_create(
        &self,
        args: &AccesscontextmanagerAccessPoliciesAccessLevelsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accesscontextmanager_access_policies_access_levels_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = accesscontextmanager_access_policies_access_levels_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accesscontextmanager access policies access levels delete.
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
    pub fn accesscontextmanager_access_policies_access_levels_delete(
        &self,
        args: &AccesscontextmanagerAccessPoliciesAccessLevelsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accesscontextmanager_access_policies_access_levels_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = accesscontextmanager_access_policies_access_levels_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accesscontextmanager access policies access levels patch.
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
    pub fn accesscontextmanager_access_policies_access_levels_patch(
        &self,
        args: &AccesscontextmanagerAccessPoliciesAccessLevelsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accesscontextmanager_access_policies_access_levels_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = accesscontextmanager_access_policies_access_levels_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accesscontextmanager access policies access levels replace all.
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
    pub fn accesscontextmanager_access_policies_access_levels_replace_all(
        &self,
        args: &AccesscontextmanagerAccessPoliciesAccessLevelsReplaceAllArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accesscontextmanager_access_policies_access_levels_replace_all_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = accesscontextmanager_access_policies_access_levels_replace_all_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accesscontextmanager access policies access levels test iam permissions.
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
    pub fn accesscontextmanager_access_policies_access_levels_test_iam_permissions(
        &self,
        args: &AccesscontextmanagerAccessPoliciesAccessLevelsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accesscontextmanager_access_policies_access_levels_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = accesscontextmanager_access_policies_access_levels_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accesscontextmanager access policies authorized orgs descs create.
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
    pub fn accesscontextmanager_access_policies_authorized_orgs_descs_create(
        &self,
        args: &AccesscontextmanagerAccessPoliciesAuthorizedOrgsDescsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accesscontextmanager_access_policies_authorized_orgs_descs_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = accesscontextmanager_access_policies_authorized_orgs_descs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accesscontextmanager access policies authorized orgs descs delete.
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
    pub fn accesscontextmanager_access_policies_authorized_orgs_descs_delete(
        &self,
        args: &AccesscontextmanagerAccessPoliciesAuthorizedOrgsDescsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accesscontextmanager_access_policies_authorized_orgs_descs_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = accesscontextmanager_access_policies_authorized_orgs_descs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accesscontextmanager access policies authorized orgs descs patch.
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
    pub fn accesscontextmanager_access_policies_authorized_orgs_descs_patch(
        &self,
        args: &AccesscontextmanagerAccessPoliciesAuthorizedOrgsDescsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accesscontextmanager_access_policies_authorized_orgs_descs_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = accesscontextmanager_access_policies_authorized_orgs_descs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accesscontextmanager access policies service perimeters commit.
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
    pub fn accesscontextmanager_access_policies_service_perimeters_commit(
        &self,
        args: &AccesscontextmanagerAccessPoliciesServicePerimetersCommitArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accesscontextmanager_access_policies_service_perimeters_commit_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = accesscontextmanager_access_policies_service_perimeters_commit_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accesscontextmanager access policies service perimeters create.
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
    pub fn accesscontextmanager_access_policies_service_perimeters_create(
        &self,
        args: &AccesscontextmanagerAccessPoliciesServicePerimetersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accesscontextmanager_access_policies_service_perimeters_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = accesscontextmanager_access_policies_service_perimeters_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accesscontextmanager access policies service perimeters delete.
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
    pub fn accesscontextmanager_access_policies_service_perimeters_delete(
        &self,
        args: &AccesscontextmanagerAccessPoliciesServicePerimetersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accesscontextmanager_access_policies_service_perimeters_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = accesscontextmanager_access_policies_service_perimeters_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accesscontextmanager access policies service perimeters patch.
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
    pub fn accesscontextmanager_access_policies_service_perimeters_patch(
        &self,
        args: &AccesscontextmanagerAccessPoliciesServicePerimetersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accesscontextmanager_access_policies_service_perimeters_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = accesscontextmanager_access_policies_service_perimeters_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accesscontextmanager access policies service perimeters replace all.
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
    pub fn accesscontextmanager_access_policies_service_perimeters_replace_all(
        &self,
        args: &AccesscontextmanagerAccessPoliciesServicePerimetersReplaceAllArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accesscontextmanager_access_policies_service_perimeters_replace_all_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = accesscontextmanager_access_policies_service_perimeters_replace_all_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accesscontextmanager access policies service perimeters test iam permissions.
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
    pub fn accesscontextmanager_access_policies_service_perimeters_test_iam_permissions(
        &self,
        args: &AccesscontextmanagerAccessPoliciesServicePerimetersTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accesscontextmanager_access_policies_service_perimeters_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = accesscontextmanager_access_policies_service_perimeters_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accesscontextmanager operations cancel.
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
    pub fn accesscontextmanager_operations_cancel(
        &self,
        args: &AccesscontextmanagerOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accesscontextmanager_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = accesscontextmanager_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accesscontextmanager operations delete.
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
    pub fn accesscontextmanager_operations_delete(
        &self,
        args: &AccesscontextmanagerOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accesscontextmanager_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = accesscontextmanager_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accesscontextmanager organizations gcp user access bindings create.
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
    pub fn accesscontextmanager_organizations_gcp_user_access_bindings_create(
        &self,
        args: &AccesscontextmanagerOrganizationsGcpUserAccessBindingsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accesscontextmanager_organizations_gcp_user_access_bindings_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = accesscontextmanager_organizations_gcp_user_access_bindings_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accesscontextmanager organizations gcp user access bindings delete.
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
    pub fn accesscontextmanager_organizations_gcp_user_access_bindings_delete(
        &self,
        args: &AccesscontextmanagerOrganizationsGcpUserAccessBindingsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accesscontextmanager_organizations_gcp_user_access_bindings_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = accesscontextmanager_organizations_gcp_user_access_bindings_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accesscontextmanager organizations gcp user access bindings patch.
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
    pub fn accesscontextmanager_organizations_gcp_user_access_bindings_patch(
        &self,
        args: &AccesscontextmanagerOrganizationsGcpUserAccessBindingsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accesscontextmanager_organizations_gcp_user_access_bindings_patch_builder(
            &self.http_client,
            &args.name,
            &args.append,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = accesscontextmanager_organizations_gcp_user_access_bindings_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
