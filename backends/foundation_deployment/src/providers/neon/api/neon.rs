//! NeonProvider - State-aware neon API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       neon API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "neon")]

use crate::providers::neon::clients::neon::{
    create_api_key_builder, create_api_key_task,
    revoke_api_key_builder, revoke_api_key_task,
    create_org_api_key_builder, create_org_api_key_task,
    revoke_org_api_key_builder, revoke_org_api_key_task,
    create_organization_invitations_builder, create_organization_invitations_task,
    update_organization_member_builder, update_organization_member_task,
    remove_organization_member_builder, remove_organization_member_task,
    assign_organization_v_p_c_endpoint_builder, assign_organization_v_p_c_endpoint_task,
    delete_organization_v_p_c_endpoint_builder, delete_organization_v_p_c_endpoint_task,
    transfer_projects_from_org_to_org_builder, transfer_projects_from_org_to_org_task,
    create_project_builder, create_project_task,
    create_neon_auth_integration_builder, create_neon_auth_integration_task,
    create_neon_auth_provider_s_d_k_keys_builder, create_neon_auth_provider_s_d_k_keys_task,
    transfer_neon_auth_provider_project_builder, transfer_neon_auth_provider_project_task,
    create_neon_auth_new_user_builder, create_neon_auth_new_user_task,
    update_project_builder, update_project_task,
    delete_project_builder, delete_project_task,
    add_neon_auth_domain_to_redirect_u_r_i_whitelist_builder, add_neon_auth_domain_to_redirect_u_r_i_whitelist_task,
    delete_neon_auth_domain_from_redirect_u_r_i_whitelist_builder, delete_neon_auth_domain_from_redirect_u_r_i_whitelist_task,
    update_neon_auth_email_server_builder, update_neon_auth_email_server_task,
    delete_neon_auth_integration_builder, delete_neon_auth_integration_task,
    add_neon_auth_oauth_provider_builder, add_neon_auth_oauth_provider_task,
    update_neon_auth_oauth_provider_builder, update_neon_auth_oauth_provider_task,
    delete_neon_auth_oauth_provider_builder, delete_neon_auth_oauth_provider_task,
    delete_neon_auth_user_builder, delete_neon_auth_user_task,
    create_project_branch_anonymized_builder, create_project_branch_anonymized_task,
    create_project_branch_builder, create_project_branch_task,
    update_project_branch_builder, update_project_branch_task,
    delete_project_branch_builder, delete_project_branch_task,
    start_anonymization_builder, start_anonymization_task,
    create_neon_auth_builder, create_neon_auth_task,
    disable_neon_auth_builder, disable_neon_auth_task,
    update_neon_auth_allow_localhost_builder, update_neon_auth_allow_localhost_task,
    add_branch_neon_auth_trusted_domain_builder, add_branch_neon_auth_trusted_domain_task,
    delete_branch_neon_auth_trusted_domain_builder, delete_branch_neon_auth_trusted_domain_task,
    update_neon_auth_email_and_password_config_builder, update_neon_auth_email_and_password_config_task,
    update_neon_auth_email_provider_builder, update_neon_auth_email_provider_task,
    add_branch_neon_auth_oauth_provider_builder, add_branch_neon_auth_oauth_provider_task,
    update_branch_neon_auth_oauth_provider_builder, update_branch_neon_auth_oauth_provider_task,
    delete_branch_neon_auth_oauth_provider_builder, delete_branch_neon_auth_oauth_provider_task,
    update_neon_auth_organization_plugin_builder, update_neon_auth_organization_plugin_task,
    send_neon_auth_test_email_builder, send_neon_auth_test_email_task,
    create_branch_neon_auth_new_user_builder, create_branch_neon_auth_new_user_task,
    delete_branch_neon_auth_user_builder, delete_branch_neon_auth_user_task,
    update_neon_auth_user_role_builder, update_neon_auth_user_role_task,
    update_neon_auth_webhook_config_builder, update_neon_auth_webhook_config_task,
    set_snapshot_schedule_builder, set_snapshot_schedule_task,
    create_project_branch_data_a_p_i_builder, create_project_branch_data_a_p_i_task,
    update_project_branch_data_a_p_i_builder, update_project_branch_data_a_p_i_task,
    delete_project_branch_data_a_p_i_builder, delete_project_branch_data_a_p_i_task,
    create_project_branch_database_builder, create_project_branch_database_task,
    update_project_branch_database_builder, update_project_branch_database_task,
    delete_project_branch_database_builder, delete_project_branch_database_task,
    finalize_restore_branch_builder, finalize_restore_branch_task,
    update_masking_rules_builder, update_masking_rules_task,
    restore_project_branch_builder, restore_project_branch_task,
    create_project_branch_role_builder, create_project_branch_role_task,
    delete_project_branch_role_builder, delete_project_branch_role_task,
    reset_project_branch_role_password_builder, reset_project_branch_role_password_task,
    set_default_project_branch_builder, set_default_project_branch_task,
    create_snapshot_builder, create_snapshot_task,
    create_project_endpoint_builder, create_project_endpoint_task,
    update_project_endpoint_builder, update_project_endpoint_task,
    delete_project_endpoint_builder, delete_project_endpoint_task,
    restart_project_endpoint_builder, restart_project_endpoint_task,
    start_project_endpoint_builder, start_project_endpoint_task,
    suspend_project_endpoint_builder, suspend_project_endpoint_task,
    add_project_j_w_k_s_builder, add_project_j_w_k_s_task,
    delete_project_j_w_k_s_builder, delete_project_j_w_k_s_task,
    grant_permission_to_project_builder, grant_permission_to_project_task,
    revoke_permission_from_project_builder, revoke_permission_from_project_task,
    recover_project_builder, recover_project_task,
    restore_project_builder, restore_project_task,
    update_snapshot_builder, update_snapshot_task,
    delete_snapshot_builder, delete_snapshot_task,
    restore_snapshot_builder, restore_snapshot_task,
    create_project_transfer_request_builder, create_project_transfer_request_task,
    accept_project_transfer_request_builder, accept_project_transfer_request_task,
    assign_project_v_p_c_endpoint_builder, assign_project_v_p_c_endpoint_task,
    delete_project_v_p_c_endpoint_builder, delete_project_v_p_c_endpoint_task,
    transfer_projects_from_user_to_org_builder, transfer_projects_from_user_to_org_task,
};
use crate::providers::neon::clients::types::{ApiError, ApiPending};
use crate::providers::neon::clients::neon::AnonymizedBranchStatusResponse;
use crate::providers::neon::clients::neon::ApiKeyCreateResponse;
use crate::providers::neon::clients::neon::ApiKeyRevokeResponse;
use crate::providers::neon::clients::neon::DataAPICreateResponse;
use crate::providers::neon::clients::neon::EmptyResponse;
use crate::providers::neon::clients::neon::JWKS;
use crate::providers::neon::clients::neon::MaskingRulesResponse;
use crate::providers::neon::clients::neon::Member;
use crate::providers::neon::clients::neon::NeonAuthAllowLocalhostResponse;
use crate::providers::neon::clients::neon::NeonAuthCreateIntegrationResponse;
use crate::providers::neon::clients::neon::NeonAuthCreateNewUserResponse;
use crate::providers::neon::clients::neon::NeonAuthEmailAndPasswordConfig;
use crate::providers::neon::clients::neon::NeonAuthOauthProvider;
use crate::providers::neon::clients::neon::NeonAuthOrganizationConfig;
use crate::providers::neon::clients::neon::NeonAuthTransferAuthProviderProjectResponse;
use crate::providers::neon::clients::neon::NeonAuthWebhookConfig;
use crate::providers::neon::clients::neon::OperationsResponse;
use crate::providers::neon::clients::neon::OrganizationInvitationsResponse;
use crate::providers::neon::clients::neon::ProjectPermission;
use crate::providers::neon::clients::neon::ProjectResponse;
use crate::providers::neon::clients::neon::ProjectTransferRequestResponse;
use crate::providers::neon::clients::neon::SendNeonAuthTestEmailResponse;
use crate::providers::neon::clients::neon::UpdateNeonAuthUserRoleResponse;
use crate::providers::neon::clients::neon::AcceptProjectTransferRequestArgs;
use crate::providers::neon::clients::neon::AddBranchNeonAuthOauthProviderArgs;
use crate::providers::neon::clients::neon::AddBranchNeonAuthTrustedDomainArgs;
use crate::providers::neon::clients::neon::AddNeonAuthDomainToRedirectURIWhitelistArgs;
use crate::providers::neon::clients::neon::AddNeonAuthOauthProviderArgs;
use crate::providers::neon::clients::neon::AddProjectJWKSArgs;
use crate::providers::neon::clients::neon::AssignOrganizationVPCEndpointArgs;
use crate::providers::neon::clients::neon::AssignProjectVPCEndpointArgs;
use crate::providers::neon::clients::neon::CreateApiKeyArgs;
use crate::providers::neon::clients::neon::CreateBranchNeonAuthNewUserArgs;
use crate::providers::neon::clients::neon::CreateNeonAuthArgs;
use crate::providers::neon::clients::neon::CreateNeonAuthIntegrationArgs;
use crate::providers::neon::clients::neon::CreateNeonAuthNewUserArgs;
use crate::providers::neon::clients::neon::CreateNeonAuthProviderSDKKeysArgs;
use crate::providers::neon::clients::neon::CreateOrgApiKeyArgs;
use crate::providers::neon::clients::neon::CreateOrganizationInvitationsArgs;
use crate::providers::neon::clients::neon::CreateProjectArgs;
use crate::providers::neon::clients::neon::CreateProjectBranchAnonymizedArgs;
use crate::providers::neon::clients::neon::CreateProjectBranchArgs;
use crate::providers::neon::clients::neon::CreateProjectBranchDataAPIArgs;
use crate::providers::neon::clients::neon::CreateProjectBranchDatabaseArgs;
use crate::providers::neon::clients::neon::CreateProjectBranchRoleArgs;
use crate::providers::neon::clients::neon::CreateProjectEndpointArgs;
use crate::providers::neon::clients::neon::CreateProjectTransferRequestArgs;
use crate::providers::neon::clients::neon::CreateSnapshotArgs;
use crate::providers::neon::clients::neon::DeleteBranchNeonAuthOauthProviderArgs;
use crate::providers::neon::clients::neon::DeleteBranchNeonAuthTrustedDomainArgs;
use crate::providers::neon::clients::neon::DeleteBranchNeonAuthUserArgs;
use crate::providers::neon::clients::neon::DeleteNeonAuthDomainFromRedirectURIWhitelistArgs;
use crate::providers::neon::clients::neon::DeleteNeonAuthIntegrationArgs;
use crate::providers::neon::clients::neon::DeleteNeonAuthOauthProviderArgs;
use crate::providers::neon::clients::neon::DeleteNeonAuthUserArgs;
use crate::providers::neon::clients::neon::DeleteOrganizationVPCEndpointArgs;
use crate::providers::neon::clients::neon::DeleteProjectArgs;
use crate::providers::neon::clients::neon::DeleteProjectBranchArgs;
use crate::providers::neon::clients::neon::DeleteProjectBranchDataAPIArgs;
use crate::providers::neon::clients::neon::DeleteProjectBranchDatabaseArgs;
use crate::providers::neon::clients::neon::DeleteProjectBranchRoleArgs;
use crate::providers::neon::clients::neon::DeleteProjectEndpointArgs;
use crate::providers::neon::clients::neon::DeleteProjectJWKSArgs;
use crate::providers::neon::clients::neon::DeleteProjectVPCEndpointArgs;
use crate::providers::neon::clients::neon::DeleteSnapshotArgs;
use crate::providers::neon::clients::neon::DisableNeonAuthArgs;
use crate::providers::neon::clients::neon::FinalizeRestoreBranchArgs;
use crate::providers::neon::clients::neon::GrantPermissionToProjectArgs;
use crate::providers::neon::clients::neon::RecoverProjectArgs;
use crate::providers::neon::clients::neon::RemoveOrganizationMemberArgs;
use crate::providers::neon::clients::neon::ResetProjectBranchRolePasswordArgs;
use crate::providers::neon::clients::neon::RestartProjectEndpointArgs;
use crate::providers::neon::clients::neon::RestoreProjectArgs;
use crate::providers::neon::clients::neon::RestoreProjectBranchArgs;
use crate::providers::neon::clients::neon::RestoreSnapshotArgs;
use crate::providers::neon::clients::neon::RevokeApiKeyArgs;
use crate::providers::neon::clients::neon::RevokeOrgApiKeyArgs;
use crate::providers::neon::clients::neon::RevokePermissionFromProjectArgs;
use crate::providers::neon::clients::neon::SendNeonAuthTestEmailArgs;
use crate::providers::neon::clients::neon::SetDefaultProjectBranchArgs;
use crate::providers::neon::clients::neon::SetSnapshotScheduleArgs;
use crate::providers::neon::clients::neon::StartAnonymizationArgs;
use crate::providers::neon::clients::neon::StartProjectEndpointArgs;
use crate::providers::neon::clients::neon::SuspendProjectEndpointArgs;
use crate::providers::neon::clients::neon::TransferNeonAuthProviderProjectArgs;
use crate::providers::neon::clients::neon::TransferProjectsFromOrgToOrgArgs;
use crate::providers::neon::clients::neon::TransferProjectsFromUserToOrgArgs;
use crate::providers::neon::clients::neon::UpdateBranchNeonAuthOauthProviderArgs;
use crate::providers::neon::clients::neon::UpdateMaskingRulesArgs;
use crate::providers::neon::clients::neon::UpdateNeonAuthAllowLocalhostArgs;
use crate::providers::neon::clients::neon::UpdateNeonAuthEmailAndPasswordConfigArgs;
use crate::providers::neon::clients::neon::UpdateNeonAuthEmailProviderArgs;
use crate::providers::neon::clients::neon::UpdateNeonAuthEmailServerArgs;
use crate::providers::neon::clients::neon::UpdateNeonAuthOauthProviderArgs;
use crate::providers::neon::clients::neon::UpdateNeonAuthOrganizationPluginArgs;
use crate::providers::neon::clients::neon::UpdateNeonAuthUserRoleArgs;
use crate::providers::neon::clients::neon::UpdateNeonAuthWebhookConfigArgs;
use crate::providers::neon::clients::neon::UpdateOrganizationMemberArgs;
use crate::providers::neon::clients::neon::UpdateProjectArgs;
use crate::providers::neon::clients::neon::UpdateProjectBranchArgs;
use crate::providers::neon::clients::neon::UpdateProjectBranchDataAPIArgs;
use crate::providers::neon::clients::neon::UpdateProjectBranchDatabaseArgs;
use crate::providers::neon::clients::neon::UpdateProjectEndpointArgs;
use crate::providers::neon::clients::neon::UpdateSnapshotArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// NeonProvider with automatic state tracking.
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
/// let provider = NeonProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct NeonProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> NeonProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new NeonProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Create api key.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApiKeyCreateResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn create_api_key(
        &self,
        args: &CreateApiKeyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApiKeyCreateResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = create_api_key_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = create_api_key_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Revoke api key.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApiKeyRevokeResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn revoke_api_key(
        &self,
        args: &RevokeApiKeyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApiKeyRevokeResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = revoke_api_key_builder(
            &self.http_client,
            &args.key_id,
        )
        .map_err(ProviderError::Api)?;

        let task = revoke_api_key_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Create org api key.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn create_org_api_key(
        &self,
        args: &CreateOrgApiKeyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = create_org_api_key_builder(
            &self.http_client,
            &args.org_id,
        )
        .map_err(ProviderError::Api)?;

        let task = create_org_api_key_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Revoke org api key.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn revoke_org_api_key(
        &self,
        args: &RevokeOrgApiKeyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = revoke_org_api_key_builder(
            &self.http_client,
            &args.key_id,
        )
        .map_err(ProviderError::Api)?;

        let task = revoke_org_api_key_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Create organization invitations.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the OrganizationInvitationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn create_organization_invitations(
        &self,
        args: &CreateOrganizationInvitationsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<OrganizationInvitationsResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = create_organization_invitations_builder(
            &self.http_client,
            &args.org_id,
        )
        .map_err(ProviderError::Api)?;

        let task = create_organization_invitations_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Update organization member.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Member result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn update_organization_member(
        &self,
        args: &UpdateOrganizationMemberArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Member, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = update_organization_member_builder(
            &self.http_client,
            &args.org_id,
            &args.member_id,
        )
        .map_err(ProviderError::Api)?;

        let task = update_organization_member_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Remove organization member.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EmptyResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn remove_organization_member(
        &self,
        args: &RemoveOrganizationMemberArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EmptyResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = remove_organization_member_builder(
            &self.http_client,
            &args.org_id,
            &args.member_id,
        )
        .map_err(ProviderError::Api)?;

        let task = remove_organization_member_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Assign organization v p c endpoint.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn assign_organization_v_p_c_endpoint(
        &self,
        args: &AssignOrganizationVPCEndpointArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = assign_organization_v_p_c_endpoint_builder(
            &self.http_client,
            &args.org_id,
            &args.region_id,
            &args.vpc_endpoint_id,
        )
        .map_err(ProviderError::Api)?;

        let task = assign_organization_v_p_c_endpoint_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete organization v p c endpoint.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_organization_v_p_c_endpoint(
        &self,
        args: &DeleteOrganizationVPCEndpointArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_organization_v_p_c_endpoint_builder(
            &self.http_client,
            &args.org_id,
            &args.region_id,
            &args.vpc_endpoint_id,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_organization_v_p_c_endpoint_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Transfer projects from org to org.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EmptyResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn transfer_projects_from_org_to_org(
        &self,
        args: &TransferProjectsFromOrgToOrgArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EmptyResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = transfer_projects_from_org_to_org_builder(
            &self.http_client,
            &args.source_org_id,
        )
        .map_err(ProviderError::Api)?;

        let task = transfer_projects_from_org_to_org_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Create project.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn create_project(
        &self,
        args: &CreateProjectArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = create_project_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = create_project_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Create neon auth integration.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NeonAuthCreateIntegrationResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn create_neon_auth_integration(
        &self,
        args: &CreateNeonAuthIntegrationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NeonAuthCreateIntegrationResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = create_neon_auth_integration_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = create_neon_auth_integration_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Create neon auth provider s d k keys.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NeonAuthCreateIntegrationResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn create_neon_auth_provider_s_d_k_keys(
        &self,
        args: &CreateNeonAuthProviderSDKKeysArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NeonAuthCreateIntegrationResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = create_neon_auth_provider_s_d_k_keys_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = create_neon_auth_provider_s_d_k_keys_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Transfer neon auth provider project.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NeonAuthTransferAuthProviderProjectResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn transfer_neon_auth_provider_project(
        &self,
        args: &TransferNeonAuthProviderProjectArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NeonAuthTransferAuthProviderProjectResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = transfer_neon_auth_provider_project_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = transfer_neon_auth_provider_project_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Create neon auth new user.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NeonAuthCreateNewUserResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn create_neon_auth_new_user(
        &self,
        args: &CreateNeonAuthNewUserArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NeonAuthCreateNewUserResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = create_neon_auth_new_user_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = create_neon_auth_new_user_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Update project.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn update_project(
        &self,
        args: &UpdateProjectArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = update_project_builder(
            &self.http_client,
            &args.project_id,
        )
        .map_err(ProviderError::Api)?;

        let task = update_project_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete project.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProjectResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_project(
        &self,
        args: &DeleteProjectArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProjectResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_project_builder(
            &self.http_client,
            &args.project_id,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_project_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Add neon auth domain to redirect u r i whitelist.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn add_neon_auth_domain_to_redirect_u_r_i_whitelist(
        &self,
        args: &AddNeonAuthDomainToRedirectURIWhitelistArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = add_neon_auth_domain_to_redirect_u_r_i_whitelist_builder(
            &self.http_client,
            &args.project_id,
        )
        .map_err(ProviderError::Api)?;

        let task = add_neon_auth_domain_to_redirect_u_r_i_whitelist_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete neon auth domain from redirect u r i whitelist.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_neon_auth_domain_from_redirect_u_r_i_whitelist(
        &self,
        args: &DeleteNeonAuthDomainFromRedirectURIWhitelistArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_neon_auth_domain_from_redirect_u_r_i_whitelist_builder(
            &self.http_client,
            &args.project_id,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_neon_auth_domain_from_redirect_u_r_i_whitelist_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Update neon auth email server.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn update_neon_auth_email_server(
        &self,
        args: &UpdateNeonAuthEmailServerArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = update_neon_auth_email_server_builder(
            &self.http_client,
            &args.project_id,
        )
        .map_err(ProviderError::Api)?;

        let task = update_neon_auth_email_server_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete neon auth integration.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_neon_auth_integration(
        &self,
        args: &DeleteNeonAuthIntegrationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_neon_auth_integration_builder(
            &self.http_client,
            &args.project_id,
            &args.auth_provider,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_neon_auth_integration_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Add neon auth oauth provider.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NeonAuthOauthProvider result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn add_neon_auth_oauth_provider(
        &self,
        args: &AddNeonAuthOauthProviderArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NeonAuthOauthProvider, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = add_neon_auth_oauth_provider_builder(
            &self.http_client,
            &args.project_id,
        )
        .map_err(ProviderError::Api)?;

        let task = add_neon_auth_oauth_provider_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Update neon auth oauth provider.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NeonAuthOauthProvider result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn update_neon_auth_oauth_provider(
        &self,
        args: &UpdateNeonAuthOauthProviderArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NeonAuthOauthProvider, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = update_neon_auth_oauth_provider_builder(
            &self.http_client,
            &args.project_id,
            &args.oauth_provider_id,
        )
        .map_err(ProviderError::Api)?;

        let task = update_neon_auth_oauth_provider_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete neon auth oauth provider.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_neon_auth_oauth_provider(
        &self,
        args: &DeleteNeonAuthOauthProviderArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_neon_auth_oauth_provider_builder(
            &self.http_client,
            &args.project_id,
            &args.oauth_provider_id,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_neon_auth_oauth_provider_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete neon auth user.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the () result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_neon_auth_user(
        &self,
        args: &DeleteNeonAuthUserArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_neon_auth_user_builder(
            &self.http_client,
            &args.project_id,
            &args.auth_user_id,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_neon_auth_user_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Create project branch anonymized.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn create_project_branch_anonymized(
        &self,
        args: &CreateProjectBranchAnonymizedArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = create_project_branch_anonymized_builder(
            &self.http_client,
            &args.project_id,
        )
        .map_err(ProviderError::Api)?;

        let task = create_project_branch_anonymized_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Create project branch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn create_project_branch(
        &self,
        args: &CreateProjectBranchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = create_project_branch_builder(
            &self.http_client,
            &args.project_id,
        )
        .map_err(ProviderError::Api)?;

        let task = create_project_branch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Update project branch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn update_project_branch(
        &self,
        args: &UpdateProjectBranchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = update_project_branch_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
        )
        .map_err(ProviderError::Api)?;

        let task = update_project_branch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete project branch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_project_branch(
        &self,
        args: &DeleteProjectBranchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_project_branch_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_project_branch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Start anonymization.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AnonymizedBranchStatusResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn start_anonymization(
        &self,
        args: &StartAnonymizationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AnonymizedBranchStatusResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = start_anonymization_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
        )
        .map_err(ProviderError::Api)?;

        let task = start_anonymization_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Create neon auth.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NeonAuthCreateIntegrationResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn create_neon_auth(
        &self,
        args: &CreateNeonAuthArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NeonAuthCreateIntegrationResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = create_neon_auth_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
        )
        .map_err(ProviderError::Api)?;

        let task = create_neon_auth_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Disable neon auth.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn disable_neon_auth(
        &self,
        args: &DisableNeonAuthArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = disable_neon_auth_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
        )
        .map_err(ProviderError::Api)?;

        let task = disable_neon_auth_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Update neon auth allow localhost.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NeonAuthAllowLocalhostResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn update_neon_auth_allow_localhost(
        &self,
        args: &UpdateNeonAuthAllowLocalhostArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NeonAuthAllowLocalhostResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = update_neon_auth_allow_localhost_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
        )
        .map_err(ProviderError::Api)?;

        let task = update_neon_auth_allow_localhost_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Add branch neon auth trusted domain.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn add_branch_neon_auth_trusted_domain(
        &self,
        args: &AddBranchNeonAuthTrustedDomainArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = add_branch_neon_auth_trusted_domain_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
        )
        .map_err(ProviderError::Api)?;

        let task = add_branch_neon_auth_trusted_domain_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete branch neon auth trusted domain.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_branch_neon_auth_trusted_domain(
        &self,
        args: &DeleteBranchNeonAuthTrustedDomainArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_branch_neon_auth_trusted_domain_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_branch_neon_auth_trusted_domain_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Update neon auth email and password config.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NeonAuthEmailAndPasswordConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn update_neon_auth_email_and_password_config(
        &self,
        args: &UpdateNeonAuthEmailAndPasswordConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NeonAuthEmailAndPasswordConfig, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = update_neon_auth_email_and_password_config_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
        )
        .map_err(ProviderError::Api)?;

        let task = update_neon_auth_email_and_password_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Update neon auth email provider.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn update_neon_auth_email_provider(
        &self,
        args: &UpdateNeonAuthEmailProviderArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = update_neon_auth_email_provider_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
        )
        .map_err(ProviderError::Api)?;

        let task = update_neon_auth_email_provider_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Add branch neon auth oauth provider.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NeonAuthOauthProvider result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn add_branch_neon_auth_oauth_provider(
        &self,
        args: &AddBranchNeonAuthOauthProviderArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NeonAuthOauthProvider, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = add_branch_neon_auth_oauth_provider_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
        )
        .map_err(ProviderError::Api)?;

        let task = add_branch_neon_auth_oauth_provider_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Update branch neon auth oauth provider.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NeonAuthOauthProvider result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn update_branch_neon_auth_oauth_provider(
        &self,
        args: &UpdateBranchNeonAuthOauthProviderArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NeonAuthOauthProvider, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = update_branch_neon_auth_oauth_provider_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
            &args.oauth_provider_id,
        )
        .map_err(ProviderError::Api)?;

        let task = update_branch_neon_auth_oauth_provider_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete branch neon auth oauth provider.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_branch_neon_auth_oauth_provider(
        &self,
        args: &DeleteBranchNeonAuthOauthProviderArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_branch_neon_auth_oauth_provider_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
            &args.oauth_provider_id,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_branch_neon_auth_oauth_provider_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Update neon auth organization plugin.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NeonAuthOrganizationConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn update_neon_auth_organization_plugin(
        &self,
        args: &UpdateNeonAuthOrganizationPluginArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NeonAuthOrganizationConfig, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = update_neon_auth_organization_plugin_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
        )
        .map_err(ProviderError::Api)?;

        let task = update_neon_auth_organization_plugin_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Send neon auth test email.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SendNeonAuthTestEmailResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn send_neon_auth_test_email(
        &self,
        args: &SendNeonAuthTestEmailArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SendNeonAuthTestEmailResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = send_neon_auth_test_email_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
        )
        .map_err(ProviderError::Api)?;

        let task = send_neon_auth_test_email_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Create branch neon auth new user.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NeonAuthCreateNewUserResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn create_branch_neon_auth_new_user(
        &self,
        args: &CreateBranchNeonAuthNewUserArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NeonAuthCreateNewUserResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = create_branch_neon_auth_new_user_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
        )
        .map_err(ProviderError::Api)?;

        let task = create_branch_neon_auth_new_user_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete branch neon auth user.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the () result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_branch_neon_auth_user(
        &self,
        args: &DeleteBranchNeonAuthUserArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_branch_neon_auth_user_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
            &args.auth_user_id,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_branch_neon_auth_user_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Update neon auth user role.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UpdateNeonAuthUserRoleResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn update_neon_auth_user_role(
        &self,
        args: &UpdateNeonAuthUserRoleArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UpdateNeonAuthUserRoleResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = update_neon_auth_user_role_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
            &args.auth_user_id,
        )
        .map_err(ProviderError::Api)?;

        let task = update_neon_auth_user_role_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Update neon auth webhook config.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NeonAuthWebhookConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn update_neon_auth_webhook_config(
        &self,
        args: &UpdateNeonAuthWebhookConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NeonAuthWebhookConfig, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = update_neon_auth_webhook_config_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
        )
        .map_err(ProviderError::Api)?;

        let task = update_neon_auth_webhook_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Set snapshot schedule.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn set_snapshot_schedule(
        &self,
        args: &SetSnapshotScheduleArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = set_snapshot_schedule_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
        )
        .map_err(ProviderError::Api)?;

        let task = set_snapshot_schedule_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Create project branch data a p i.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DataAPICreateResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn create_project_branch_data_a_p_i(
        &self,
        args: &CreateProjectBranchDataAPIArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DataAPICreateResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = create_project_branch_data_a_p_i_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
            &args.database_name,
        )
        .map_err(ProviderError::Api)?;

        let task = create_project_branch_data_a_p_i_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Update project branch data a p i.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EmptyResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn update_project_branch_data_a_p_i(
        &self,
        args: &UpdateProjectBranchDataAPIArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EmptyResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = update_project_branch_data_a_p_i_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
            &args.database_name,
        )
        .map_err(ProviderError::Api)?;

        let task = update_project_branch_data_a_p_i_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete project branch data a p i.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EmptyResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_project_branch_data_a_p_i(
        &self,
        args: &DeleteProjectBranchDataAPIArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EmptyResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_project_branch_data_a_p_i_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
            &args.database_name,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_project_branch_data_a_p_i_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Create project branch database.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn create_project_branch_database(
        &self,
        args: &CreateProjectBranchDatabaseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = create_project_branch_database_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
        )
        .map_err(ProviderError::Api)?;

        let task = create_project_branch_database_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Update project branch database.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn update_project_branch_database(
        &self,
        args: &UpdateProjectBranchDatabaseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = update_project_branch_database_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
            &args.database_name,
        )
        .map_err(ProviderError::Api)?;

        let task = update_project_branch_database_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete project branch database.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_project_branch_database(
        &self,
        args: &DeleteProjectBranchDatabaseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_project_branch_database_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
            &args.database_name,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_project_branch_database_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Finalize restore branch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the OperationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn finalize_restore_branch(
        &self,
        args: &FinalizeRestoreBranchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<OperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = finalize_restore_branch_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
        )
        .map_err(ProviderError::Api)?;

        let task = finalize_restore_branch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Update masking rules.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the MaskingRulesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn update_masking_rules(
        &self,
        args: &UpdateMaskingRulesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<MaskingRulesResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = update_masking_rules_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
        )
        .map_err(ProviderError::Api)?;

        let task = update_masking_rules_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Restore project branch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn restore_project_branch(
        &self,
        args: &RestoreProjectBranchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = restore_project_branch_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
        )
        .map_err(ProviderError::Api)?;

        let task = restore_project_branch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Create project branch role.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn create_project_branch_role(
        &self,
        args: &CreateProjectBranchRoleArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = create_project_branch_role_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
        )
        .map_err(ProviderError::Api)?;

        let task = create_project_branch_role_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete project branch role.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_project_branch_role(
        &self,
        args: &DeleteProjectBranchRoleArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_project_branch_role_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
            &args.role_name,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_project_branch_role_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Reset project branch role password.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn reset_project_branch_role_password(
        &self,
        args: &ResetProjectBranchRolePasswordArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = reset_project_branch_role_password_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
            &args.role_name,
        )
        .map_err(ProviderError::Api)?;

        let task = reset_project_branch_role_password_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Set default project branch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn set_default_project_branch(
        &self,
        args: &SetDefaultProjectBranchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = set_default_project_branch_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
        )
        .map_err(ProviderError::Api)?;

        let task = set_default_project_branch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Create snapshot.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn create_snapshot(
        &self,
        args: &CreateSnapshotArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = create_snapshot_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
            &args.lsn,
            &args.timestamp,
            &args.name,
            &args.expires_at,
        )
        .map_err(ProviderError::Api)?;

        let task = create_snapshot_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Create project endpoint.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn create_project_endpoint(
        &self,
        args: &CreateProjectEndpointArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = create_project_endpoint_builder(
            &self.http_client,
            &args.project_id,
        )
        .map_err(ProviderError::Api)?;

        let task = create_project_endpoint_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Update project endpoint.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn update_project_endpoint(
        &self,
        args: &UpdateProjectEndpointArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = update_project_endpoint_builder(
            &self.http_client,
            &args.project_id,
            &args.endpoint_id,
        )
        .map_err(ProviderError::Api)?;

        let task = update_project_endpoint_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete project endpoint.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_project_endpoint(
        &self,
        args: &DeleteProjectEndpointArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_project_endpoint_builder(
            &self.http_client,
            &args.project_id,
            &args.endpoint_id,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_project_endpoint_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Restart project endpoint.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn restart_project_endpoint(
        &self,
        args: &RestartProjectEndpointArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = restart_project_endpoint_builder(
            &self.http_client,
            &args.project_id,
            &args.endpoint_id,
        )
        .map_err(ProviderError::Api)?;

        let task = restart_project_endpoint_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Start project endpoint.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn start_project_endpoint(
        &self,
        args: &StartProjectEndpointArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = start_project_endpoint_builder(
            &self.http_client,
            &args.project_id,
            &args.endpoint_id,
        )
        .map_err(ProviderError::Api)?;

        let task = start_project_endpoint_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Suspend project endpoint.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn suspend_project_endpoint(
        &self,
        args: &SuspendProjectEndpointArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = suspend_project_endpoint_builder(
            &self.http_client,
            &args.project_id,
            &args.endpoint_id,
        )
        .map_err(ProviderError::Api)?;

        let task = suspend_project_endpoint_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Add project j w k s.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn add_project_j_w_k_s(
        &self,
        args: &AddProjectJWKSArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = add_project_j_w_k_s_builder(
            &self.http_client,
            &args.project_id,
        )
        .map_err(ProviderError::Api)?;

        let task = add_project_j_w_k_s_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete project j w k s.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the JWKS result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_project_j_w_k_s(
        &self,
        args: &DeleteProjectJWKSArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<JWKS, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_project_j_w_k_s_builder(
            &self.http_client,
            &args.project_id,
            &args.jwks_id,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_project_j_w_k_s_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Grant permission to project.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProjectPermission result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn grant_permission_to_project(
        &self,
        args: &GrantPermissionToProjectArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProjectPermission, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = grant_permission_to_project_builder(
            &self.http_client,
            &args.project_id,
        )
        .map_err(ProviderError::Api)?;

        let task = grant_permission_to_project_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Revoke permission from project.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProjectPermission result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn revoke_permission_from_project(
        &self,
        args: &RevokePermissionFromProjectArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProjectPermission, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = revoke_permission_from_project_builder(
            &self.http_client,
            &args.project_id,
            &args.permission_id,
        )
        .map_err(ProviderError::Api)?;

        let task = revoke_permission_from_project_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recover project.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn recover_project(
        &self,
        args: &RecoverProjectArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recover_project_builder(
            &self.http_client,
            &args.project_id,
        )
        .map_err(ProviderError::Api)?;

        let task = recover_project_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Restore project.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn restore_project(
        &self,
        args: &RestoreProjectArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = restore_project_builder(
            &self.http_client,
            &args.project_id,
        )
        .map_err(ProviderError::Api)?;

        let task = restore_project_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Update snapshot.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn update_snapshot(
        &self,
        args: &UpdateSnapshotArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = update_snapshot_builder(
            &self.http_client,
            &args.project_id,
            &args.snapshot_id,
        )
        .map_err(ProviderError::Api)?;

        let task = update_snapshot_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete snapshot.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the OperationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_snapshot(
        &self,
        args: &DeleteSnapshotArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<OperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_snapshot_builder(
            &self.http_client,
            &args.project_id,
            &args.snapshot_id,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_snapshot_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Restore snapshot.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn restore_snapshot(
        &self,
        args: &RestoreSnapshotArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = restore_snapshot_builder(
            &self.http_client,
            &args.project_id,
            &args.snapshot_id,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = restore_snapshot_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Create project transfer request.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProjectTransferRequestResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn create_project_transfer_request(
        &self,
        args: &CreateProjectTransferRequestArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProjectTransferRequestResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = create_project_transfer_request_builder(
            &self.http_client,
            &args.project_id,
        )
        .map_err(ProviderError::Api)?;

        let task = create_project_transfer_request_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accept project transfer request.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the () result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn accept_project_transfer_request(
        &self,
        args: &AcceptProjectTransferRequestArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accept_project_transfer_request_builder(
            &self.http_client,
            &args.project_id,
            &args.request_id,
        )
        .map_err(ProviderError::Api)?;

        let task = accept_project_transfer_request_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Assign project v p c endpoint.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn assign_project_v_p_c_endpoint(
        &self,
        args: &AssignProjectVPCEndpointArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = assign_project_v_p_c_endpoint_builder(
            &self.http_client,
            &args.project_id,
            &args.vpc_endpoint_id,
        )
        .map_err(ProviderError::Api)?;

        let task = assign_project_v_p_c_endpoint_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete project v p c endpoint.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_project_v_p_c_endpoint(
        &self,
        args: &DeleteProjectVPCEndpointArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_project_v_p_c_endpoint_builder(
            &self.http_client,
            &args.project_id,
            &args.vpc_endpoint_id,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_project_v_p_c_endpoint_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Transfer projects from user to org.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EmptyResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn transfer_projects_from_user_to_org(
        &self,
        args: &TransferProjectsFromUserToOrgArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EmptyResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = transfer_projects_from_user_to_org_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = transfer_projects_from_user_to_org_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
