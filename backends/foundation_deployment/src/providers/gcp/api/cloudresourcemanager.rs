//! CloudresourcemanagerProvider - State-aware cloudresourcemanager API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       cloudresourcemanager API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::cloudresourcemanager::{
    cloudresourcemanager_effective_tags_list_builder, cloudresourcemanager_effective_tags_list_task,
    cloudresourcemanager_folders_create_builder, cloudresourcemanager_folders_create_task,
    cloudresourcemanager_folders_delete_builder, cloudresourcemanager_folders_delete_task,
    cloudresourcemanager_folders_get_builder, cloudresourcemanager_folders_get_task,
    cloudresourcemanager_folders_get_iam_policy_builder, cloudresourcemanager_folders_get_iam_policy_task,
    cloudresourcemanager_folders_list_builder, cloudresourcemanager_folders_list_task,
    cloudresourcemanager_folders_move_builder, cloudresourcemanager_folders_move_task,
    cloudresourcemanager_folders_patch_builder, cloudresourcemanager_folders_patch_task,
    cloudresourcemanager_folders_search_builder, cloudresourcemanager_folders_search_task,
    cloudresourcemanager_folders_set_iam_policy_builder, cloudresourcemanager_folders_set_iam_policy_task,
    cloudresourcemanager_folders_test_iam_permissions_builder, cloudresourcemanager_folders_test_iam_permissions_task,
    cloudresourcemanager_folders_undelete_builder, cloudresourcemanager_folders_undelete_task,
    cloudresourcemanager_folders_capabilities_get_builder, cloudresourcemanager_folders_capabilities_get_task,
    cloudresourcemanager_folders_capabilities_patch_builder, cloudresourcemanager_folders_capabilities_patch_task,
    cloudresourcemanager_liens_create_builder, cloudresourcemanager_liens_create_task,
    cloudresourcemanager_liens_delete_builder, cloudresourcemanager_liens_delete_task,
    cloudresourcemanager_liens_get_builder, cloudresourcemanager_liens_get_task,
    cloudresourcemanager_liens_list_builder, cloudresourcemanager_liens_list_task,
    cloudresourcemanager_locations_effective_tag_binding_collections_get_builder, cloudresourcemanager_locations_effective_tag_binding_collections_get_task,
    cloudresourcemanager_locations_tag_binding_collections_get_builder, cloudresourcemanager_locations_tag_binding_collections_get_task,
    cloudresourcemanager_locations_tag_binding_collections_patch_builder, cloudresourcemanager_locations_tag_binding_collections_patch_task,
    cloudresourcemanager_operations_get_builder, cloudresourcemanager_operations_get_task,
    cloudresourcemanager_organizations_get_builder, cloudresourcemanager_organizations_get_task,
    cloudresourcemanager_organizations_get_iam_policy_builder, cloudresourcemanager_organizations_get_iam_policy_task,
    cloudresourcemanager_organizations_search_builder, cloudresourcemanager_organizations_search_task,
    cloudresourcemanager_organizations_set_iam_policy_builder, cloudresourcemanager_organizations_set_iam_policy_task,
    cloudresourcemanager_organizations_test_iam_permissions_builder, cloudresourcemanager_organizations_test_iam_permissions_task,
    cloudresourcemanager_projects_create_builder, cloudresourcemanager_projects_create_task,
    cloudresourcemanager_projects_delete_builder, cloudresourcemanager_projects_delete_task,
    cloudresourcemanager_projects_get_builder, cloudresourcemanager_projects_get_task,
    cloudresourcemanager_projects_get_iam_policy_builder, cloudresourcemanager_projects_get_iam_policy_task,
    cloudresourcemanager_projects_list_builder, cloudresourcemanager_projects_list_task,
    cloudresourcemanager_projects_move_builder, cloudresourcemanager_projects_move_task,
    cloudresourcemanager_projects_patch_builder, cloudresourcemanager_projects_patch_task,
    cloudresourcemanager_projects_search_builder, cloudresourcemanager_projects_search_task,
    cloudresourcemanager_projects_set_iam_policy_builder, cloudresourcemanager_projects_set_iam_policy_task,
    cloudresourcemanager_projects_test_iam_permissions_builder, cloudresourcemanager_projects_test_iam_permissions_task,
    cloudresourcemanager_projects_undelete_builder, cloudresourcemanager_projects_undelete_task,
    cloudresourcemanager_tag_bindings_create_builder, cloudresourcemanager_tag_bindings_create_task,
    cloudresourcemanager_tag_bindings_delete_builder, cloudresourcemanager_tag_bindings_delete_task,
    cloudresourcemanager_tag_bindings_list_builder, cloudresourcemanager_tag_bindings_list_task,
    cloudresourcemanager_tag_keys_create_builder, cloudresourcemanager_tag_keys_create_task,
    cloudresourcemanager_tag_keys_delete_builder, cloudresourcemanager_tag_keys_delete_task,
    cloudresourcemanager_tag_keys_get_builder, cloudresourcemanager_tag_keys_get_task,
    cloudresourcemanager_tag_keys_get_iam_policy_builder, cloudresourcemanager_tag_keys_get_iam_policy_task,
    cloudresourcemanager_tag_keys_get_namespaced_builder, cloudresourcemanager_tag_keys_get_namespaced_task,
    cloudresourcemanager_tag_keys_list_builder, cloudresourcemanager_tag_keys_list_task,
    cloudresourcemanager_tag_keys_patch_builder, cloudresourcemanager_tag_keys_patch_task,
    cloudresourcemanager_tag_keys_set_iam_policy_builder, cloudresourcemanager_tag_keys_set_iam_policy_task,
    cloudresourcemanager_tag_keys_test_iam_permissions_builder, cloudresourcemanager_tag_keys_test_iam_permissions_task,
    cloudresourcemanager_tag_values_create_builder, cloudresourcemanager_tag_values_create_task,
    cloudresourcemanager_tag_values_delete_builder, cloudresourcemanager_tag_values_delete_task,
    cloudresourcemanager_tag_values_get_builder, cloudresourcemanager_tag_values_get_task,
    cloudresourcemanager_tag_values_get_iam_policy_builder, cloudresourcemanager_tag_values_get_iam_policy_task,
    cloudresourcemanager_tag_values_get_namespaced_builder, cloudresourcemanager_tag_values_get_namespaced_task,
    cloudresourcemanager_tag_values_list_builder, cloudresourcemanager_tag_values_list_task,
    cloudresourcemanager_tag_values_patch_builder, cloudresourcemanager_tag_values_patch_task,
    cloudresourcemanager_tag_values_set_iam_policy_builder, cloudresourcemanager_tag_values_set_iam_policy_task,
    cloudresourcemanager_tag_values_test_iam_permissions_builder, cloudresourcemanager_tag_values_test_iam_permissions_task,
    cloudresourcemanager_tag_values_tag_holds_create_builder, cloudresourcemanager_tag_values_tag_holds_create_task,
    cloudresourcemanager_tag_values_tag_holds_delete_builder, cloudresourcemanager_tag_values_tag_holds_delete_task,
    cloudresourcemanager_tag_values_tag_holds_list_builder, cloudresourcemanager_tag_values_tag_holds_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::cloudresourcemanager::Capability;
use crate::providers::gcp::clients::cloudresourcemanager::EffectiveTagBindingCollection;
use crate::providers::gcp::clients::cloudresourcemanager::Empty;
use crate::providers::gcp::clients::cloudresourcemanager::Folder;
use crate::providers::gcp::clients::cloudresourcemanager::Lien;
use crate::providers::gcp::clients::cloudresourcemanager::ListEffectiveTagsResponse;
use crate::providers::gcp::clients::cloudresourcemanager::ListFoldersResponse;
use crate::providers::gcp::clients::cloudresourcemanager::ListLiensResponse;
use crate::providers::gcp::clients::cloudresourcemanager::ListProjectsResponse;
use crate::providers::gcp::clients::cloudresourcemanager::ListTagBindingsResponse;
use crate::providers::gcp::clients::cloudresourcemanager::ListTagHoldsResponse;
use crate::providers::gcp::clients::cloudresourcemanager::ListTagKeysResponse;
use crate::providers::gcp::clients::cloudresourcemanager::ListTagValuesResponse;
use crate::providers::gcp::clients::cloudresourcemanager::Operation;
use crate::providers::gcp::clients::cloudresourcemanager::Organization;
use crate::providers::gcp::clients::cloudresourcemanager::Policy;
use crate::providers::gcp::clients::cloudresourcemanager::Project;
use crate::providers::gcp::clients::cloudresourcemanager::SearchFoldersResponse;
use crate::providers::gcp::clients::cloudresourcemanager::SearchOrganizationsResponse;
use crate::providers::gcp::clients::cloudresourcemanager::SearchProjectsResponse;
use crate::providers::gcp::clients::cloudresourcemanager::TagBindingCollection;
use crate::providers::gcp::clients::cloudresourcemanager::TagKey;
use crate::providers::gcp::clients::cloudresourcemanager::TagValue;
use crate::providers::gcp::clients::cloudresourcemanager::TestIamPermissionsResponse;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerEffectiveTagsListArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerFoldersCapabilitiesGetArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerFoldersCapabilitiesPatchArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerFoldersDeleteArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerFoldersGetArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerFoldersGetIamPolicyArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerFoldersListArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerFoldersMoveArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerFoldersPatchArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerFoldersSearchArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerFoldersSetIamPolicyArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerFoldersTestIamPermissionsArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerFoldersUndeleteArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerLiensDeleteArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerLiensGetArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerLiensListArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerLocationsEffectiveTagBindingCollectionsGetArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerLocationsTagBindingCollectionsGetArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerLocationsTagBindingCollectionsPatchArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerOperationsGetArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerOrganizationsGetArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerOrganizationsGetIamPolicyArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerOrganizationsSearchArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerOrganizationsSetIamPolicyArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerOrganizationsTestIamPermissionsArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerProjectsDeleteArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerProjectsGetArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerProjectsGetIamPolicyArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerProjectsListArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerProjectsMoveArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerProjectsPatchArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerProjectsSearchArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerProjectsSetIamPolicyArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerProjectsTestIamPermissionsArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerProjectsUndeleteArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerTagBindingsCreateArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerTagBindingsDeleteArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerTagBindingsListArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerTagKeysCreateArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerTagKeysDeleteArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerTagKeysGetArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerTagKeysGetIamPolicyArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerTagKeysGetNamespacedArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerTagKeysListArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerTagKeysPatchArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerTagKeysSetIamPolicyArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerTagKeysTestIamPermissionsArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerTagValuesCreateArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerTagValuesDeleteArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerTagValuesGetArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerTagValuesGetIamPolicyArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerTagValuesGetNamespacedArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerTagValuesListArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerTagValuesPatchArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerTagValuesSetIamPolicyArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerTagValuesTagHoldsCreateArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerTagValuesTagHoldsDeleteArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerTagValuesTagHoldsListArgs;
use crate::providers::gcp::clients::cloudresourcemanager::CloudresourcemanagerTagValuesTestIamPermissionsArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// CloudresourcemanagerProvider with automatic state tracking.
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
/// let provider = CloudresourcemanagerProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct CloudresourcemanagerProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> CloudresourcemanagerProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new CloudresourcemanagerProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new CloudresourcemanagerProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Cloudresourcemanager effective tags list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListEffectiveTagsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudresourcemanager_effective_tags_list(
        &self,
        args: &CloudresourcemanagerEffectiveTagsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListEffectiveTagsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_effective_tags_list_builder(
            &self.http_client,
            &args.pageSize,
            &args.pageToken,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_effective_tags_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager folders create.
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
    pub fn cloudresourcemanager_folders_create(
        &self,
        args: &CloudresourcemanagerFoldersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_folders_create_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_folders_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager folders delete.
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
    pub fn cloudresourcemanager_folders_delete(
        &self,
        args: &CloudresourcemanagerFoldersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_folders_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_folders_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager folders get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Folder result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudresourcemanager_folders_get(
        &self,
        args: &CloudresourcemanagerFoldersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Folder, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_folders_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_folders_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager folders get iam policy.
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
    pub fn cloudresourcemanager_folders_get_iam_policy(
        &self,
        args: &CloudresourcemanagerFoldersGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_folders_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_folders_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager folders list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListFoldersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudresourcemanager_folders_list(
        &self,
        args: &CloudresourcemanagerFoldersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListFoldersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_folders_list_builder(
            &self.http_client,
            &args.pageSize,
            &args.pageToken,
            &args.parent,
            &args.showDeleted,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_folders_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager folders move.
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
    pub fn cloudresourcemanager_folders_move(
        &self,
        args: &CloudresourcemanagerFoldersMoveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_folders_move_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_folders_move_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager folders patch.
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
    pub fn cloudresourcemanager_folders_patch(
        &self,
        args: &CloudresourcemanagerFoldersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_folders_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_folders_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager folders search.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchFoldersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudresourcemanager_folders_search(
        &self,
        args: &CloudresourcemanagerFoldersSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchFoldersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_folders_search_builder(
            &self.http_client,
            &args.pageSize,
            &args.pageToken,
            &args.query,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_folders_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager folders set iam policy.
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
    pub fn cloudresourcemanager_folders_set_iam_policy(
        &self,
        args: &CloudresourcemanagerFoldersSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_folders_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_folders_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager folders test iam permissions.
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
    pub fn cloudresourcemanager_folders_test_iam_permissions(
        &self,
        args: &CloudresourcemanagerFoldersTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_folders_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_folders_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager folders undelete.
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
    pub fn cloudresourcemanager_folders_undelete(
        &self,
        args: &CloudresourcemanagerFoldersUndeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_folders_undelete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_folders_undelete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager folders capabilities get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Capability result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudresourcemanager_folders_capabilities_get(
        &self,
        args: &CloudresourcemanagerFoldersCapabilitiesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Capability, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_folders_capabilities_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_folders_capabilities_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager folders capabilities patch.
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
    pub fn cloudresourcemanager_folders_capabilities_patch(
        &self,
        args: &CloudresourcemanagerFoldersCapabilitiesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_folders_capabilities_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_folders_capabilities_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager liens create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Lien result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudresourcemanager_liens_create(
        &self,
        args: &CloudresourcemanagerLiensCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Lien, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_liens_create_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_liens_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager liens delete.
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
    pub fn cloudresourcemanager_liens_delete(
        &self,
        args: &CloudresourcemanagerLiensDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_liens_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_liens_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager liens get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Lien result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudresourcemanager_liens_get(
        &self,
        args: &CloudresourcemanagerLiensGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Lien, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_liens_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_liens_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager liens list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListLiensResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudresourcemanager_liens_list(
        &self,
        args: &CloudresourcemanagerLiensListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLiensResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_liens_list_builder(
            &self.http_client,
            &args.pageSize,
            &args.pageToken,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_liens_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager locations effective tag binding collections get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EffectiveTagBindingCollection result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudresourcemanager_locations_effective_tag_binding_collections_get(
        &self,
        args: &CloudresourcemanagerLocationsEffectiveTagBindingCollectionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EffectiveTagBindingCollection, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_locations_effective_tag_binding_collections_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_locations_effective_tag_binding_collections_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager locations tag binding collections get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TagBindingCollection result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudresourcemanager_locations_tag_binding_collections_get(
        &self,
        args: &CloudresourcemanagerLocationsTagBindingCollectionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TagBindingCollection, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_locations_tag_binding_collections_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_locations_tag_binding_collections_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager locations tag binding collections patch.
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
    pub fn cloudresourcemanager_locations_tag_binding_collections_patch(
        &self,
        args: &CloudresourcemanagerLocationsTagBindingCollectionsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_locations_tag_binding_collections_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_locations_tag_binding_collections_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager operations get.
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
    pub fn cloudresourcemanager_operations_get(
        &self,
        args: &CloudresourcemanagerOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager organizations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Organization result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudresourcemanager_organizations_get(
        &self,
        args: &CloudresourcemanagerOrganizationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Organization, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_organizations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_organizations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager organizations get iam policy.
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
    pub fn cloudresourcemanager_organizations_get_iam_policy(
        &self,
        args: &CloudresourcemanagerOrganizationsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_organizations_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_organizations_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager organizations search.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchOrganizationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudresourcemanager_organizations_search(
        &self,
        args: &CloudresourcemanagerOrganizationsSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchOrganizationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_organizations_search_builder(
            &self.http_client,
            &args.pageSize,
            &args.pageToken,
            &args.query,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_organizations_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager organizations set iam policy.
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
    pub fn cloudresourcemanager_organizations_set_iam_policy(
        &self,
        args: &CloudresourcemanagerOrganizationsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_organizations_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_organizations_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager organizations test iam permissions.
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
    pub fn cloudresourcemanager_organizations_test_iam_permissions(
        &self,
        args: &CloudresourcemanagerOrganizationsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_organizations_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_organizations_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager projects create.
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
    pub fn cloudresourcemanager_projects_create(
        &self,
        args: &CloudresourcemanagerProjectsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_projects_create_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_projects_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager projects delete.
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
    pub fn cloudresourcemanager_projects_delete(
        &self,
        args: &CloudresourcemanagerProjectsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_projects_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_projects_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager projects get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Project result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudresourcemanager_projects_get(
        &self,
        args: &CloudresourcemanagerProjectsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Project, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_projects_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_projects_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager projects get iam policy.
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
    pub fn cloudresourcemanager_projects_get_iam_policy(
        &self,
        args: &CloudresourcemanagerProjectsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_projects_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_projects_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager projects list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListProjectsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudresourcemanager_projects_list(
        &self,
        args: &CloudresourcemanagerProjectsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListProjectsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_projects_list_builder(
            &self.http_client,
            &args.pageSize,
            &args.pageToken,
            &args.parent,
            &args.showDeleted,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_projects_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager projects move.
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
    pub fn cloudresourcemanager_projects_move(
        &self,
        args: &CloudresourcemanagerProjectsMoveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_projects_move_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_projects_move_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager projects patch.
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
    pub fn cloudresourcemanager_projects_patch(
        &self,
        args: &CloudresourcemanagerProjectsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_projects_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_projects_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager projects search.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchProjectsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudresourcemanager_projects_search(
        &self,
        args: &CloudresourcemanagerProjectsSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchProjectsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_projects_search_builder(
            &self.http_client,
            &args.pageSize,
            &args.pageToken,
            &args.query,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_projects_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager projects set iam policy.
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
    pub fn cloudresourcemanager_projects_set_iam_policy(
        &self,
        args: &CloudresourcemanagerProjectsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_projects_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_projects_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager projects test iam permissions.
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
    pub fn cloudresourcemanager_projects_test_iam_permissions(
        &self,
        args: &CloudresourcemanagerProjectsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_projects_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_projects_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager projects undelete.
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
    pub fn cloudresourcemanager_projects_undelete(
        &self,
        args: &CloudresourcemanagerProjectsUndeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_projects_undelete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_projects_undelete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager tag bindings create.
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
    pub fn cloudresourcemanager_tag_bindings_create(
        &self,
        args: &CloudresourcemanagerTagBindingsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_tag_bindings_create_builder(
            &self.http_client,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_tag_bindings_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager tag bindings delete.
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
    pub fn cloudresourcemanager_tag_bindings_delete(
        &self,
        args: &CloudresourcemanagerTagBindingsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_tag_bindings_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_tag_bindings_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager tag bindings list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListTagBindingsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudresourcemanager_tag_bindings_list(
        &self,
        args: &CloudresourcemanagerTagBindingsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListTagBindingsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_tag_bindings_list_builder(
            &self.http_client,
            &args.pageSize,
            &args.pageToken,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_tag_bindings_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager tag keys create.
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
    pub fn cloudresourcemanager_tag_keys_create(
        &self,
        args: &CloudresourcemanagerTagKeysCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_tag_keys_create_builder(
            &self.http_client,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_tag_keys_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager tag keys delete.
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
    pub fn cloudresourcemanager_tag_keys_delete(
        &self,
        args: &CloudresourcemanagerTagKeysDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_tag_keys_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_tag_keys_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager tag keys get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TagKey result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudresourcemanager_tag_keys_get(
        &self,
        args: &CloudresourcemanagerTagKeysGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TagKey, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_tag_keys_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_tag_keys_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager tag keys get iam policy.
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
    pub fn cloudresourcemanager_tag_keys_get_iam_policy(
        &self,
        args: &CloudresourcemanagerTagKeysGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_tag_keys_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_tag_keys_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager tag keys get namespaced.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TagKey result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudresourcemanager_tag_keys_get_namespaced(
        &self,
        args: &CloudresourcemanagerTagKeysGetNamespacedArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TagKey, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_tag_keys_get_namespaced_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_tag_keys_get_namespaced_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager tag keys list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListTagKeysResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudresourcemanager_tag_keys_list(
        &self,
        args: &CloudresourcemanagerTagKeysListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListTagKeysResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_tag_keys_list_builder(
            &self.http_client,
            &args.pageSize,
            &args.pageToken,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_tag_keys_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager tag keys patch.
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
    pub fn cloudresourcemanager_tag_keys_patch(
        &self,
        args: &CloudresourcemanagerTagKeysPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_tag_keys_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_tag_keys_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager tag keys set iam policy.
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
    pub fn cloudresourcemanager_tag_keys_set_iam_policy(
        &self,
        args: &CloudresourcemanagerTagKeysSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_tag_keys_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_tag_keys_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager tag keys test iam permissions.
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
    pub fn cloudresourcemanager_tag_keys_test_iam_permissions(
        &self,
        args: &CloudresourcemanagerTagKeysTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_tag_keys_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_tag_keys_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager tag values create.
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
    pub fn cloudresourcemanager_tag_values_create(
        &self,
        args: &CloudresourcemanagerTagValuesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_tag_values_create_builder(
            &self.http_client,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_tag_values_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager tag values delete.
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
    pub fn cloudresourcemanager_tag_values_delete(
        &self,
        args: &CloudresourcemanagerTagValuesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_tag_values_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_tag_values_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager tag values get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TagValue result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudresourcemanager_tag_values_get(
        &self,
        args: &CloudresourcemanagerTagValuesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TagValue, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_tag_values_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_tag_values_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager tag values get iam policy.
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
    pub fn cloudresourcemanager_tag_values_get_iam_policy(
        &self,
        args: &CloudresourcemanagerTagValuesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_tag_values_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_tag_values_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager tag values get namespaced.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TagValue result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudresourcemanager_tag_values_get_namespaced(
        &self,
        args: &CloudresourcemanagerTagValuesGetNamespacedArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TagValue, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_tag_values_get_namespaced_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_tag_values_get_namespaced_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager tag values list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListTagValuesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudresourcemanager_tag_values_list(
        &self,
        args: &CloudresourcemanagerTagValuesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListTagValuesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_tag_values_list_builder(
            &self.http_client,
            &args.pageSize,
            &args.pageToken,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_tag_values_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager tag values patch.
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
    pub fn cloudresourcemanager_tag_values_patch(
        &self,
        args: &CloudresourcemanagerTagValuesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_tag_values_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_tag_values_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager tag values set iam policy.
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
    pub fn cloudresourcemanager_tag_values_set_iam_policy(
        &self,
        args: &CloudresourcemanagerTagValuesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_tag_values_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_tag_values_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager tag values test iam permissions.
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
    pub fn cloudresourcemanager_tag_values_test_iam_permissions(
        &self,
        args: &CloudresourcemanagerTagValuesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_tag_values_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_tag_values_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager tag values tag holds create.
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
    pub fn cloudresourcemanager_tag_values_tag_holds_create(
        &self,
        args: &CloudresourcemanagerTagValuesTagHoldsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_tag_values_tag_holds_create_builder(
            &self.http_client,
            &args.parent,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_tag_values_tag_holds_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager tag values tag holds delete.
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
    pub fn cloudresourcemanager_tag_values_tag_holds_delete(
        &self,
        args: &CloudresourcemanagerTagValuesTagHoldsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_tag_values_tag_holds_delete_builder(
            &self.http_client,
            &args.name,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_tag_values_tag_holds_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudresourcemanager tag values tag holds list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListTagHoldsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudresourcemanager_tag_values_tag_holds_list(
        &self,
        args: &CloudresourcemanagerTagValuesTagHoldsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListTagHoldsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudresourcemanager_tag_values_tag_holds_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudresourcemanager_tag_values_tag_holds_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
