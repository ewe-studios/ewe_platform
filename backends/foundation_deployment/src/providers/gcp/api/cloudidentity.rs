//! CloudidentityProvider - State-aware cloudidentity API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       cloudidentity API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::cloudidentity::{
    cloudidentity_customers_userinvitations_cancel_builder, cloudidentity_customers_userinvitations_cancel_task,
    cloudidentity_customers_userinvitations_get_builder, cloudidentity_customers_userinvitations_get_task,
    cloudidentity_customers_userinvitations_is_invitable_user_builder, cloudidentity_customers_userinvitations_is_invitable_user_task,
    cloudidentity_customers_userinvitations_list_builder, cloudidentity_customers_userinvitations_list_task,
    cloudidentity_customers_userinvitations_send_builder, cloudidentity_customers_userinvitations_send_task,
    cloudidentity_devices_cancel_wipe_builder, cloudidentity_devices_cancel_wipe_task,
    cloudidentity_devices_create_builder, cloudidentity_devices_create_task,
    cloudidentity_devices_delete_builder, cloudidentity_devices_delete_task,
    cloudidentity_devices_get_builder, cloudidentity_devices_get_task,
    cloudidentity_devices_list_builder, cloudidentity_devices_list_task,
    cloudidentity_devices_wipe_builder, cloudidentity_devices_wipe_task,
    cloudidentity_devices_device_users_approve_builder, cloudidentity_devices_device_users_approve_task,
    cloudidentity_devices_device_users_block_builder, cloudidentity_devices_device_users_block_task,
    cloudidentity_devices_device_users_cancel_wipe_builder, cloudidentity_devices_device_users_cancel_wipe_task,
    cloudidentity_devices_device_users_delete_builder, cloudidentity_devices_device_users_delete_task,
    cloudidentity_devices_device_users_get_builder, cloudidentity_devices_device_users_get_task,
    cloudidentity_devices_device_users_list_builder, cloudidentity_devices_device_users_list_task,
    cloudidentity_devices_device_users_lookup_builder, cloudidentity_devices_device_users_lookup_task,
    cloudidentity_devices_device_users_wipe_builder, cloudidentity_devices_device_users_wipe_task,
    cloudidentity_devices_device_users_client_states_get_builder, cloudidentity_devices_device_users_client_states_get_task,
    cloudidentity_devices_device_users_client_states_list_builder, cloudidentity_devices_device_users_client_states_list_task,
    cloudidentity_devices_device_users_client_states_patch_builder, cloudidentity_devices_device_users_client_states_patch_task,
    cloudidentity_groups_create_builder, cloudidentity_groups_create_task,
    cloudidentity_groups_delete_builder, cloudidentity_groups_delete_task,
    cloudidentity_groups_get_builder, cloudidentity_groups_get_task,
    cloudidentity_groups_get_security_settings_builder, cloudidentity_groups_get_security_settings_task,
    cloudidentity_groups_list_builder, cloudidentity_groups_list_task,
    cloudidentity_groups_lookup_builder, cloudidentity_groups_lookup_task,
    cloudidentity_groups_patch_builder, cloudidentity_groups_patch_task,
    cloudidentity_groups_search_builder, cloudidentity_groups_search_task,
    cloudidentity_groups_update_security_settings_builder, cloudidentity_groups_update_security_settings_task,
    cloudidentity_groups_memberships_check_transitive_membership_builder, cloudidentity_groups_memberships_check_transitive_membership_task,
    cloudidentity_groups_memberships_create_builder, cloudidentity_groups_memberships_create_task,
    cloudidentity_groups_memberships_delete_builder, cloudidentity_groups_memberships_delete_task,
    cloudidentity_groups_memberships_get_builder, cloudidentity_groups_memberships_get_task,
    cloudidentity_groups_memberships_get_membership_graph_builder, cloudidentity_groups_memberships_get_membership_graph_task,
    cloudidentity_groups_memberships_list_builder, cloudidentity_groups_memberships_list_task,
    cloudidentity_groups_memberships_lookup_builder, cloudidentity_groups_memberships_lookup_task,
    cloudidentity_groups_memberships_modify_membership_roles_builder, cloudidentity_groups_memberships_modify_membership_roles_task,
    cloudidentity_groups_memberships_search_direct_groups_builder, cloudidentity_groups_memberships_search_direct_groups_task,
    cloudidentity_groups_memberships_search_transitive_groups_builder, cloudidentity_groups_memberships_search_transitive_groups_task,
    cloudidentity_groups_memberships_search_transitive_memberships_builder, cloudidentity_groups_memberships_search_transitive_memberships_task,
    cloudidentity_inbound_oidc_sso_profiles_create_builder, cloudidentity_inbound_oidc_sso_profiles_create_task,
    cloudidentity_inbound_oidc_sso_profiles_delete_builder, cloudidentity_inbound_oidc_sso_profiles_delete_task,
    cloudidentity_inbound_oidc_sso_profiles_get_builder, cloudidentity_inbound_oidc_sso_profiles_get_task,
    cloudidentity_inbound_oidc_sso_profiles_list_builder, cloudidentity_inbound_oidc_sso_profiles_list_task,
    cloudidentity_inbound_oidc_sso_profiles_patch_builder, cloudidentity_inbound_oidc_sso_profiles_patch_task,
    cloudidentity_inbound_saml_sso_profiles_create_builder, cloudidentity_inbound_saml_sso_profiles_create_task,
    cloudidentity_inbound_saml_sso_profiles_delete_builder, cloudidentity_inbound_saml_sso_profiles_delete_task,
    cloudidentity_inbound_saml_sso_profiles_get_builder, cloudidentity_inbound_saml_sso_profiles_get_task,
    cloudidentity_inbound_saml_sso_profiles_list_builder, cloudidentity_inbound_saml_sso_profiles_list_task,
    cloudidentity_inbound_saml_sso_profiles_patch_builder, cloudidentity_inbound_saml_sso_profiles_patch_task,
    cloudidentity_inbound_saml_sso_profiles_idp_credentials_add_builder, cloudidentity_inbound_saml_sso_profiles_idp_credentials_add_task,
    cloudidentity_inbound_saml_sso_profiles_idp_credentials_delete_builder, cloudidentity_inbound_saml_sso_profiles_idp_credentials_delete_task,
    cloudidentity_inbound_saml_sso_profiles_idp_credentials_get_builder, cloudidentity_inbound_saml_sso_profiles_idp_credentials_get_task,
    cloudidentity_inbound_saml_sso_profiles_idp_credentials_list_builder, cloudidentity_inbound_saml_sso_profiles_idp_credentials_list_task,
    cloudidentity_inbound_sso_assignments_create_builder, cloudidentity_inbound_sso_assignments_create_task,
    cloudidentity_inbound_sso_assignments_delete_builder, cloudidentity_inbound_sso_assignments_delete_task,
    cloudidentity_inbound_sso_assignments_get_builder, cloudidentity_inbound_sso_assignments_get_task,
    cloudidentity_inbound_sso_assignments_list_builder, cloudidentity_inbound_sso_assignments_list_task,
    cloudidentity_inbound_sso_assignments_patch_builder, cloudidentity_inbound_sso_assignments_patch_task,
    cloudidentity_policies_get_builder, cloudidentity_policies_get_task,
    cloudidentity_policies_list_builder, cloudidentity_policies_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::cloudidentity::CheckTransitiveMembershipResponse;
use crate::providers::gcp::clients::cloudidentity::GoogleAppsCloudidentityDevicesV1ClientState;
use crate::providers::gcp::clients::cloudidentity::GoogleAppsCloudidentityDevicesV1Device;
use crate::providers::gcp::clients::cloudidentity::GoogleAppsCloudidentityDevicesV1DeviceUser;
use crate::providers::gcp::clients::cloudidentity::GoogleAppsCloudidentityDevicesV1ListClientStatesResponse;
use crate::providers::gcp::clients::cloudidentity::GoogleAppsCloudidentityDevicesV1ListDeviceUsersResponse;
use crate::providers::gcp::clients::cloudidentity::GoogleAppsCloudidentityDevicesV1ListDevicesResponse;
use crate::providers::gcp::clients::cloudidentity::GoogleAppsCloudidentityDevicesV1LookupSelfDeviceUsersResponse;
use crate::providers::gcp::clients::cloudidentity::Group;
use crate::providers::gcp::clients::cloudidentity::IdpCredential;
use crate::providers::gcp::clients::cloudidentity::InboundOidcSsoProfile;
use crate::providers::gcp::clients::cloudidentity::InboundSamlSsoProfile;
use crate::providers::gcp::clients::cloudidentity::InboundSsoAssignment;
use crate::providers::gcp::clients::cloudidentity::IsInvitableUserResponse;
use crate::providers::gcp::clients::cloudidentity::ListGroupsResponse;
use crate::providers::gcp::clients::cloudidentity::ListIdpCredentialsResponse;
use crate::providers::gcp::clients::cloudidentity::ListInboundOidcSsoProfilesResponse;
use crate::providers::gcp::clients::cloudidentity::ListInboundSamlSsoProfilesResponse;
use crate::providers::gcp::clients::cloudidentity::ListInboundSsoAssignmentsResponse;
use crate::providers::gcp::clients::cloudidentity::ListMembershipsResponse;
use crate::providers::gcp::clients::cloudidentity::ListPoliciesResponse;
use crate::providers::gcp::clients::cloudidentity::ListUserInvitationsResponse;
use crate::providers::gcp::clients::cloudidentity::LookupGroupNameResponse;
use crate::providers::gcp::clients::cloudidentity::LookupMembershipNameResponse;
use crate::providers::gcp::clients::cloudidentity::Membership;
use crate::providers::gcp::clients::cloudidentity::ModifyMembershipRolesResponse;
use crate::providers::gcp::clients::cloudidentity::Operation;
use crate::providers::gcp::clients::cloudidentity::Policy;
use crate::providers::gcp::clients::cloudidentity::SearchDirectGroupsResponse;
use crate::providers::gcp::clients::cloudidentity::SearchGroupsResponse;
use crate::providers::gcp::clients::cloudidentity::SearchTransitiveGroupsResponse;
use crate::providers::gcp::clients::cloudidentity::SearchTransitiveMembershipsResponse;
use crate::providers::gcp::clients::cloudidentity::SecuritySettings;
use crate::providers::gcp::clients::cloudidentity::UserInvitation;
use crate::providers::gcp::clients::cloudidentity::CloudidentityCustomersUserinvitationsCancelArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityCustomersUserinvitationsGetArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityCustomersUserinvitationsIsInvitableUserArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityCustomersUserinvitationsListArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityCustomersUserinvitationsSendArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityDevicesCancelWipeArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityDevicesCreateArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityDevicesDeleteArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityDevicesDeviceUsersApproveArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityDevicesDeviceUsersBlockArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityDevicesDeviceUsersCancelWipeArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityDevicesDeviceUsersClientStatesGetArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityDevicesDeviceUsersClientStatesListArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityDevicesDeviceUsersClientStatesPatchArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityDevicesDeviceUsersDeleteArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityDevicesDeviceUsersGetArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityDevicesDeviceUsersListArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityDevicesDeviceUsersLookupArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityDevicesDeviceUsersWipeArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityDevicesGetArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityDevicesListArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityDevicesWipeArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityGroupsCreateArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityGroupsDeleteArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityGroupsGetArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityGroupsGetSecuritySettingsArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityGroupsListArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityGroupsLookupArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityGroupsMembershipsCheckTransitiveMembershipArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityGroupsMembershipsCreateArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityGroupsMembershipsDeleteArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityGroupsMembershipsGetArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityGroupsMembershipsGetMembershipGraphArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityGroupsMembershipsListArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityGroupsMembershipsLookupArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityGroupsMembershipsModifyMembershipRolesArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityGroupsMembershipsSearchDirectGroupsArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityGroupsMembershipsSearchTransitiveGroupsArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityGroupsMembershipsSearchTransitiveMembershipsArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityGroupsPatchArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityGroupsSearchArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityGroupsUpdateSecuritySettingsArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityInboundOidcSsoProfilesDeleteArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityInboundOidcSsoProfilesGetArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityInboundOidcSsoProfilesListArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityInboundOidcSsoProfilesPatchArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityInboundSamlSsoProfilesDeleteArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityInboundSamlSsoProfilesGetArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityInboundSamlSsoProfilesIdpCredentialsAddArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityInboundSamlSsoProfilesIdpCredentialsDeleteArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityInboundSamlSsoProfilesIdpCredentialsGetArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityInboundSamlSsoProfilesIdpCredentialsListArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityInboundSamlSsoProfilesListArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityInboundSamlSsoProfilesPatchArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityInboundSsoAssignmentsDeleteArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityInboundSsoAssignmentsGetArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityInboundSsoAssignmentsListArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityInboundSsoAssignmentsPatchArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityPoliciesGetArgs;
use crate::providers::gcp::clients::cloudidentity::CloudidentityPoliciesListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// CloudidentityProvider with automatic state tracking.
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
/// let provider = CloudidentityProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct CloudidentityProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> CloudidentityProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new CloudidentityProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new CloudidentityProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Cloudidentity customers userinvitations cancel.
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
    pub fn cloudidentity_customers_userinvitations_cancel(
        &self,
        args: &CloudidentityCustomersUserinvitationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_customers_userinvitations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_customers_userinvitations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity customers userinvitations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UserInvitation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudidentity_customers_userinvitations_get(
        &self,
        args: &CloudidentityCustomersUserinvitationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UserInvitation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_customers_userinvitations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_customers_userinvitations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity customers userinvitations is invitable user.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IsInvitableUserResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudidentity_customers_userinvitations_is_invitable_user(
        &self,
        args: &CloudidentityCustomersUserinvitationsIsInvitableUserArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IsInvitableUserResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_customers_userinvitations_is_invitable_user_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_customers_userinvitations_is_invitable_user_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity customers userinvitations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListUserInvitationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudidentity_customers_userinvitations_list(
        &self,
        args: &CloudidentityCustomersUserinvitationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListUserInvitationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_customers_userinvitations_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_customers_userinvitations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity customers userinvitations send.
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
    pub fn cloudidentity_customers_userinvitations_send(
        &self,
        args: &CloudidentityCustomersUserinvitationsSendArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_customers_userinvitations_send_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_customers_userinvitations_send_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity devices cancel wipe.
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
    pub fn cloudidentity_devices_cancel_wipe(
        &self,
        args: &CloudidentityDevicesCancelWipeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_devices_cancel_wipe_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_devices_cancel_wipe_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity devices create.
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
    pub fn cloudidentity_devices_create(
        &self,
        args: &CloudidentityDevicesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_devices_create_builder(
            &self.http_client,
            &args.customer,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_devices_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity devices delete.
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
    pub fn cloudidentity_devices_delete(
        &self,
        args: &CloudidentityDevicesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_devices_delete_builder(
            &self.http_client,
            &args.name,
            &args.customer,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_devices_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity devices get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAppsCloudidentityDevicesV1Device result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudidentity_devices_get(
        &self,
        args: &CloudidentityDevicesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAppsCloudidentityDevicesV1Device, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_devices_get_builder(
            &self.http_client,
            &args.name,
            &args.customer,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_devices_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity devices list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAppsCloudidentityDevicesV1ListDevicesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudidentity_devices_list(
        &self,
        args: &CloudidentityDevicesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAppsCloudidentityDevicesV1ListDevicesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_devices_list_builder(
            &self.http_client,
            &args.customer,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_devices_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity devices wipe.
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
    pub fn cloudidentity_devices_wipe(
        &self,
        args: &CloudidentityDevicesWipeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_devices_wipe_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_devices_wipe_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity devices device users approve.
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
    pub fn cloudidentity_devices_device_users_approve(
        &self,
        args: &CloudidentityDevicesDeviceUsersApproveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_devices_device_users_approve_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_devices_device_users_approve_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity devices device users block.
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
    pub fn cloudidentity_devices_device_users_block(
        &self,
        args: &CloudidentityDevicesDeviceUsersBlockArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_devices_device_users_block_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_devices_device_users_block_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity devices device users cancel wipe.
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
    pub fn cloudidentity_devices_device_users_cancel_wipe(
        &self,
        args: &CloudidentityDevicesDeviceUsersCancelWipeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_devices_device_users_cancel_wipe_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_devices_device_users_cancel_wipe_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity devices device users delete.
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
    pub fn cloudidentity_devices_device_users_delete(
        &self,
        args: &CloudidentityDevicesDeviceUsersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_devices_device_users_delete_builder(
            &self.http_client,
            &args.name,
            &args.customer,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_devices_device_users_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity devices device users get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAppsCloudidentityDevicesV1DeviceUser result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudidentity_devices_device_users_get(
        &self,
        args: &CloudidentityDevicesDeviceUsersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAppsCloudidentityDevicesV1DeviceUser, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_devices_device_users_get_builder(
            &self.http_client,
            &args.name,
            &args.customer,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_devices_device_users_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity devices device users list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAppsCloudidentityDevicesV1ListDeviceUsersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudidentity_devices_device_users_list(
        &self,
        args: &CloudidentityDevicesDeviceUsersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAppsCloudidentityDevicesV1ListDeviceUsersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_devices_device_users_list_builder(
            &self.http_client,
            &args.parent,
            &args.customer,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_devices_device_users_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity devices device users lookup.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAppsCloudidentityDevicesV1LookupSelfDeviceUsersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudidentity_devices_device_users_lookup(
        &self,
        args: &CloudidentityDevicesDeviceUsersLookupArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAppsCloudidentityDevicesV1LookupSelfDeviceUsersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_devices_device_users_lookup_builder(
            &self.http_client,
            &args.parent,
            &args.androidId,
            &args.iosDeviceId,
            &args.pageSize,
            &args.pageToken,
            &args.partner,
            &args.rawResourceId,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_devices_device_users_lookup_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity devices device users wipe.
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
    pub fn cloudidentity_devices_device_users_wipe(
        &self,
        args: &CloudidentityDevicesDeviceUsersWipeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_devices_device_users_wipe_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_devices_device_users_wipe_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity devices device users client states get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAppsCloudidentityDevicesV1ClientState result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudidentity_devices_device_users_client_states_get(
        &self,
        args: &CloudidentityDevicesDeviceUsersClientStatesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAppsCloudidentityDevicesV1ClientState, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_devices_device_users_client_states_get_builder(
            &self.http_client,
            &args.name,
            &args.customer,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_devices_device_users_client_states_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity devices device users client states list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAppsCloudidentityDevicesV1ListClientStatesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudidentity_devices_device_users_client_states_list(
        &self,
        args: &CloudidentityDevicesDeviceUsersClientStatesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAppsCloudidentityDevicesV1ListClientStatesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_devices_device_users_client_states_list_builder(
            &self.http_client,
            &args.parent,
            &args.customer,
            &args.filter,
            &args.orderBy,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_devices_device_users_client_states_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity devices device users client states patch.
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
    pub fn cloudidentity_devices_device_users_client_states_patch(
        &self,
        args: &CloudidentityDevicesDeviceUsersClientStatesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_devices_device_users_client_states_patch_builder(
            &self.http_client,
            &args.name,
            &args.customer,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_devices_device_users_client_states_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity groups create.
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
    pub fn cloudidentity_groups_create(
        &self,
        args: &CloudidentityGroupsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_groups_create_builder(
            &self.http_client,
            &args.initialGroupConfig,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_groups_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity groups delete.
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
    pub fn cloudidentity_groups_delete(
        &self,
        args: &CloudidentityGroupsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_groups_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_groups_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity groups get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Group result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudidentity_groups_get(
        &self,
        args: &CloudidentityGroupsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Group, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_groups_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_groups_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity groups get security settings.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SecuritySettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudidentity_groups_get_security_settings(
        &self,
        args: &CloudidentityGroupsGetSecuritySettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SecuritySettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_groups_get_security_settings_builder(
            &self.http_client,
            &args.name,
            &args.readMask,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_groups_get_security_settings_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity groups list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListGroupsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudidentity_groups_list(
        &self,
        args: &CloudidentityGroupsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListGroupsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_groups_list_builder(
            &self.http_client,
            &args.pageSize,
            &args.pageToken,
            &args.parent,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_groups_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity groups lookup.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LookupGroupNameResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudidentity_groups_lookup(
        &self,
        args: &CloudidentityGroupsLookupArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LookupGroupNameResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_groups_lookup_builder(
            &self.http_client,
            &args.groupKey_id,
            &args.groupKey_namespace,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_groups_lookup_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity groups patch.
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
    pub fn cloudidentity_groups_patch(
        &self,
        args: &CloudidentityGroupsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_groups_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_groups_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity groups search.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchGroupsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudidentity_groups_search(
        &self,
        args: &CloudidentityGroupsSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchGroupsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_groups_search_builder(
            &self.http_client,
            &args.pageSize,
            &args.pageToken,
            &args.query,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_groups_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity groups update security settings.
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
    pub fn cloudidentity_groups_update_security_settings(
        &self,
        args: &CloudidentityGroupsUpdateSecuritySettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_groups_update_security_settings_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_groups_update_security_settings_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity groups memberships check transitive membership.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CheckTransitiveMembershipResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudidentity_groups_memberships_check_transitive_membership(
        &self,
        args: &CloudidentityGroupsMembershipsCheckTransitiveMembershipArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CheckTransitiveMembershipResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_groups_memberships_check_transitive_membership_builder(
            &self.http_client,
            &args.parent,
            &args.query,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_groups_memberships_check_transitive_membership_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity groups memberships create.
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
    pub fn cloudidentity_groups_memberships_create(
        &self,
        args: &CloudidentityGroupsMembershipsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_groups_memberships_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_groups_memberships_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity groups memberships delete.
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
    pub fn cloudidentity_groups_memberships_delete(
        &self,
        args: &CloudidentityGroupsMembershipsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_groups_memberships_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_groups_memberships_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity groups memberships get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Membership result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudidentity_groups_memberships_get(
        &self,
        args: &CloudidentityGroupsMembershipsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Membership, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_groups_memberships_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_groups_memberships_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity groups memberships get membership graph.
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
    pub fn cloudidentity_groups_memberships_get_membership_graph(
        &self,
        args: &CloudidentityGroupsMembershipsGetMembershipGraphArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_groups_memberships_get_membership_graph_builder(
            &self.http_client,
            &args.parent,
            &args.query,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_groups_memberships_get_membership_graph_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity groups memberships list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListMembershipsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudidentity_groups_memberships_list(
        &self,
        args: &CloudidentityGroupsMembershipsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListMembershipsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_groups_memberships_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_groups_memberships_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity groups memberships lookup.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LookupMembershipNameResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudidentity_groups_memberships_lookup(
        &self,
        args: &CloudidentityGroupsMembershipsLookupArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LookupMembershipNameResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_groups_memberships_lookup_builder(
            &self.http_client,
            &args.parent,
            &args.memberKey_id,
            &args.memberKey_namespace,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_groups_memberships_lookup_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity groups memberships modify membership roles.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ModifyMembershipRolesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudidentity_groups_memberships_modify_membership_roles(
        &self,
        args: &CloudidentityGroupsMembershipsModifyMembershipRolesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ModifyMembershipRolesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_groups_memberships_modify_membership_roles_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_groups_memberships_modify_membership_roles_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity groups memberships search direct groups.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchDirectGroupsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudidentity_groups_memberships_search_direct_groups(
        &self,
        args: &CloudidentityGroupsMembershipsSearchDirectGroupsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchDirectGroupsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_groups_memberships_search_direct_groups_builder(
            &self.http_client,
            &args.parent,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.query,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_groups_memberships_search_direct_groups_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity groups memberships search transitive groups.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchTransitiveGroupsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudidentity_groups_memberships_search_transitive_groups(
        &self,
        args: &CloudidentityGroupsMembershipsSearchTransitiveGroupsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchTransitiveGroupsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_groups_memberships_search_transitive_groups_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.query,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_groups_memberships_search_transitive_groups_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity groups memberships search transitive memberships.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchTransitiveMembershipsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudidentity_groups_memberships_search_transitive_memberships(
        &self,
        args: &CloudidentityGroupsMembershipsSearchTransitiveMembershipsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchTransitiveMembershipsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_groups_memberships_search_transitive_memberships_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_groups_memberships_search_transitive_memberships_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity inbound oidc sso profiles create.
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
    pub fn cloudidentity_inbound_oidc_sso_profiles_create(
        &self,
        args: &CloudidentityInboundOidcSsoProfilesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_inbound_oidc_sso_profiles_create_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_inbound_oidc_sso_profiles_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity inbound oidc sso profiles delete.
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
    pub fn cloudidentity_inbound_oidc_sso_profiles_delete(
        &self,
        args: &CloudidentityInboundOidcSsoProfilesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_inbound_oidc_sso_profiles_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_inbound_oidc_sso_profiles_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity inbound oidc sso profiles get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the InboundOidcSsoProfile result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudidentity_inbound_oidc_sso_profiles_get(
        &self,
        args: &CloudidentityInboundOidcSsoProfilesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<InboundOidcSsoProfile, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_inbound_oidc_sso_profiles_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_inbound_oidc_sso_profiles_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity inbound oidc sso profiles list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListInboundOidcSsoProfilesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudidentity_inbound_oidc_sso_profiles_list(
        &self,
        args: &CloudidentityInboundOidcSsoProfilesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListInboundOidcSsoProfilesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_inbound_oidc_sso_profiles_list_builder(
            &self.http_client,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_inbound_oidc_sso_profiles_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity inbound oidc sso profiles patch.
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
    pub fn cloudidentity_inbound_oidc_sso_profiles_patch(
        &self,
        args: &CloudidentityInboundOidcSsoProfilesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_inbound_oidc_sso_profiles_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_inbound_oidc_sso_profiles_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity inbound saml sso profiles create.
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
    pub fn cloudidentity_inbound_saml_sso_profiles_create(
        &self,
        args: &CloudidentityInboundSamlSsoProfilesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_inbound_saml_sso_profiles_create_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_inbound_saml_sso_profiles_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity inbound saml sso profiles delete.
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
    pub fn cloudidentity_inbound_saml_sso_profiles_delete(
        &self,
        args: &CloudidentityInboundSamlSsoProfilesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_inbound_saml_sso_profiles_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_inbound_saml_sso_profiles_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity inbound saml sso profiles get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the InboundSamlSsoProfile result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudidentity_inbound_saml_sso_profiles_get(
        &self,
        args: &CloudidentityInboundSamlSsoProfilesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<InboundSamlSsoProfile, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_inbound_saml_sso_profiles_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_inbound_saml_sso_profiles_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity inbound saml sso profiles list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListInboundSamlSsoProfilesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudidentity_inbound_saml_sso_profiles_list(
        &self,
        args: &CloudidentityInboundSamlSsoProfilesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListInboundSamlSsoProfilesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_inbound_saml_sso_profiles_list_builder(
            &self.http_client,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_inbound_saml_sso_profiles_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity inbound saml sso profiles patch.
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
    pub fn cloudidentity_inbound_saml_sso_profiles_patch(
        &self,
        args: &CloudidentityInboundSamlSsoProfilesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_inbound_saml_sso_profiles_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_inbound_saml_sso_profiles_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity inbound saml sso profiles idp credentials add.
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
    pub fn cloudidentity_inbound_saml_sso_profiles_idp_credentials_add(
        &self,
        args: &CloudidentityInboundSamlSsoProfilesIdpCredentialsAddArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_inbound_saml_sso_profiles_idp_credentials_add_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_inbound_saml_sso_profiles_idp_credentials_add_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity inbound saml sso profiles idp credentials delete.
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
    pub fn cloudidentity_inbound_saml_sso_profiles_idp_credentials_delete(
        &self,
        args: &CloudidentityInboundSamlSsoProfilesIdpCredentialsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_inbound_saml_sso_profiles_idp_credentials_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_inbound_saml_sso_profiles_idp_credentials_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity inbound saml sso profiles idp credentials get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IdpCredential result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudidentity_inbound_saml_sso_profiles_idp_credentials_get(
        &self,
        args: &CloudidentityInboundSamlSsoProfilesIdpCredentialsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IdpCredential, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_inbound_saml_sso_profiles_idp_credentials_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_inbound_saml_sso_profiles_idp_credentials_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity inbound saml sso profiles idp credentials list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListIdpCredentialsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudidentity_inbound_saml_sso_profiles_idp_credentials_list(
        &self,
        args: &CloudidentityInboundSamlSsoProfilesIdpCredentialsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListIdpCredentialsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_inbound_saml_sso_profiles_idp_credentials_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_inbound_saml_sso_profiles_idp_credentials_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity inbound sso assignments create.
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
    pub fn cloudidentity_inbound_sso_assignments_create(
        &self,
        args: &CloudidentityInboundSsoAssignmentsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_inbound_sso_assignments_create_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_inbound_sso_assignments_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity inbound sso assignments delete.
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
    pub fn cloudidentity_inbound_sso_assignments_delete(
        &self,
        args: &CloudidentityInboundSsoAssignmentsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_inbound_sso_assignments_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_inbound_sso_assignments_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity inbound sso assignments get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the InboundSsoAssignment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudidentity_inbound_sso_assignments_get(
        &self,
        args: &CloudidentityInboundSsoAssignmentsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<InboundSsoAssignment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_inbound_sso_assignments_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_inbound_sso_assignments_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity inbound sso assignments list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListInboundSsoAssignmentsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudidentity_inbound_sso_assignments_list(
        &self,
        args: &CloudidentityInboundSsoAssignmentsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListInboundSsoAssignmentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_inbound_sso_assignments_list_builder(
            &self.http_client,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_inbound_sso_assignments_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity inbound sso assignments patch.
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
    pub fn cloudidentity_inbound_sso_assignments_patch(
        &self,
        args: &CloudidentityInboundSsoAssignmentsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_inbound_sso_assignments_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_inbound_sso_assignments_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity policies get.
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
    pub fn cloudidentity_policies_get(
        &self,
        args: &CloudidentityPoliciesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_policies_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_policies_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudidentity policies list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListPoliciesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudidentity_policies_list(
        &self,
        args: &CloudidentityPoliciesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListPoliciesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudidentity_policies_list_builder(
            &self.http_client,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudidentity_policies_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
