//! IapProvider - State-aware iap API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       iap API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::iap::{
    iap_projects_brands_create_builder, iap_projects_brands_create_task,
    iap_projects_brands_get_builder, iap_projects_brands_get_task,
    iap_projects_brands_list_builder, iap_projects_brands_list_task,
    iap_projects_brands_identity_aware_proxy_clients_create_builder, iap_projects_brands_identity_aware_proxy_clients_create_task,
    iap_projects_brands_identity_aware_proxy_clients_delete_builder, iap_projects_brands_identity_aware_proxy_clients_delete_task,
    iap_projects_brands_identity_aware_proxy_clients_get_builder, iap_projects_brands_identity_aware_proxy_clients_get_task,
    iap_projects_brands_identity_aware_proxy_clients_list_builder, iap_projects_brands_identity_aware_proxy_clients_list_task,
    iap_projects_brands_identity_aware_proxy_clients_reset_secret_builder, iap_projects_brands_identity_aware_proxy_clients_reset_secret_task,
    iap_projects_iap_tunnel_locations_dest_groups_create_builder, iap_projects_iap_tunnel_locations_dest_groups_create_task,
    iap_projects_iap_tunnel_locations_dest_groups_delete_builder, iap_projects_iap_tunnel_locations_dest_groups_delete_task,
    iap_projects_iap_tunnel_locations_dest_groups_get_builder, iap_projects_iap_tunnel_locations_dest_groups_get_task,
    iap_projects_iap_tunnel_locations_dest_groups_list_builder, iap_projects_iap_tunnel_locations_dest_groups_list_task,
    iap_projects_iap_tunnel_locations_dest_groups_patch_builder, iap_projects_iap_tunnel_locations_dest_groups_patch_task,
    iap_get_iam_policy_builder, iap_get_iam_policy_task,
    iap_get_iap_settings_builder, iap_get_iap_settings_task,
    iap_set_iam_policy_builder, iap_set_iam_policy_task,
    iap_test_iam_permissions_builder, iap_test_iam_permissions_task,
    iap_update_iap_settings_builder, iap_update_iap_settings_task,
    iap_validate_attribute_expression_builder, iap_validate_attribute_expression_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::iap::Brand;
use crate::providers::gcp::clients::iap::Empty;
use crate::providers::gcp::clients::iap::IapSettings;
use crate::providers::gcp::clients::iap::IdentityAwareProxyClient;
use crate::providers::gcp::clients::iap::ListBrandsResponse;
use crate::providers::gcp::clients::iap::ListIdentityAwareProxyClientsResponse;
use crate::providers::gcp::clients::iap::ListTunnelDestGroupsResponse;
use crate::providers::gcp::clients::iap::Policy;
use crate::providers::gcp::clients::iap::TestIamPermissionsResponse;
use crate::providers::gcp::clients::iap::TunnelDestGroup;
use crate::providers::gcp::clients::iap::ValidateIapAttributeExpressionResponse;
use crate::providers::gcp::clients::iap::IapGetIamPolicyArgs;
use crate::providers::gcp::clients::iap::IapGetIapSettingsArgs;
use crate::providers::gcp::clients::iap::IapProjectsBrandsCreateArgs;
use crate::providers::gcp::clients::iap::IapProjectsBrandsGetArgs;
use crate::providers::gcp::clients::iap::IapProjectsBrandsIdentityAwareProxyClientsCreateArgs;
use crate::providers::gcp::clients::iap::IapProjectsBrandsIdentityAwareProxyClientsDeleteArgs;
use crate::providers::gcp::clients::iap::IapProjectsBrandsIdentityAwareProxyClientsGetArgs;
use crate::providers::gcp::clients::iap::IapProjectsBrandsIdentityAwareProxyClientsListArgs;
use crate::providers::gcp::clients::iap::IapProjectsBrandsIdentityAwareProxyClientsResetSecretArgs;
use crate::providers::gcp::clients::iap::IapProjectsBrandsListArgs;
use crate::providers::gcp::clients::iap::IapProjectsIapTunnelLocationsDestGroupsCreateArgs;
use crate::providers::gcp::clients::iap::IapProjectsIapTunnelLocationsDestGroupsDeleteArgs;
use crate::providers::gcp::clients::iap::IapProjectsIapTunnelLocationsDestGroupsGetArgs;
use crate::providers::gcp::clients::iap::IapProjectsIapTunnelLocationsDestGroupsListArgs;
use crate::providers::gcp::clients::iap::IapProjectsIapTunnelLocationsDestGroupsPatchArgs;
use crate::providers::gcp::clients::iap::IapSetIamPolicyArgs;
use crate::providers::gcp::clients::iap::IapTestIamPermissionsArgs;
use crate::providers::gcp::clients::iap::IapUpdateIapSettingsArgs;
use crate::providers::gcp::clients::iap::IapValidateAttributeExpressionArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// IapProvider with automatic state tracking.
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
/// let provider = IapProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct IapProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> IapProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new IapProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Iap projects brands create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Brand result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn iap_projects_brands_create(
        &self,
        args: &IapProjectsBrandsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Brand, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = iap_projects_brands_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = iap_projects_brands_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Iap projects brands get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Brand result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn iap_projects_brands_get(
        &self,
        args: &IapProjectsBrandsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Brand, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = iap_projects_brands_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = iap_projects_brands_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Iap projects brands list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListBrandsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn iap_projects_brands_list(
        &self,
        args: &IapProjectsBrandsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBrandsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = iap_projects_brands_list_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = iap_projects_brands_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Iap projects brands identity aware proxy clients create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IdentityAwareProxyClient result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn iap_projects_brands_identity_aware_proxy_clients_create(
        &self,
        args: &IapProjectsBrandsIdentityAwareProxyClientsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IdentityAwareProxyClient, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = iap_projects_brands_identity_aware_proxy_clients_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = iap_projects_brands_identity_aware_proxy_clients_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Iap projects brands identity aware proxy clients delete.
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
    pub fn iap_projects_brands_identity_aware_proxy_clients_delete(
        &self,
        args: &IapProjectsBrandsIdentityAwareProxyClientsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = iap_projects_brands_identity_aware_proxy_clients_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = iap_projects_brands_identity_aware_proxy_clients_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Iap projects brands identity aware proxy clients get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IdentityAwareProxyClient result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn iap_projects_brands_identity_aware_proxy_clients_get(
        &self,
        args: &IapProjectsBrandsIdentityAwareProxyClientsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IdentityAwareProxyClient, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = iap_projects_brands_identity_aware_proxy_clients_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = iap_projects_brands_identity_aware_proxy_clients_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Iap projects brands identity aware proxy clients list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListIdentityAwareProxyClientsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn iap_projects_brands_identity_aware_proxy_clients_list(
        &self,
        args: &IapProjectsBrandsIdentityAwareProxyClientsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListIdentityAwareProxyClientsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = iap_projects_brands_identity_aware_proxy_clients_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = iap_projects_brands_identity_aware_proxy_clients_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Iap projects brands identity aware proxy clients reset secret.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IdentityAwareProxyClient result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn iap_projects_brands_identity_aware_proxy_clients_reset_secret(
        &self,
        args: &IapProjectsBrandsIdentityAwareProxyClientsResetSecretArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IdentityAwareProxyClient, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = iap_projects_brands_identity_aware_proxy_clients_reset_secret_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = iap_projects_brands_identity_aware_proxy_clients_reset_secret_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Iap projects iap tunnel locations dest groups create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TunnelDestGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn iap_projects_iap_tunnel_locations_dest_groups_create(
        &self,
        args: &IapProjectsIapTunnelLocationsDestGroupsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TunnelDestGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = iap_projects_iap_tunnel_locations_dest_groups_create_builder(
            &self.http_client,
            &args.parent,
            &args.tunnelDestGroupId,
        )
        .map_err(ProviderError::Api)?;

        let task = iap_projects_iap_tunnel_locations_dest_groups_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Iap projects iap tunnel locations dest groups delete.
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
    pub fn iap_projects_iap_tunnel_locations_dest_groups_delete(
        &self,
        args: &IapProjectsIapTunnelLocationsDestGroupsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = iap_projects_iap_tunnel_locations_dest_groups_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = iap_projects_iap_tunnel_locations_dest_groups_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Iap projects iap tunnel locations dest groups get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TunnelDestGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn iap_projects_iap_tunnel_locations_dest_groups_get(
        &self,
        args: &IapProjectsIapTunnelLocationsDestGroupsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TunnelDestGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = iap_projects_iap_tunnel_locations_dest_groups_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = iap_projects_iap_tunnel_locations_dest_groups_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Iap projects iap tunnel locations dest groups list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListTunnelDestGroupsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn iap_projects_iap_tunnel_locations_dest_groups_list(
        &self,
        args: &IapProjectsIapTunnelLocationsDestGroupsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListTunnelDestGroupsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = iap_projects_iap_tunnel_locations_dest_groups_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = iap_projects_iap_tunnel_locations_dest_groups_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Iap projects iap tunnel locations dest groups patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TunnelDestGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn iap_projects_iap_tunnel_locations_dest_groups_patch(
        &self,
        args: &IapProjectsIapTunnelLocationsDestGroupsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TunnelDestGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = iap_projects_iap_tunnel_locations_dest_groups_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = iap_projects_iap_tunnel_locations_dest_groups_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Iap get iam policy.
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
    pub fn iap_get_iam_policy(
        &self,
        args: &IapGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = iap_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = iap_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Iap get iap settings.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IapSettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn iap_get_iap_settings(
        &self,
        args: &IapGetIapSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IapSettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = iap_get_iap_settings_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = iap_get_iap_settings_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Iap set iam policy.
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
    pub fn iap_set_iam_policy(
        &self,
        args: &IapSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = iap_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = iap_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Iap test iam permissions.
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
    pub fn iap_test_iam_permissions(
        &self,
        args: &IapTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = iap_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = iap_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Iap update iap settings.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IapSettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn iap_update_iap_settings(
        &self,
        args: &IapUpdateIapSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IapSettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = iap_update_iap_settings_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = iap_update_iap_settings_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Iap validate attribute expression.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ValidateIapAttributeExpressionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn iap_validate_attribute_expression(
        &self,
        args: &IapValidateAttributeExpressionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ValidateIapAttributeExpressionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = iap_validate_attribute_expression_builder(
            &self.http_client,
            &args.name,
            &args.expression,
        )
        .map_err(ProviderError::Api)?;

        let task = iap_validate_attribute_expression_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
