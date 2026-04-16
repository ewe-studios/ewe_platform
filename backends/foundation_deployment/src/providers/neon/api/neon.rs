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

use crate::providers::neon::clients::{
    list_api_keys_builder, list_api_keys_task,
    create_api_key_builder, create_api_key_task,
    revoke_api_key_builder, revoke_api_key_task,
    get_auth_details_builder, get_auth_details_task,
    get_consumption_history_per_account_builder, get_consumption_history_per_account_task,
    get_consumption_history_per_project_builder, get_consumption_history_per_project_task,
    get_consumption_history_per_project_v2_builder, get_consumption_history_per_project_v2_task,
    get_organization_builder, get_organization_task,
    list_org_api_keys_builder, list_org_api_keys_task,
    create_org_api_key_builder, create_org_api_key_task,
    revoke_org_api_key_builder, revoke_org_api_key_task,
    get_organization_invitations_builder, get_organization_invitations_task,
    create_organization_invitations_builder, create_organization_invitations_task,
    get_organization_members_builder, get_organization_members_task,
    get_organization_member_builder, get_organization_member_task,
    update_organization_member_builder, update_organization_member_task,
    remove_organization_member_builder, remove_organization_member_task,
    list_organization_v_p_c_endpoints_builder, list_organization_v_p_c_endpoints_task,
    get_organization_v_p_c_endpoint_details_builder, get_organization_v_p_c_endpoint_details_task,
    assign_organization_v_p_c_endpoint_builder, assign_organization_v_p_c_endpoint_task,
    delete_organization_v_p_c_endpoint_builder, delete_organization_v_p_c_endpoint_task,
    list_organization_v_p_c_endpoints_all_regions_builder, list_organization_v_p_c_endpoints_all_regions_task,
    transfer_projects_from_org_to_org_builder, transfer_projects_from_org_to_org_task,
    list_projects_builder, list_projects_task,
    create_project_builder, create_project_task,
    create_neon_auth_integration_builder, create_neon_auth_integration_task,
    create_neon_auth_provider_s_d_k_keys_builder, create_neon_auth_provider_s_d_k_keys_task,
    transfer_neon_auth_provider_project_builder, transfer_neon_auth_provider_project_task,
    create_neon_auth_new_user_builder, create_neon_auth_new_user_task,
    list_shared_projects_builder, list_shared_projects_task,
    get_project_builder, get_project_task,
    update_project_builder, update_project_task,
    delete_project_builder, delete_project_task,
    get_project_advisor_security_issues_builder, get_project_advisor_security_issues_task,
    list_neon_auth_redirect_u_r_i_whitelist_domains_builder, list_neon_auth_redirect_u_r_i_whitelist_domains_task,
    add_neon_auth_domain_to_redirect_u_r_i_whitelist_builder, add_neon_auth_domain_to_redirect_u_r_i_whitelist_task,
    delete_neon_auth_domain_from_redirect_u_r_i_whitelist_builder, delete_neon_auth_domain_from_redirect_u_r_i_whitelist_task,
    get_neon_auth_email_server_builder, get_neon_auth_email_server_task,
    update_neon_auth_email_server_builder, update_neon_auth_email_server_task,
    delete_neon_auth_integration_builder, delete_neon_auth_integration_task,
    list_neon_auth_integrations_builder, list_neon_auth_integrations_task,
    list_neon_auth_oauth_providers_builder, list_neon_auth_oauth_providers_task,
    add_neon_auth_oauth_provider_builder, add_neon_auth_oauth_provider_task,
    update_neon_auth_oauth_provider_builder, update_neon_auth_oauth_provider_task,
    delete_neon_auth_oauth_provider_builder, delete_neon_auth_oauth_provider_task,
    delete_neon_auth_user_builder, delete_neon_auth_user_task,
    get_available_preload_libraries_builder, get_available_preload_libraries_task,
    create_project_branch_anonymized_builder, create_project_branch_anonymized_task,
    list_project_branches_builder, list_project_branches_task,
    create_project_branch_builder, create_project_branch_task,
    count_project_branches_builder, count_project_branches_task,
    get_project_branch_builder, get_project_branch_task,
    update_project_branch_builder, update_project_branch_task,
    delete_project_branch_builder, delete_project_branch_task,
    start_anonymization_builder, start_anonymization_task,
    get_anonymized_branch_status_builder, get_anonymized_branch_status_task,
    get_neon_auth_builder, get_neon_auth_task,
    create_neon_auth_builder, create_neon_auth_task,
    disable_neon_auth_builder, disable_neon_auth_task,
    get_neon_auth_allow_localhost_builder, get_neon_auth_allow_localhost_task,
    update_neon_auth_allow_localhost_builder, update_neon_auth_allow_localhost_task,
    list_branch_neon_auth_trusted_domains_builder, list_branch_neon_auth_trusted_domains_task,
    add_branch_neon_auth_trusted_domain_builder, add_branch_neon_auth_trusted_domain_task,
    delete_branch_neon_auth_trusted_domain_builder, delete_branch_neon_auth_trusted_domain_task,
    get_neon_auth_email_and_password_config_builder, get_neon_auth_email_and_password_config_task,
    update_neon_auth_email_and_password_config_builder, update_neon_auth_email_and_password_config_task,
    get_neon_auth_email_provider_builder, get_neon_auth_email_provider_task,
    update_neon_auth_email_provider_builder, update_neon_auth_email_provider_task,
    list_branch_neon_auth_oauth_providers_builder, list_branch_neon_auth_oauth_providers_task,
    add_branch_neon_auth_oauth_provider_builder, add_branch_neon_auth_oauth_provider_task,
    update_branch_neon_auth_oauth_provider_builder, update_branch_neon_auth_oauth_provider_task,
    delete_branch_neon_auth_oauth_provider_builder, delete_branch_neon_auth_oauth_provider_task,
    get_neon_auth_plugin_configs_builder, get_neon_auth_plugin_configs_task,
    update_neon_auth_organization_plugin_builder, update_neon_auth_organization_plugin_task,
    send_neon_auth_test_email_builder, send_neon_auth_test_email_task,
    create_branch_neon_auth_new_user_builder, create_branch_neon_auth_new_user_task,
    delete_branch_neon_auth_user_builder, delete_branch_neon_auth_user_task,
    update_neon_auth_user_role_builder, update_neon_auth_user_role_task,
    get_neon_auth_webhook_config_builder, get_neon_auth_webhook_config_task,
    update_neon_auth_webhook_config_builder, update_neon_auth_webhook_config_task,
    get_snapshot_schedule_builder, get_snapshot_schedule_task,
    set_snapshot_schedule_builder, set_snapshot_schedule_task,
    get_project_branch_schema_comparison_builder, get_project_branch_schema_comparison_task,
    get_project_branch_data_a_p_i_builder, get_project_branch_data_a_p_i_task,
    create_project_branch_data_a_p_i_builder, create_project_branch_data_a_p_i_task,
    update_project_branch_data_a_p_i_builder, update_project_branch_data_a_p_i_task,
    delete_project_branch_data_a_p_i_builder, delete_project_branch_data_a_p_i_task,
    list_project_branch_databases_builder, list_project_branch_databases_task,
    create_project_branch_database_builder, create_project_branch_database_task,
    get_project_branch_database_builder, get_project_branch_database_task,
    update_project_branch_database_builder, update_project_branch_database_task,
    delete_project_branch_database_builder, delete_project_branch_database_task,
    list_project_branch_endpoints_builder, list_project_branch_endpoints_task,
    finalize_restore_branch_builder, finalize_restore_branch_task,
    get_masking_rules_builder, get_masking_rules_task,
    update_masking_rules_builder, update_masking_rules_task,
    restore_project_branch_builder, restore_project_branch_task,
    list_project_branch_roles_builder, list_project_branch_roles_task,
    create_project_branch_role_builder, create_project_branch_role_task,
    get_project_branch_role_builder, get_project_branch_role_task,
    delete_project_branch_role_builder, delete_project_branch_role_task,
    reset_project_branch_role_password_builder, reset_project_branch_role_password_task,
    get_project_branch_role_password_builder, get_project_branch_role_password_task,
    get_project_branch_schema_builder, get_project_branch_schema_task,
    set_default_project_branch_builder, set_default_project_branch_task,
    create_snapshot_builder, create_snapshot_task,
    get_connection_u_r_i_builder, get_connection_u_r_i_task,
    list_project_endpoints_builder, list_project_endpoints_task,
    create_project_endpoint_builder, create_project_endpoint_task,
    get_project_endpoint_builder, get_project_endpoint_task,
    update_project_endpoint_builder, update_project_endpoint_task,
    delete_project_endpoint_builder, delete_project_endpoint_task,
    restart_project_endpoint_builder, restart_project_endpoint_task,
    start_project_endpoint_builder, start_project_endpoint_task,
    suspend_project_endpoint_builder, suspend_project_endpoint_task,
    get_project_j_w_k_s_builder, get_project_j_w_k_s_task,
    add_project_j_w_k_s_builder, add_project_j_w_k_s_task,
    delete_project_j_w_k_s_builder, delete_project_j_w_k_s_task,
    list_project_operations_builder, list_project_operations_task,
    get_project_operation_builder, get_project_operation_task,
    list_project_permissions_builder, list_project_permissions_task,
    grant_permission_to_project_builder, grant_permission_to_project_task,
    revoke_permission_from_project_builder, revoke_permission_from_project_task,
    recover_project_builder, recover_project_task,
    restore_project_builder, restore_project_task,
    list_snapshots_builder, list_snapshots_task,
    update_snapshot_builder, update_snapshot_task,
    delete_snapshot_builder, delete_snapshot_task,
    restore_snapshot_builder, restore_snapshot_task,
    create_project_transfer_request_builder, create_project_transfer_request_task,
    accept_project_transfer_request_builder, accept_project_transfer_request_task,
    list_project_v_p_c_endpoints_builder, list_project_v_p_c_endpoints_task,
    assign_project_v_p_c_endpoint_builder, assign_project_v_p_c_endpoint_task,
    delete_project_v_p_c_endpoint_builder, delete_project_v_p_c_endpoint_task,
    get_active_regions_builder, get_active_regions_task,
    get_current_user_info_builder, get_current_user_info_task,
    get_current_user_organizations_builder, get_current_user_organizations_task,
    transfer_projects_from_user_to_org_builder, transfer_projects_from_user_to_org_task,
};
use crate::providers::neon::clients::types::{ApiError, ApiPending};
use crate::providers::neon::clients::ActiveRegionsResponse;
use crate::providers::neon::clients::AnonymizedBranchStatusResponse;
use crate::providers::neon::clients::ApiKeyCreateResponse;
use crate::providers::neon::clients::ApiKeyRevokeResponse;
use crate::providers::neon::clients::AuthDetailsResponse;
use crate::providers::neon::clients::AvailablePreloadLibraries;
use crate::providers::neon::clients::BranchOperations;
use crate::providers::neon::clients::BranchSchemaCompareResponse;
use crate::providers::neon::clients::BranchSchemaResponse;
use crate::providers::neon::clients::ConnectionURIResponse;
use crate::providers::neon::clients::ConsumptionHistoryPerAccountResponse;
use crate::providers::neon::clients::CurrentUserInfoResponse;
use crate::providers::neon::clients::DataAPICreateResponse;
use crate::providers::neon::clients::DataAPIReponse;
use crate::providers::neon::clients::DatabaseOperations;
use crate::providers::neon::clients::DatabaseResponse;
use crate::providers::neon::clients::DatabasesResponse;
use crate::providers::neon::clients::EmptyResponse;
use crate::providers::neon::clients::EndpointOperations;
use crate::providers::neon::clients::EndpointResponse;
use crate::providers::neon::clients::EndpointsResponse;
use crate::providers::neon::clients::JWKS;
use crate::providers::neon::clients::JWKSCreationOperation;
use crate::providers::neon::clients::ListNeonAuthIntegrationsResponse;
use crate::providers::neon::clients::ListNeonAuthOauthProvidersResponse;
use crate::providers::neon::clients::MaskingRulesResponse;
use crate::providers::neon::clients::Member;
use crate::providers::neon::clients::NeonAuthAllowLocalhostResponse;
use crate::providers::neon::clients::NeonAuthCreateIntegrationResponse;
use crate::providers::neon::clients::NeonAuthCreateNewUserResponse;
use crate::providers::neon::clients::NeonAuthEmailAndPasswordConfig;
use crate::providers::neon::clients::NeonAuthEmailServerConfig;
use crate::providers::neon::clients::NeonAuthIntegration;
use crate::providers::neon::clients::NeonAuthOauthProvider;
use crate::providers::neon::clients::NeonAuthOrganizationConfig;
use crate::providers::neon::clients::NeonAuthPluginConfigs;
use crate::providers::neon::clients::NeonAuthRedirectURIWhitelistResponse;
use crate::providers::neon::clients::NeonAuthTransferAuthProviderProjectResponse;
use crate::providers::neon::clients::NeonAuthWebhookConfig;
use crate::providers::neon::clients::OperationResponse;
use crate::providers::neon::clients::OperationsResponse;
use crate::providers::neon::clients::OrgApiKeyCreateResponse;
use crate::providers::neon::clients::OrgApiKeyRevokeResponse;
use crate::providers::neon::clients::Organization;
use crate::providers::neon::clients::OrganizationInvitationsResponse;
use crate::providers::neon::clients::OrganizationsResponse;
use crate::providers::neon::clients::ProjectJWKSResponse;
use crate::providers::neon::clients::ProjectPermission;
use crate::providers::neon::clients::ProjectPermissions;
use crate::providers::neon::clients::ProjectRecoverResponse;
use crate::providers::neon::clients::ProjectResponse;
use crate::providers::neon::clients::ProjectTransferRequestResponse;
use crate::providers::neon::clients::RoleOperations;
use crate::providers::neon::clients::RolePasswordResponse;
use crate::providers::neon::clients::RoleResponse;
use crate::providers::neon::clients::RolesResponse;
use crate::providers::neon::clients::SendNeonAuthTestEmailResponse;
use crate::providers::neon::clients::UpdateNeonAuthUserRoleResponse;
use crate::providers::neon::clients::VPCEndpointDetails;
use crate::providers::neon::clients::VPCEndpointsResponse;
use crate::providers::neon::clients::VPCEndpointsWithRegionResponse;
use crate::providers::neon::clients::AcceptProjectTransferRequestArgs;
use crate::providers::neon::clients::AddBranchNeonAuthOauthProviderArgs;
use crate::providers::neon::clients::AddBranchNeonAuthTrustedDomainArgs;
use crate::providers::neon::clients::AddNeonAuthDomainToRedirectURIWhitelistArgs;
use crate::providers::neon::clients::AddNeonAuthOauthProviderArgs;
use crate::providers::neon::clients::AddProjectJWKSArgs;
use crate::providers::neon::clients::AssignOrganizationVPCEndpointArgs;
use crate::providers::neon::clients::AssignProjectVPCEndpointArgs;
use crate::providers::neon::clients::CountProjectBranchesArgs;
use crate::providers::neon::clients::CreateApiKeyArgs;
use crate::providers::neon::clients::CreateBranchNeonAuthNewUserArgs;
use crate::providers::neon::clients::CreateNeonAuthArgs;
use crate::providers::neon::clients::CreateNeonAuthIntegrationArgs;
use crate::providers::neon::clients::CreateNeonAuthNewUserArgs;
use crate::providers::neon::clients::CreateNeonAuthProviderSDKKeysArgs;
use crate::providers::neon::clients::CreateOrgApiKeyArgs;
use crate::providers::neon::clients::CreateOrganizationInvitationsArgs;
use crate::providers::neon::clients::CreateProjectArgs;
use crate::providers::neon::clients::CreateProjectBranchAnonymizedArgs;
use crate::providers::neon::clients::CreateProjectBranchArgs;
use crate::providers::neon::clients::CreateProjectBranchDataAPIArgs;
use crate::providers::neon::clients::CreateProjectBranchDatabaseArgs;
use crate::providers::neon::clients::CreateProjectBranchRoleArgs;
use crate::providers::neon::clients::CreateProjectEndpointArgs;
use crate::providers::neon::clients::CreateProjectTransferRequestArgs;
use crate::providers::neon::clients::CreateSnapshotArgs;
use crate::providers::neon::clients::DeleteBranchNeonAuthOauthProviderArgs;
use crate::providers::neon::clients::DeleteBranchNeonAuthTrustedDomainArgs;
use crate::providers::neon::clients::DeleteBranchNeonAuthUserArgs;
use crate::providers::neon::clients::DeleteNeonAuthDomainFromRedirectURIWhitelistArgs;
use crate::providers::neon::clients::DeleteNeonAuthIntegrationArgs;
use crate::providers::neon::clients::DeleteNeonAuthOauthProviderArgs;
use crate::providers::neon::clients::DeleteNeonAuthUserArgs;
use crate::providers::neon::clients::DeleteOrganizationVPCEndpointArgs;
use crate::providers::neon::clients::DeleteProjectArgs;
use crate::providers::neon::clients::DeleteProjectBranchArgs;
use crate::providers::neon::clients::DeleteProjectBranchDataAPIArgs;
use crate::providers::neon::clients::DeleteProjectBranchDatabaseArgs;
use crate::providers::neon::clients::DeleteProjectBranchRoleArgs;
use crate::providers::neon::clients::DeleteProjectEndpointArgs;
use crate::providers::neon::clients::DeleteProjectJWKSArgs;
use crate::providers::neon::clients::DeleteProjectVPCEndpointArgs;
use crate::providers::neon::clients::DeleteSnapshotArgs;
use crate::providers::neon::clients::DisableNeonAuthArgs;
use crate::providers::neon::clients::FinalizeRestoreBranchArgs;
use crate::providers::neon::clients::GetActiveRegionsArgs;
use crate::providers::neon::clients::GetAnonymizedBranchStatusArgs;
use crate::providers::neon::clients::GetAvailablePreloadLibrariesArgs;
use crate::providers::neon::clients::GetConnectionURIArgs;
use crate::providers::neon::clients::GetConsumptionHistoryPerAccountArgs;
use crate::providers::neon::clients::GetConsumptionHistoryPerProjectArgs;
use crate::providers::neon::clients::GetConsumptionHistoryPerProjectV2Args;
use crate::providers::neon::clients::GetMaskingRulesArgs;
use crate::providers::neon::clients::GetNeonAuthAllowLocalhostArgs;
use crate::providers::neon::clients::GetNeonAuthArgs;
use crate::providers::neon::clients::GetNeonAuthEmailAndPasswordConfigArgs;
use crate::providers::neon::clients::GetNeonAuthEmailProviderArgs;
use crate::providers::neon::clients::GetNeonAuthEmailServerArgs;
use crate::providers::neon::clients::GetNeonAuthPluginConfigsArgs;
use crate::providers::neon::clients::GetNeonAuthWebhookConfigArgs;
use crate::providers::neon::clients::GetOrganizationArgs;
use crate::providers::neon::clients::GetOrganizationInvitationsArgs;
use crate::providers::neon::clients::GetOrganizationMemberArgs;
use crate::providers::neon::clients::GetOrganizationMembersArgs;
use crate::providers::neon::clients::GetOrganizationVPCEndpointDetailsArgs;
use crate::providers::neon::clients::GetProjectAdvisorSecurityIssuesArgs;
use crate::providers::neon::clients::GetProjectArgs;
use crate::providers::neon::clients::GetProjectBranchArgs;
use crate::providers::neon::clients::GetProjectBranchDataAPIArgs;
use crate::providers::neon::clients::GetProjectBranchDatabaseArgs;
use crate::providers::neon::clients::GetProjectBranchRoleArgs;
use crate::providers::neon::clients::GetProjectBranchRolePasswordArgs;
use crate::providers::neon::clients::GetProjectBranchSchemaArgs;
use crate::providers::neon::clients::GetProjectBranchSchemaComparisonArgs;
use crate::providers::neon::clients::GetProjectEndpointArgs;
use crate::providers::neon::clients::GetProjectJWKSArgs;
use crate::providers::neon::clients::GetProjectOperationArgs;
use crate::providers::neon::clients::GetSnapshotScheduleArgs;
use crate::providers::neon::clients::GrantPermissionToProjectArgs;
use crate::providers::neon::clients::ListBranchNeonAuthOauthProvidersArgs;
use crate::providers::neon::clients::ListBranchNeonAuthTrustedDomainsArgs;
use crate::providers::neon::clients::ListNeonAuthIntegrationsArgs;
use crate::providers::neon::clients::ListNeonAuthOauthProvidersArgs;
use crate::providers::neon::clients::ListNeonAuthRedirectURIWhitelistDomainsArgs;
use crate::providers::neon::clients::ListOrgApiKeysArgs;
use crate::providers::neon::clients::ListOrganizationVPCEndpointsAllRegionsArgs;
use crate::providers::neon::clients::ListOrganizationVPCEndpointsArgs;
use crate::providers::neon::clients::ListProjectBranchDatabasesArgs;
use crate::providers::neon::clients::ListProjectBranchEndpointsArgs;
use crate::providers::neon::clients::ListProjectBranchRolesArgs;
use crate::providers::neon::clients::ListProjectBranchesArgs;
use crate::providers::neon::clients::ListProjectEndpointsArgs;
use crate::providers::neon::clients::ListProjectOperationsArgs;
use crate::providers::neon::clients::ListProjectPermissionsArgs;
use crate::providers::neon::clients::ListProjectVPCEndpointsArgs;
use crate::providers::neon::clients::ListProjectsArgs;
use crate::providers::neon::clients::ListSharedProjectsArgs;
use crate::providers::neon::clients::ListSnapshotsArgs;
use crate::providers::neon::clients::RecoverProjectArgs;
use crate::providers::neon::clients::RemoveOrganizationMemberArgs;
use crate::providers::neon::clients::ResetProjectBranchRolePasswordArgs;
use crate::providers::neon::clients::RestartProjectEndpointArgs;
use crate::providers::neon::clients::RestoreProjectArgs;
use crate::providers::neon::clients::RestoreProjectBranchArgs;
use crate::providers::neon::clients::RestoreSnapshotArgs;
use crate::providers::neon::clients::RevokeApiKeyArgs;
use crate::providers::neon::clients::RevokeOrgApiKeyArgs;
use crate::providers::neon::clients::RevokePermissionFromProjectArgs;
use crate::providers::neon::clients::SendNeonAuthTestEmailArgs;
use crate::providers::neon::clients::SetDefaultProjectBranchArgs;
use crate::providers::neon::clients::SetSnapshotScheduleArgs;
use crate::providers::neon::clients::StartAnonymizationArgs;
use crate::providers::neon::clients::StartProjectEndpointArgs;
use crate::providers::neon::clients::SuspendProjectEndpointArgs;
use crate::providers::neon::clients::TransferNeonAuthProviderProjectArgs;
use crate::providers::neon::clients::TransferProjectsFromOrgToOrgArgs;
use crate::providers::neon::clients::TransferProjectsFromUserToOrgArgs;
use crate::providers::neon::clients::UpdateBranchNeonAuthOauthProviderArgs;
use crate::providers::neon::clients::UpdateMaskingRulesArgs;
use crate::providers::neon::clients::UpdateNeonAuthAllowLocalhostArgs;
use crate::providers::neon::clients::UpdateNeonAuthEmailAndPasswordConfigArgs;
use crate::providers::neon::clients::UpdateNeonAuthEmailProviderArgs;
use crate::providers::neon::clients::UpdateNeonAuthEmailServerArgs;
use crate::providers::neon::clients::UpdateNeonAuthOauthProviderArgs;
use crate::providers::neon::clients::UpdateNeonAuthOrganizationPluginArgs;
use crate::providers::neon::clients::UpdateNeonAuthUserRoleArgs;
use crate::providers::neon::clients::UpdateNeonAuthWebhookConfigArgs;
use crate::providers::neon::clients::UpdateOrganizationMemberArgs;
use crate::providers::neon::clients::UpdateProjectArgs;
use crate::providers::neon::clients::UpdateProjectBranchArgs;
use crate::providers::neon::clients::UpdateProjectBranchDataAPIArgs;
use crate::providers::neon::clients::UpdateProjectBranchDatabaseArgs;
use crate::providers::neon::clients::UpdateProjectEndpointArgs;
use crate::providers::neon::clients::UpdateSnapshotArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// NeonProvider with automatic state tracking.
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
/// let provider = NeonProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct NeonProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> NeonProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new NeonProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new NeonProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// List api keys.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn list_api_keys(
        &self,
        args: &ListApiKeysArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_api_keys_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = list_api_keys_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get auth details.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AuthDetailsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_auth_details(
        &self,
        args: &GetAuthDetailsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AuthDetailsResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_auth_details_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = get_auth_details_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get consumption history per account.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ConsumptionHistoryPerAccountResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_consumption_history_per_account(
        &self,
        args: &GetConsumptionHistoryPerAccountArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ConsumptionHistoryPerAccountResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_consumption_history_per_account_builder(
            &self.http_client,
            &args.from,
            &args.to,
            &args.granularity,
            &args.org_id,
            &args.include_v1_metrics,
            &args.metrics,
        )
        .map_err(ProviderError::Api)?;

        let task = get_consumption_history_per_account_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get consumption history per project.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_consumption_history_per_project(
        &self,
        args: &GetConsumptionHistoryPerProjectArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_consumption_history_per_project_builder(
            &self.http_client,
            &args.cursor,
            &args.limit,
            &args.project_ids,
            &args.from,
            &args.to,
            &args.granularity,
            &args.org_id,
            &args.include_v1_metrics,
            &args.metrics,
        )
        .map_err(ProviderError::Api)?;

        let task = get_consumption_history_per_project_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get consumption history per project v2.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_consumption_history_per_project_v2(
        &self,
        args: &GetConsumptionHistoryPerProjectV2Args,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_consumption_history_per_project_v2_builder(
            &self.http_client,
            &args.cursor,
            &args.limit,
            &args.project_ids,
            &args.from,
            &args.to,
            &args.granularity,
            &args.org_id,
            &args.metrics,
        )
        .map_err(ProviderError::Api)?;

        let task = get_consumption_history_per_project_v2_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get organization.
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
    pub fn get_organization(
        &self,
        args: &GetOrganizationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Organization, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_organization_builder(
            &self.http_client,
            &args.org_id,
        )
        .map_err(ProviderError::Api)?;

        let task = get_organization_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// List org api keys.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn list_org_api_keys(
        &self,
        args: &ListOrgApiKeysArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_org_api_keys_builder(
            &self.http_client,
            &args.org_id,
        )
        .map_err(ProviderError::Api)?;

        let task = list_org_api_keys_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// StreamIterator yielding the OrgApiKeyCreateResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn create_org_api_key(
        &self,
        args: &CreateOrgApiKeyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<OrgApiKeyCreateResponse, ProviderError<ApiError>>,
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
    /// StreamIterator yielding the OrgApiKeyRevokeResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn revoke_org_api_key(
        &self,
        args: &RevokeOrgApiKeyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<OrgApiKeyRevokeResponse, ProviderError<ApiError>>,
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

    /// Get organization invitations.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_organization_invitations(
        &self,
        args: &GetOrganizationInvitationsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<OrganizationInvitationsResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_organization_invitations_builder(
            &self.http_client,
            &args.org_id,
        )
        .map_err(ProviderError::Api)?;

        let task = get_organization_invitations_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get organization members.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_organization_members(
        &self,
        args: &GetOrganizationMembersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_organization_members_builder(
            &self.http_client,
            &args.org_id,
            &args.sort_by,
            &args.limit,
        )
        .map_err(ProviderError::Api)?;

        let task = get_organization_members_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get organization member.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_organization_member(
        &self,
        args: &GetOrganizationMemberArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Member, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_organization_member_builder(
            &self.http_client,
            &args.org_id,
            &args.member_id,
        )
        .map_err(ProviderError::Api)?;

        let task = get_organization_member_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// List organization v p c endpoints.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the VPCEndpointsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn list_organization_v_p_c_endpoints(
        &self,
        args: &ListOrganizationVPCEndpointsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<VPCEndpointsResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_organization_v_p_c_endpoints_builder(
            &self.http_client,
            &args.org_id,
            &args.region_id,
        )
        .map_err(ProviderError::Api)?;

        let task = list_organization_v_p_c_endpoints_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get organization v p c endpoint details.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the VPCEndpointDetails result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_organization_v_p_c_endpoint_details(
        &self,
        args: &GetOrganizationVPCEndpointDetailsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<VPCEndpointDetails, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_organization_v_p_c_endpoint_details_builder(
            &self.http_client,
            &args.org_id,
            &args.region_id,
            &args.vpc_endpoint_id,
        )
        .map_err(ProviderError::Api)?;

        let task = get_organization_v_p_c_endpoint_details_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// List organization v p c endpoints all regions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the VPCEndpointsWithRegionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn list_organization_v_p_c_endpoints_all_regions(
        &self,
        args: &ListOrganizationVPCEndpointsAllRegionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<VPCEndpointsWithRegionResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_organization_v_p_c_endpoints_all_regions_builder(
            &self.http_client,
            &args.org_id,
        )
        .map_err(ProviderError::Api)?;

        let task = list_organization_v_p_c_endpoints_all_regions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// List projects.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn list_projects(
        &self,
        args: &ListProjectsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_projects_builder(
            &self.http_client,
            &args.cursor,
            &args.limit,
            &args.search,
            &args.org_id,
            &args.recoverable,
        )
        .map_err(ProviderError::Api)?;

        let task = list_projects_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// List shared projects.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn list_shared_projects(
        &self,
        args: &ListSharedProjectsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_shared_projects_builder(
            &self.http_client,
            &args.cursor,
            &args.limit,
            &args.search,
        )
        .map_err(ProviderError::Api)?;

        let task = list_shared_projects_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get project.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_project(
        &self,
        args: &GetProjectArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProjectResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_project_builder(
            &self.http_client,
            &args.project_id,
        )
        .map_err(ProviderError::Api)?;

        let task = get_project_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get project advisor security issues.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_project_advisor_security_issues(
        &self,
        args: &GetProjectAdvisorSecurityIssuesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_project_advisor_security_issues_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
            &args.database_name,
            &args.category,
            &args.min_severity,
        )
        .map_err(ProviderError::Api)?;

        let task = get_project_advisor_security_issues_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// List neon auth redirect u r i whitelist domains.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NeonAuthRedirectURIWhitelistResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn list_neon_auth_redirect_u_r_i_whitelist_domains(
        &self,
        args: &ListNeonAuthRedirectURIWhitelistDomainsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NeonAuthRedirectURIWhitelistResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_neon_auth_redirect_u_r_i_whitelist_domains_builder(
            &self.http_client,
            &args.project_id,
        )
        .map_err(ProviderError::Api)?;

        let task = list_neon_auth_redirect_u_r_i_whitelist_domains_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get neon auth email server.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NeonAuthEmailServerConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_neon_auth_email_server(
        &self,
        args: &GetNeonAuthEmailServerArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NeonAuthEmailServerConfig, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_neon_auth_email_server_builder(
            &self.http_client,
            &args.project_id,
        )
        .map_err(ProviderError::Api)?;

        let task = get_neon_auth_email_server_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// StreamIterator yielding the NeonAuthEmailServerConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn update_neon_auth_email_server(
        &self,
        args: &UpdateNeonAuthEmailServerArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NeonAuthEmailServerConfig, ProviderError<ApiError>>,
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

    /// List neon auth integrations.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListNeonAuthIntegrationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn list_neon_auth_integrations(
        &self,
        args: &ListNeonAuthIntegrationsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListNeonAuthIntegrationsResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_neon_auth_integrations_builder(
            &self.http_client,
            &args.project_id,
        )
        .map_err(ProviderError::Api)?;

        let task = list_neon_auth_integrations_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// List neon auth oauth providers.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListNeonAuthOauthProvidersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn list_neon_auth_oauth_providers(
        &self,
        args: &ListNeonAuthOauthProvidersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListNeonAuthOauthProvidersResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_neon_auth_oauth_providers_builder(
            &self.http_client,
            &args.project_id,
        )
        .map_err(ProviderError::Api)?;

        let task = list_neon_auth_oauth_providers_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get available preload libraries.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AvailablePreloadLibraries result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_available_preload_libraries(
        &self,
        args: &GetAvailablePreloadLibrariesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AvailablePreloadLibraries, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_available_preload_libraries_builder(
            &self.http_client,
            &args.project_id,
        )
        .map_err(ProviderError::Api)?;

        let task = get_available_preload_libraries_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// List project branches.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn list_project_branches(
        &self,
        args: &ListProjectBranchesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_project_branches_builder(
            &self.http_client,
            &args.project_id,
            &args.search,
            &args.sort_by,
        )
        .map_err(ProviderError::Api)?;

        let task = list_project_branches_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Count project branches.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn count_project_branches(
        &self,
        args: &CountProjectBranchesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = count_project_branches_builder(
            &self.http_client,
            &args.project_id,
            &args.search,
        )
        .map_err(ProviderError::Api)?;

        let task = count_project_branches_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get project branch.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_project_branch(
        &self,
        args: &GetProjectBranchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_project_branch_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
        )
        .map_err(ProviderError::Api)?;

        let task = get_project_branch_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// StreamIterator yielding the BranchOperations result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn update_project_branch(
        &self,
        args: &UpdateProjectBranchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BranchOperations, ProviderError<ApiError>>,
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
    /// StreamIterator yielding the BranchOperations result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_project_branch(
        &self,
        args: &DeleteProjectBranchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BranchOperations, ProviderError<ApiError>>,
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

    /// Get anonymized branch status.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_anonymized_branch_status(
        &self,
        args: &GetAnonymizedBranchStatusArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AnonymizedBranchStatusResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_anonymized_branch_status_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
        )
        .map_err(ProviderError::Api)?;

        let task = get_anonymized_branch_status_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get neon auth.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NeonAuthIntegration result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_neon_auth(
        &self,
        args: &GetNeonAuthArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NeonAuthIntegration, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_neon_auth_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
        )
        .map_err(ProviderError::Api)?;

        let task = get_neon_auth_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get neon auth allow localhost.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_neon_auth_allow_localhost(
        &self,
        args: &GetNeonAuthAllowLocalhostArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NeonAuthAllowLocalhostResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_neon_auth_allow_localhost_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
        )
        .map_err(ProviderError::Api)?;

        let task = get_neon_auth_allow_localhost_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// List branch neon auth trusted domains.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NeonAuthRedirectURIWhitelistResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn list_branch_neon_auth_trusted_domains(
        &self,
        args: &ListBranchNeonAuthTrustedDomainsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NeonAuthRedirectURIWhitelistResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_branch_neon_auth_trusted_domains_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
        )
        .map_err(ProviderError::Api)?;

        let task = list_branch_neon_auth_trusted_domains_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get neon auth email and password config.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_neon_auth_email_and_password_config(
        &self,
        args: &GetNeonAuthEmailAndPasswordConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NeonAuthEmailAndPasswordConfig, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_neon_auth_email_and_password_config_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
        )
        .map_err(ProviderError::Api)?;

        let task = get_neon_auth_email_and_password_config_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get neon auth email provider.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NeonAuthEmailServerConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_neon_auth_email_provider(
        &self,
        args: &GetNeonAuthEmailProviderArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NeonAuthEmailServerConfig, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_neon_auth_email_provider_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
        )
        .map_err(ProviderError::Api)?;

        let task = get_neon_auth_email_provider_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// StreamIterator yielding the NeonAuthEmailServerConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn update_neon_auth_email_provider(
        &self,
        args: &UpdateNeonAuthEmailProviderArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NeonAuthEmailServerConfig, ProviderError<ApiError>>,
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

    /// List branch neon auth oauth providers.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListNeonAuthOauthProvidersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn list_branch_neon_auth_oauth_providers(
        &self,
        args: &ListBranchNeonAuthOauthProvidersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListNeonAuthOauthProvidersResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_branch_neon_auth_oauth_providers_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
        )
        .map_err(ProviderError::Api)?;

        let task = list_branch_neon_auth_oauth_providers_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get neon auth plugin configs.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NeonAuthPluginConfigs result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_neon_auth_plugin_configs(
        &self,
        args: &GetNeonAuthPluginConfigsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NeonAuthPluginConfigs, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_neon_auth_plugin_configs_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
        )
        .map_err(ProviderError::Api)?;

        let task = get_neon_auth_plugin_configs_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get neon auth webhook config.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_neon_auth_webhook_config(
        &self,
        args: &GetNeonAuthWebhookConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NeonAuthWebhookConfig, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_neon_auth_webhook_config_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
        )
        .map_err(ProviderError::Api)?;

        let task = get_neon_auth_webhook_config_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get snapshot schedule.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_snapshot_schedule(
        &self,
        args: &GetSnapshotScheduleArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_snapshot_schedule_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
        )
        .map_err(ProviderError::Api)?;

        let task = get_snapshot_schedule_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get project branch schema comparison.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BranchSchemaCompareResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_project_branch_schema_comparison(
        &self,
        args: &GetProjectBranchSchemaComparisonArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BranchSchemaCompareResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_project_branch_schema_comparison_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
            &args.base_branch_id,
            &args.db_name,
            &args.lsn,
            &args.timestamp,
            &args.base_lsn,
            &args.base_timestamp,
        )
        .map_err(ProviderError::Api)?;

        let task = get_project_branch_schema_comparison_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get project branch data a p i.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DataAPIReponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_project_branch_data_a_p_i(
        &self,
        args: &GetProjectBranchDataAPIArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DataAPIReponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_project_branch_data_a_p_i_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
            &args.database_name,
        )
        .map_err(ProviderError::Api)?;

        let task = get_project_branch_data_a_p_i_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// List project branch databases.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DatabasesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn list_project_branch_databases(
        &self,
        args: &ListProjectBranchDatabasesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DatabasesResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_project_branch_databases_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
        )
        .map_err(ProviderError::Api)?;

        let task = list_project_branch_databases_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// StreamIterator yielding the DatabaseOperations result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn create_project_branch_database(
        &self,
        args: &CreateProjectBranchDatabaseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DatabaseOperations, ProviderError<ApiError>>,
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

    /// Get project branch database.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DatabaseResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_project_branch_database(
        &self,
        args: &GetProjectBranchDatabaseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DatabaseResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_project_branch_database_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
            &args.database_name,
        )
        .map_err(ProviderError::Api)?;

        let task = get_project_branch_database_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// StreamIterator yielding the DatabaseOperations result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn update_project_branch_database(
        &self,
        args: &UpdateProjectBranchDatabaseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DatabaseOperations, ProviderError<ApiError>>,
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
    /// StreamIterator yielding the DatabaseOperations result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_project_branch_database(
        &self,
        args: &DeleteProjectBranchDatabaseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DatabaseOperations, ProviderError<ApiError>>,
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

    /// List project branch endpoints.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EndpointsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn list_project_branch_endpoints(
        &self,
        args: &ListProjectBranchEndpointsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EndpointsResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_project_branch_endpoints_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
        )
        .map_err(ProviderError::Api)?;

        let task = list_project_branch_endpoints_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get masking rules.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn get_masking_rules(
        &self,
        args: &GetMaskingRulesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<MaskingRulesResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_masking_rules_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
        )
        .map_err(ProviderError::Api)?;

        let task = get_masking_rules_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// StreamIterator yielding the BranchOperations result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn restore_project_branch(
        &self,
        args: &RestoreProjectBranchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BranchOperations, ProviderError<ApiError>>,
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

    /// List project branch roles.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RolesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn list_project_branch_roles(
        &self,
        args: &ListProjectBranchRolesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RolesResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_project_branch_roles_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
        )
        .map_err(ProviderError::Api)?;

        let task = list_project_branch_roles_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// StreamIterator yielding the RoleOperations result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn create_project_branch_role(
        &self,
        args: &CreateProjectBranchRoleArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RoleOperations, ProviderError<ApiError>>,
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

    /// Get project branch role.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RoleResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_project_branch_role(
        &self,
        args: &GetProjectBranchRoleArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RoleResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_project_branch_role_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
            &args.role_name,
        )
        .map_err(ProviderError::Api)?;

        let task = get_project_branch_role_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// StreamIterator yielding the RoleOperations result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_project_branch_role(
        &self,
        args: &DeleteProjectBranchRoleArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RoleOperations, ProviderError<ApiError>>,
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
    /// StreamIterator yielding the RoleOperations result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn reset_project_branch_role_password(
        &self,
        args: &ResetProjectBranchRolePasswordArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RoleOperations, ProviderError<ApiError>>,
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

    /// Get project branch role password.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RolePasswordResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_project_branch_role_password(
        &self,
        args: &GetProjectBranchRolePasswordArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RolePasswordResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_project_branch_role_password_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
            &args.role_name,
        )
        .map_err(ProviderError::Api)?;

        let task = get_project_branch_role_password_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get project branch schema.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BranchSchemaResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_project_branch_schema(
        &self,
        args: &GetProjectBranchSchemaArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BranchSchemaResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_project_branch_schema_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
            &args.db_name,
            &args.lsn,
            &args.timestamp,
            &args.format,
        )
        .map_err(ProviderError::Api)?;

        let task = get_project_branch_schema_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// StreamIterator yielding the BranchOperations result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn set_default_project_branch(
        &self,
        args: &SetDefaultProjectBranchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BranchOperations, ProviderError<ApiError>>,
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

    /// Get connection u r i.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ConnectionURIResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_connection_u_r_i(
        &self,
        args: &GetConnectionURIArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ConnectionURIResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_connection_u_r_i_builder(
            &self.http_client,
            &args.project_id,
            &args.branch_id,
            &args.endpoint_id,
            &args.database_name,
            &args.role_name,
            &args.pooled,
        )
        .map_err(ProviderError::Api)?;

        let task = get_connection_u_r_i_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// List project endpoints.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EndpointsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn list_project_endpoints(
        &self,
        args: &ListProjectEndpointsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EndpointsResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_project_endpoints_builder(
            &self.http_client,
            &args.project_id,
        )
        .map_err(ProviderError::Api)?;

        let task = list_project_endpoints_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// StreamIterator yielding the EndpointOperations result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn create_project_endpoint(
        &self,
        args: &CreateProjectEndpointArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EndpointOperations, ProviderError<ApiError>>,
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

    /// Get project endpoint.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EndpointResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_project_endpoint(
        &self,
        args: &GetProjectEndpointArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EndpointResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_project_endpoint_builder(
            &self.http_client,
            &args.project_id,
            &args.endpoint_id,
        )
        .map_err(ProviderError::Api)?;

        let task = get_project_endpoint_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// StreamIterator yielding the EndpointOperations result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn update_project_endpoint(
        &self,
        args: &UpdateProjectEndpointArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EndpointOperations, ProviderError<ApiError>>,
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
    /// StreamIterator yielding the EndpointOperations result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_project_endpoint(
        &self,
        args: &DeleteProjectEndpointArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EndpointOperations, ProviderError<ApiError>>,
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
    /// StreamIterator yielding the EndpointOperations result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn restart_project_endpoint(
        &self,
        args: &RestartProjectEndpointArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EndpointOperations, ProviderError<ApiError>>,
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
    /// StreamIterator yielding the EndpointOperations result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn start_project_endpoint(
        &self,
        args: &StartProjectEndpointArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EndpointOperations, ProviderError<ApiError>>,
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
    /// StreamIterator yielding the EndpointOperations result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn suspend_project_endpoint(
        &self,
        args: &SuspendProjectEndpointArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EndpointOperations, ProviderError<ApiError>>,
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

    /// Get project j w k s.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProjectJWKSResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_project_j_w_k_s(
        &self,
        args: &GetProjectJWKSArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProjectJWKSResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_project_j_w_k_s_builder(
            &self.http_client,
            &args.project_id,
        )
        .map_err(ProviderError::Api)?;

        let task = get_project_j_w_k_s_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// StreamIterator yielding the JWKSCreationOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn add_project_j_w_k_s(
        &self,
        args: &AddProjectJWKSArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<JWKSCreationOperation, ProviderError<ApiError>>,
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

    /// List project operations.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn list_project_operations(
        &self,
        args: &ListProjectOperationsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_project_operations_builder(
            &self.http_client,
            &args.project_id,
            &args.cursor,
            &args.limit,
        )
        .map_err(ProviderError::Api)?;

        let task = list_project_operations_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get project operation.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the OperationResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_project_operation(
        &self,
        args: &GetProjectOperationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<OperationResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_project_operation_builder(
            &self.http_client,
            &args.project_id,
            &args.operation_id,
        )
        .map_err(ProviderError::Api)?;

        let task = get_project_operation_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// List project permissions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProjectPermissions result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn list_project_permissions(
        &self,
        args: &ListProjectPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProjectPermissions, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_project_permissions_builder(
            &self.http_client,
            &args.project_id,
        )
        .map_err(ProviderError::Api)?;

        let task = list_project_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// StreamIterator yielding the ProjectRecoverResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn recover_project(
        &self,
        args: &RecoverProjectArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProjectRecoverResponse, ProviderError<ApiError>>,
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
    /// StreamIterator yielding the ProjectRecoverResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn restore_project(
        &self,
        args: &RestoreProjectArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProjectRecoverResponse, ProviderError<ApiError>>,
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

    /// List snapshots.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn list_snapshots(
        &self,
        args: &ListSnapshotsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_snapshots_builder(
            &self.http_client,
            &args.project_id,
        )
        .map_err(ProviderError::Api)?;

        let task = list_snapshots_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// List project v p c endpoints.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the VPCEndpointsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn list_project_v_p_c_endpoints(
        &self,
        args: &ListProjectVPCEndpointsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<VPCEndpointsResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_project_v_p_c_endpoints_builder(
            &self.http_client,
            &args.project_id,
        )
        .map_err(ProviderError::Api)?;

        let task = list_project_v_p_c_endpoints_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get active regions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ActiveRegionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_active_regions(
        &self,
        args: &GetActiveRegionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ActiveRegionsResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_active_regions_builder(
            &self.http_client,
            &args.org_id,
        )
        .map_err(ProviderError::Api)?;

        let task = get_active_regions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get current user info.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CurrentUserInfoResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_current_user_info(
        &self,
        args: &GetCurrentUserInfoArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CurrentUserInfoResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_current_user_info_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = get_current_user_info_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get current user organizations.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the OrganizationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_current_user_organizations(
        &self,
        args: &GetCurrentUserOrganizationsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<OrganizationsResponse, ProviderError<ApiError>>,
            P = crate::providers::neon::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_current_user_organizations_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = get_current_user_organizations_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
