//! OrgpolicyProvider - State-aware orgpolicy API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       orgpolicy API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::orgpolicy::{
    orgpolicy_folders_constraints_list_builder, orgpolicy_folders_constraints_list_task,
    orgpolicy_folders_policies_create_builder, orgpolicy_folders_policies_create_task,
    orgpolicy_folders_policies_delete_builder, orgpolicy_folders_policies_delete_task,
    orgpolicy_folders_policies_get_builder, orgpolicy_folders_policies_get_task,
    orgpolicy_folders_policies_get_effective_policy_builder, orgpolicy_folders_policies_get_effective_policy_task,
    orgpolicy_folders_policies_list_builder, orgpolicy_folders_policies_list_task,
    orgpolicy_folders_policies_patch_builder, orgpolicy_folders_policies_patch_task,
    orgpolicy_organizations_constraints_list_builder, orgpolicy_organizations_constraints_list_task,
    orgpolicy_organizations_custom_constraints_create_builder, orgpolicy_organizations_custom_constraints_create_task,
    orgpolicy_organizations_custom_constraints_delete_builder, orgpolicy_organizations_custom_constraints_delete_task,
    orgpolicy_organizations_custom_constraints_get_builder, orgpolicy_organizations_custom_constraints_get_task,
    orgpolicy_organizations_custom_constraints_list_builder, orgpolicy_organizations_custom_constraints_list_task,
    orgpolicy_organizations_custom_constraints_patch_builder, orgpolicy_organizations_custom_constraints_patch_task,
    orgpolicy_organizations_policies_create_builder, orgpolicy_organizations_policies_create_task,
    orgpolicy_organizations_policies_delete_builder, orgpolicy_organizations_policies_delete_task,
    orgpolicy_organizations_policies_get_builder, orgpolicy_organizations_policies_get_task,
    orgpolicy_organizations_policies_get_effective_policy_builder, orgpolicy_organizations_policies_get_effective_policy_task,
    orgpolicy_organizations_policies_list_builder, orgpolicy_organizations_policies_list_task,
    orgpolicy_organizations_policies_patch_builder, orgpolicy_organizations_policies_patch_task,
    orgpolicy_projects_constraints_list_builder, orgpolicy_projects_constraints_list_task,
    orgpolicy_projects_policies_create_builder, orgpolicy_projects_policies_create_task,
    orgpolicy_projects_policies_delete_builder, orgpolicy_projects_policies_delete_task,
    orgpolicy_projects_policies_get_builder, orgpolicy_projects_policies_get_task,
    orgpolicy_projects_policies_get_effective_policy_builder, orgpolicy_projects_policies_get_effective_policy_task,
    orgpolicy_projects_policies_list_builder, orgpolicy_projects_policies_list_task,
    orgpolicy_projects_policies_patch_builder, orgpolicy_projects_policies_patch_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::orgpolicy::GoogleCloudOrgpolicyV2CustomConstraint;
use crate::providers::gcp::clients::orgpolicy::GoogleCloudOrgpolicyV2ListConstraintsResponse;
use crate::providers::gcp::clients::orgpolicy::GoogleCloudOrgpolicyV2ListCustomConstraintsResponse;
use crate::providers::gcp::clients::orgpolicy::GoogleCloudOrgpolicyV2ListPoliciesResponse;
use crate::providers::gcp::clients::orgpolicy::GoogleCloudOrgpolicyV2Policy;
use crate::providers::gcp::clients::orgpolicy::GoogleProtobufEmpty;
use crate::providers::gcp::clients::orgpolicy::OrgpolicyFoldersConstraintsListArgs;
use crate::providers::gcp::clients::orgpolicy::OrgpolicyFoldersPoliciesCreateArgs;
use crate::providers::gcp::clients::orgpolicy::OrgpolicyFoldersPoliciesDeleteArgs;
use crate::providers::gcp::clients::orgpolicy::OrgpolicyFoldersPoliciesGetArgs;
use crate::providers::gcp::clients::orgpolicy::OrgpolicyFoldersPoliciesGetEffectivePolicyArgs;
use crate::providers::gcp::clients::orgpolicy::OrgpolicyFoldersPoliciesListArgs;
use crate::providers::gcp::clients::orgpolicy::OrgpolicyFoldersPoliciesPatchArgs;
use crate::providers::gcp::clients::orgpolicy::OrgpolicyOrganizationsConstraintsListArgs;
use crate::providers::gcp::clients::orgpolicy::OrgpolicyOrganizationsCustomConstraintsCreateArgs;
use crate::providers::gcp::clients::orgpolicy::OrgpolicyOrganizationsCustomConstraintsDeleteArgs;
use crate::providers::gcp::clients::orgpolicy::OrgpolicyOrganizationsCustomConstraintsGetArgs;
use crate::providers::gcp::clients::orgpolicy::OrgpolicyOrganizationsCustomConstraintsListArgs;
use crate::providers::gcp::clients::orgpolicy::OrgpolicyOrganizationsCustomConstraintsPatchArgs;
use crate::providers::gcp::clients::orgpolicy::OrgpolicyOrganizationsPoliciesCreateArgs;
use crate::providers::gcp::clients::orgpolicy::OrgpolicyOrganizationsPoliciesDeleteArgs;
use crate::providers::gcp::clients::orgpolicy::OrgpolicyOrganizationsPoliciesGetArgs;
use crate::providers::gcp::clients::orgpolicy::OrgpolicyOrganizationsPoliciesGetEffectivePolicyArgs;
use crate::providers::gcp::clients::orgpolicy::OrgpolicyOrganizationsPoliciesListArgs;
use crate::providers::gcp::clients::orgpolicy::OrgpolicyOrganizationsPoliciesPatchArgs;
use crate::providers::gcp::clients::orgpolicy::OrgpolicyProjectsConstraintsListArgs;
use crate::providers::gcp::clients::orgpolicy::OrgpolicyProjectsPoliciesCreateArgs;
use crate::providers::gcp::clients::orgpolicy::OrgpolicyProjectsPoliciesDeleteArgs;
use crate::providers::gcp::clients::orgpolicy::OrgpolicyProjectsPoliciesGetArgs;
use crate::providers::gcp::clients::orgpolicy::OrgpolicyProjectsPoliciesGetEffectivePolicyArgs;
use crate::providers::gcp::clients::orgpolicy::OrgpolicyProjectsPoliciesListArgs;
use crate::providers::gcp::clients::orgpolicy::OrgpolicyProjectsPoliciesPatchArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// OrgpolicyProvider with automatic state tracking.
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
/// let provider = OrgpolicyProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct OrgpolicyProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> OrgpolicyProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new OrgpolicyProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Orgpolicy folders constraints list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudOrgpolicyV2ListConstraintsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn orgpolicy_folders_constraints_list(
        &self,
        args: &OrgpolicyFoldersConstraintsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudOrgpolicyV2ListConstraintsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = orgpolicy_folders_constraints_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = orgpolicy_folders_constraints_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Orgpolicy folders policies create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudOrgpolicyV2Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn orgpolicy_folders_policies_create(
        &self,
        args: &OrgpolicyFoldersPoliciesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudOrgpolicyV2Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = orgpolicy_folders_policies_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = orgpolicy_folders_policies_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Orgpolicy folders policies delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn orgpolicy_folders_policies_delete(
        &self,
        args: &OrgpolicyFoldersPoliciesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = orgpolicy_folders_policies_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
        )
        .map_err(ProviderError::Api)?;

        let task = orgpolicy_folders_policies_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Orgpolicy folders policies get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudOrgpolicyV2Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn orgpolicy_folders_policies_get(
        &self,
        args: &OrgpolicyFoldersPoliciesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudOrgpolicyV2Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = orgpolicy_folders_policies_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = orgpolicy_folders_policies_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Orgpolicy folders policies get effective policy.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudOrgpolicyV2Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn orgpolicy_folders_policies_get_effective_policy(
        &self,
        args: &OrgpolicyFoldersPoliciesGetEffectivePolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudOrgpolicyV2Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = orgpolicy_folders_policies_get_effective_policy_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = orgpolicy_folders_policies_get_effective_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Orgpolicy folders policies list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudOrgpolicyV2ListPoliciesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn orgpolicy_folders_policies_list(
        &self,
        args: &OrgpolicyFoldersPoliciesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudOrgpolicyV2ListPoliciesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = orgpolicy_folders_policies_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = orgpolicy_folders_policies_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Orgpolicy folders policies patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudOrgpolicyV2Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn orgpolicy_folders_policies_patch(
        &self,
        args: &OrgpolicyFoldersPoliciesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudOrgpolicyV2Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = orgpolicy_folders_policies_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = orgpolicy_folders_policies_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Orgpolicy organizations constraints list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudOrgpolicyV2ListConstraintsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn orgpolicy_organizations_constraints_list(
        &self,
        args: &OrgpolicyOrganizationsConstraintsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudOrgpolicyV2ListConstraintsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = orgpolicy_organizations_constraints_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = orgpolicy_organizations_constraints_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Orgpolicy organizations custom constraints create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudOrgpolicyV2CustomConstraint result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn orgpolicy_organizations_custom_constraints_create(
        &self,
        args: &OrgpolicyOrganizationsCustomConstraintsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudOrgpolicyV2CustomConstraint, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = orgpolicy_organizations_custom_constraints_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = orgpolicy_organizations_custom_constraints_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Orgpolicy organizations custom constraints delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn orgpolicy_organizations_custom_constraints_delete(
        &self,
        args: &OrgpolicyOrganizationsCustomConstraintsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = orgpolicy_organizations_custom_constraints_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = orgpolicy_organizations_custom_constraints_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Orgpolicy organizations custom constraints get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudOrgpolicyV2CustomConstraint result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn orgpolicy_organizations_custom_constraints_get(
        &self,
        args: &OrgpolicyOrganizationsCustomConstraintsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudOrgpolicyV2CustomConstraint, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = orgpolicy_organizations_custom_constraints_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = orgpolicy_organizations_custom_constraints_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Orgpolicy organizations custom constraints list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudOrgpolicyV2ListCustomConstraintsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn orgpolicy_organizations_custom_constraints_list(
        &self,
        args: &OrgpolicyOrganizationsCustomConstraintsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudOrgpolicyV2ListCustomConstraintsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = orgpolicy_organizations_custom_constraints_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = orgpolicy_organizations_custom_constraints_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Orgpolicy organizations custom constraints patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudOrgpolicyV2CustomConstraint result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn orgpolicy_organizations_custom_constraints_patch(
        &self,
        args: &OrgpolicyOrganizationsCustomConstraintsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudOrgpolicyV2CustomConstraint, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = orgpolicy_organizations_custom_constraints_patch_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = orgpolicy_organizations_custom_constraints_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Orgpolicy organizations policies create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudOrgpolicyV2Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn orgpolicy_organizations_policies_create(
        &self,
        args: &OrgpolicyOrganizationsPoliciesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudOrgpolicyV2Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = orgpolicy_organizations_policies_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = orgpolicy_organizations_policies_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Orgpolicy organizations policies delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn orgpolicy_organizations_policies_delete(
        &self,
        args: &OrgpolicyOrganizationsPoliciesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = orgpolicy_organizations_policies_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
        )
        .map_err(ProviderError::Api)?;

        let task = orgpolicy_organizations_policies_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Orgpolicy organizations policies get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudOrgpolicyV2Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn orgpolicy_organizations_policies_get(
        &self,
        args: &OrgpolicyOrganizationsPoliciesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudOrgpolicyV2Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = orgpolicy_organizations_policies_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = orgpolicy_organizations_policies_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Orgpolicy organizations policies get effective policy.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudOrgpolicyV2Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn orgpolicy_organizations_policies_get_effective_policy(
        &self,
        args: &OrgpolicyOrganizationsPoliciesGetEffectivePolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudOrgpolicyV2Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = orgpolicy_organizations_policies_get_effective_policy_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = orgpolicy_organizations_policies_get_effective_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Orgpolicy organizations policies list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudOrgpolicyV2ListPoliciesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn orgpolicy_organizations_policies_list(
        &self,
        args: &OrgpolicyOrganizationsPoliciesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudOrgpolicyV2ListPoliciesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = orgpolicy_organizations_policies_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = orgpolicy_organizations_policies_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Orgpolicy organizations policies patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudOrgpolicyV2Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn orgpolicy_organizations_policies_patch(
        &self,
        args: &OrgpolicyOrganizationsPoliciesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudOrgpolicyV2Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = orgpolicy_organizations_policies_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = orgpolicy_organizations_policies_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Orgpolicy projects constraints list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudOrgpolicyV2ListConstraintsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn orgpolicy_projects_constraints_list(
        &self,
        args: &OrgpolicyProjectsConstraintsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudOrgpolicyV2ListConstraintsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = orgpolicy_projects_constraints_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = orgpolicy_projects_constraints_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Orgpolicy projects policies create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudOrgpolicyV2Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn orgpolicy_projects_policies_create(
        &self,
        args: &OrgpolicyProjectsPoliciesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudOrgpolicyV2Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = orgpolicy_projects_policies_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = orgpolicy_projects_policies_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Orgpolicy projects policies delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn orgpolicy_projects_policies_delete(
        &self,
        args: &OrgpolicyProjectsPoliciesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = orgpolicy_projects_policies_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
        )
        .map_err(ProviderError::Api)?;

        let task = orgpolicy_projects_policies_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Orgpolicy projects policies get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudOrgpolicyV2Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn orgpolicy_projects_policies_get(
        &self,
        args: &OrgpolicyProjectsPoliciesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudOrgpolicyV2Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = orgpolicy_projects_policies_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = orgpolicy_projects_policies_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Orgpolicy projects policies get effective policy.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudOrgpolicyV2Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn orgpolicy_projects_policies_get_effective_policy(
        &self,
        args: &OrgpolicyProjectsPoliciesGetEffectivePolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudOrgpolicyV2Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = orgpolicy_projects_policies_get_effective_policy_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = orgpolicy_projects_policies_get_effective_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Orgpolicy projects policies list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudOrgpolicyV2ListPoliciesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn orgpolicy_projects_policies_list(
        &self,
        args: &OrgpolicyProjectsPoliciesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudOrgpolicyV2ListPoliciesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = orgpolicy_projects_policies_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = orgpolicy_projects_policies_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Orgpolicy projects policies patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudOrgpolicyV2Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn orgpolicy_projects_policies_patch(
        &self,
        args: &OrgpolicyProjectsPoliciesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudOrgpolicyV2Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = orgpolicy_projects_policies_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = orgpolicy_projects_policies_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
