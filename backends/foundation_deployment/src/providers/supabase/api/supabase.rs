//! SupabaseProvider - State-aware supabase API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       supabase API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "supabase")]

use crate::providers::supabase::clients::supabase::{
    v1_update_a_branch_config_builder, v1_update_a_branch_config_task,
    v1_delete_a_branch_builder, v1_delete_a_branch_task,
    v1_merge_a_branch_builder, v1_merge_a_branch_task,
    v1_push_a_branch_builder, v1_push_a_branch_task,
    v1_reset_a_branch_builder, v1_reset_a_branch_task,
    v1_restore_a_branch_builder, v1_restore_a_branch_task,
    v1_revoke_token_builder, v1_revoke_token_task,
    v1_exchange_oauth_token_builder, v1_exchange_oauth_token_task,
    v1_create_an_organization_builder, v1_create_an_organization_task,
    v1_claim_project_for_organization_builder, v1_claim_project_for_organization_task,
    v1_create_a_project_builder, v1_create_a_project_task,
    v1_update_a_project_builder, v1_update_a_project_task,
    v1_delete_a_project_builder, v1_delete_a_project_task,
    v1_update_action_run_status_builder, v1_update_action_run_status_task,
    v1_create_project_api_key_builder, v1_create_project_api_key_task,
    v1_update_project_legacy_api_keys_builder, v1_update_project_legacy_api_keys_task,
    v1_update_project_api_key_builder, v1_update_project_api_key_task,
    v1_delete_project_api_key_builder, v1_delete_project_api_key_task,
    v1_apply_project_addon_builder, v1_apply_project_addon_task,
    v1_remove_project_addon_builder, v1_remove_project_addon_task,
    v1_create_a_branch_builder, v1_create_a_branch_task,
    v1_disable_preview_branching_builder, v1_disable_preview_branching_task,
    v1_create_project_claim_token_builder, v1_create_project_claim_token_task,
    v1_delete_project_claim_token_builder, v1_delete_project_claim_token_task,
    v1_create_login_role_builder, v1_create_login_role_task,
    v1_delete_login_roles_builder, v1_delete_login_roles_task,
    v1_update_auth_service_config_builder, v1_update_auth_service_config_task,
    v1_create_project_signing_key_builder, v1_create_project_signing_key_task,
    v1_create_legacy_signing_key_builder, v1_create_legacy_signing_key_task,
    v1_update_project_signing_key_builder, v1_update_project_signing_key_task,
    v1_remove_project_signing_key_builder, v1_remove_project_signing_key_task,
    v1_create_a_sso_provider_builder, v1_create_a_sso_provider_task,
    v1_update_a_sso_provider_builder, v1_update_a_sso_provider_task,
    v1_delete_a_sso_provider_builder, v1_delete_a_sso_provider_task,
    v1_create_project_tpa_integration_builder, v1_create_project_tpa_integration_task,
    v1_delete_project_tpa_integration_builder, v1_delete_project_tpa_integration_task,
    v1_update_pooler_config_builder, v1_update_pooler_config_task,
    v1_update_postgres_config_builder, v1_update_postgres_config_task,
    v1_modify_database_disk_builder, v1_modify_database_disk_task,
    v1_update_realtime_config_builder, v1_update_realtime_config_task,
    v1_shutdown_realtime_builder, v1_shutdown_realtime_task,
    v1_update_storage_config_builder, v1_update_storage_config_task,
    v1_delete hostname config_builder, v1_delete hostname config_task,
    v1_activate_custom_hostname_builder, v1_activate_custom_hostname_task,
    v1_update_hostname_config_builder, v1_update_hostname_config_task,
    v1_verify_dns_config_builder, v1_verify_dns_config_task,
    v1_restore_pitr_backup_builder, v1_restore_pitr_backup_task,
    v1_create_restore_point_builder, v1_create_restore_point_task,
    v1_undo_builder, v1_undo_task,
    v1_authorize_jit_access_builder, v1_authorize_jit_access_task,
    v1_update_jit_access_builder, v1_update_jit_access_task,
    v1_delete_jit_access_builder, v1_delete_jit_access_task,
    v1_apply_a_migration_builder, v1_apply_a_migration_task,
    v1_upsert_a_migration_builder, v1_upsert_a_migration_task,
    v1_rollback_migrations_builder, v1_rollback_migrations_task,
    v1_patch_a_migration_builder, v1_patch_a_migration_task,
    v1_update_database_password_builder, v1_update_database_password_task,
    v1_run_a_query_builder, v1_run_a_query_task,
    v1_read_only_query_builder, v1_read_only_query_task,
    v1_enable_database_webhook_builder, v1_enable_database_webhook_task,
    v1_create_a_function_builder, v1_create_a_function_task,
    v1_bulk_update_functions_builder, v1_bulk_update_functions_task,
    v1_deploy_a_function_builder, v1_deploy_a_function_task,
    v1_update_a_function_builder, v1_update_a_function_task,
    v1_delete_a_function_builder, v1_delete_a_function_task,
    v1_update_jit_access_config_builder, v1_update_jit_access_config_task,
    v1_delete_network_bans_builder, v1_delete_network_bans_task,
    v1_list_all_network_bans_builder, v1_list_all_network_bans_task,
    v1_list_all_network_bans_enriched_builder, v1_list_all_network_bans_enriched_task,
    v1_patch_network_restrictions_builder, v1_patch_network_restrictions_task,
    v1_update_network_restrictions_builder, v1_update_network_restrictions_task,
    v1_pause_a_project_builder, v1_pause_a_project_task,
    v1_update_pgsodium_config_builder, v1_update_pgsodium_config_task,
    v1_update_postgrest_service_config_builder, v1_update_postgrest_service_config_task,
    v1_remove_a_read_replica_builder, v1_remove_a_read_replica_task,
    v1_setup_a_read_replica_builder, v1_setup_a_read_replica_task,
    v1_disable_readonly_mode_temporarily_builder, v1_disable_readonly_mode_temporarily_task,
    v1_restore_a_project_builder, v1_restore_a_project_task,
    v1_cancel_a_project_restoration_builder, v1_cancel_a_project_restoration_task,
    v1_bulk_create_secrets_builder, v1_bulk_create_secrets_task,
    v1_bulk_delete_secrets_builder, v1_bulk_delete_secrets_task,
    v1_update_ssl_enforcement_config_builder, v1_update_ssl_enforcement_config_task,
    v1_upgrade_postgres_version_builder, v1_upgrade_postgres_version_task,
    v1_deactivate_vanity_subdomain_config_builder, v1_deactivate_vanity_subdomain_config_task,
    v1_activate_vanity_subdomain_config_builder, v1_activate_vanity_subdomain_config_task,
    v1_check_vanity_subdomain_availability_builder, v1_check_vanity_subdomain_availability_task,
};
use crate::providers::supabase::clients::types::{ApiError, ApiPending};
use crate::providers::supabase::clients::supabase::ActivateVanitySubdomainResponse;
use crate::providers::supabase::clients::supabase::ApiKeyResponse;
use crate::providers::supabase::clients::supabase::AuthConfigResponse;
use crate::providers::supabase::clients::supabase::BranchDeleteResponse;
use crate::providers::supabase::clients::supabase::BranchResponse;
use crate::providers::supabase::clients::supabase::BranchRestoreResponse;
use crate::providers::supabase::clients::supabase::BranchUpdateResponse;
use crate::providers::supabase::clients::supabase::BulkUpdateFunctionResponse;
use crate::providers::supabase::clients::supabase::CreateProjectClaimTokenResponse;
use crate::providers::supabase::clients::supabase::CreateProviderResponse;
use crate::providers::supabase::clients::supabase::CreateRoleResponse;
use crate::providers::supabase::clients::supabase::DeleteProviderResponse;
use crate::providers::supabase::clients::supabase::DeleteRolesResponse;
use crate::providers::supabase::clients::supabase::DeployFunctionResponse;
use crate::providers::supabase::clients::supabase::FunctionResponse;
use crate::providers::supabase::clients::supabase::JitAccessResponse;
use crate::providers::supabase::clients::supabase::JitAuthorizeAccessResponse;
use crate::providers::supabase::clients::supabase::LegacyApiKeysResponse;
use crate::providers::supabase::clients::supabase::NetworkBanResponse;
use crate::providers::supabase::clients::supabase::NetworkBanResponseEnriched;
use crate::providers::supabase::clients::supabase::NetworkRestrictionsResponse;
use crate::providers::supabase::clients::supabase::NetworkRestrictionsV2Response;
use crate::providers::supabase::clients::supabase::OAuthTokenResponse;
use crate::providers::supabase::clients::supabase::OrganizationResponseV1;
use crate::providers::supabase::clients::supabase::PgsodiumConfigResponse;
use crate::providers::supabase::clients::supabase::PostgresConfigResponse;
use crate::providers::supabase::clients::supabase::ProjectUpgradeInitiateResponse;
use crate::providers::supabase::clients::supabase::SigningKeyResponse;
use crate::providers::supabase::clients::supabase::SslEnforcementResponse;
use crate::providers::supabase::clients::supabase::SubdomainAvailabilityResponse;
use crate::providers::supabase::clients::supabase::ThirdPartyAuth;
use crate::providers::supabase::clients::supabase::UpdateCustomHostnameResponse;
use crate::providers::supabase::clients::supabase::UpdateProviderResponse;
use crate::providers::supabase::clients::supabase::UpdateRunStatusResponse;
use crate::providers::supabase::clients::supabase::UpdateSupavisorConfigResponse;
use crate::providers::supabase::clients::supabase::V1PostgrestConfigResponse;
use crate::providers::supabase::clients::supabase::V1ProjectRefResponse;
use crate::providers::supabase::clients::supabase::V1ProjectResponse;
use crate::providers::supabase::clients::supabase::V1RestorePointResponse;
use crate::providers::supabase::clients::supabase::V1UpdatePasswordResponse;
use crate::providers::supabase::clients::supabase::V1ActivateCustomHostnameArgs;
use crate::providers::supabase::clients::supabase::V1ActivateVanitySubdomainConfigArgs;
use crate::providers::supabase::clients::supabase::V1ApplyAMigrationArgs;
use crate::providers::supabase::clients::supabase::V1ApplyProjectAddonArgs;
use crate::providers::supabase::clients::supabase::V1AuthorizeJitAccessArgs;
use crate::providers::supabase::clients::supabase::V1BulkCreateSecretsArgs;
use crate::providers::supabase::clients::supabase::V1BulkDeleteSecretsArgs;
use crate::providers::supabase::clients::supabase::V1BulkUpdateFunctionsArgs;
use crate::providers::supabase::clients::supabase::V1CancelAProjectRestorationArgs;
use crate::providers::supabase::clients::supabase::V1CheckVanitySubdomainAvailabilityArgs;
use crate::providers::supabase::clients::supabase::V1ClaimProjectForOrganizationArgs;
use crate::providers::supabase::clients::supabase::V1CreateABranchArgs;
use crate::providers::supabase::clients::supabase::V1CreateAFunctionArgs;
use crate::providers::supabase::clients::supabase::V1CreateAProjectArgs;
use crate::providers::supabase::clients::supabase::V1CreateASsoProviderArgs;
use crate::providers::supabase::clients::supabase::V1CreateAnOrganizationArgs;
use crate::providers::supabase::clients::supabase::V1CreateLegacySigningKeyArgs;
use crate::providers::supabase::clients::supabase::V1CreateLoginRoleArgs;
use crate::providers::supabase::clients::supabase::V1CreateProjectApiKeyArgs;
use crate::providers::supabase::clients::supabase::V1CreateProjectClaimTokenArgs;
use crate::providers::supabase::clients::supabase::V1CreateProjectSigningKeyArgs;
use crate::providers::supabase::clients::supabase::V1CreateProjectTpaIntegrationArgs;
use crate::providers::supabase::clients::supabase::V1CreateRestorePointArgs;
use crate::providers::supabase::clients::supabase::V1DeactivateVanitySubdomainConfigArgs;
use crate::providers::supabase::clients::supabase::V1Delete hostname configArgs;
use crate::providers::supabase::clients::supabase::V1DeleteABranchArgs;
use crate::providers::supabase::clients::supabase::V1DeleteAFunctionArgs;
use crate::providers::supabase::clients::supabase::V1DeleteAProjectArgs;
use crate::providers::supabase::clients::supabase::V1DeleteASsoProviderArgs;
use crate::providers::supabase::clients::supabase::V1DeleteJitAccessArgs;
use crate::providers::supabase::clients::supabase::V1DeleteLoginRolesArgs;
use crate::providers::supabase::clients::supabase::V1DeleteNetworkBansArgs;
use crate::providers::supabase::clients::supabase::V1DeleteProjectApiKeyArgs;
use crate::providers::supabase::clients::supabase::V1DeleteProjectClaimTokenArgs;
use crate::providers::supabase::clients::supabase::V1DeleteProjectTpaIntegrationArgs;
use crate::providers::supabase::clients::supabase::V1DeployAFunctionArgs;
use crate::providers::supabase::clients::supabase::V1DisablePreviewBranchingArgs;
use crate::providers::supabase::clients::supabase::V1DisableReadonlyModeTemporarilyArgs;
use crate::providers::supabase::clients::supabase::V1EnableDatabaseWebhookArgs;
use crate::providers::supabase::clients::supabase::V1ExchangeOauthTokenArgs;
use crate::providers::supabase::clients::supabase::V1ListAllNetworkBansArgs;
use crate::providers::supabase::clients::supabase::V1ListAllNetworkBansEnrichedArgs;
use crate::providers::supabase::clients::supabase::V1MergeABranchArgs;
use crate::providers::supabase::clients::supabase::V1ModifyDatabaseDiskArgs;
use crate::providers::supabase::clients::supabase::V1PatchAMigrationArgs;
use crate::providers::supabase::clients::supabase::V1PatchNetworkRestrictionsArgs;
use crate::providers::supabase::clients::supabase::V1PauseAProjectArgs;
use crate::providers::supabase::clients::supabase::V1PushABranchArgs;
use crate::providers::supabase::clients::supabase::V1ReadOnlyQueryArgs;
use crate::providers::supabase::clients::supabase::V1RemoveAReadReplicaArgs;
use crate::providers::supabase::clients::supabase::V1RemoveProjectAddonArgs;
use crate::providers::supabase::clients::supabase::V1RemoveProjectSigningKeyArgs;
use crate::providers::supabase::clients::supabase::V1ResetABranchArgs;
use crate::providers::supabase::clients::supabase::V1RestoreABranchArgs;
use crate::providers::supabase::clients::supabase::V1RestoreAProjectArgs;
use crate::providers::supabase::clients::supabase::V1RestorePitrBackupArgs;
use crate::providers::supabase::clients::supabase::V1RevokeTokenArgs;
use crate::providers::supabase::clients::supabase::V1RollbackMigrationsArgs;
use crate::providers::supabase::clients::supabase::V1RunAQueryArgs;
use crate::providers::supabase::clients::supabase::V1SetupAReadReplicaArgs;
use crate::providers::supabase::clients::supabase::V1ShutdownRealtimeArgs;
use crate::providers::supabase::clients::supabase::V1UndoArgs;
use crate::providers::supabase::clients::supabase::V1UpdateABranchConfigArgs;
use crate::providers::supabase::clients::supabase::V1UpdateAFunctionArgs;
use crate::providers::supabase::clients::supabase::V1UpdateAProjectArgs;
use crate::providers::supabase::clients::supabase::V1UpdateASsoProviderArgs;
use crate::providers::supabase::clients::supabase::V1UpdateActionRunStatusArgs;
use crate::providers::supabase::clients::supabase::V1UpdateAuthServiceConfigArgs;
use crate::providers::supabase::clients::supabase::V1UpdateDatabasePasswordArgs;
use crate::providers::supabase::clients::supabase::V1UpdateHostnameConfigArgs;
use crate::providers::supabase::clients::supabase::V1UpdateJitAccessArgs;
use crate::providers::supabase::clients::supabase::V1UpdateJitAccessConfigArgs;
use crate::providers::supabase::clients::supabase::V1UpdateNetworkRestrictionsArgs;
use crate::providers::supabase::clients::supabase::V1UpdatePgsodiumConfigArgs;
use crate::providers::supabase::clients::supabase::V1UpdatePoolerConfigArgs;
use crate::providers::supabase::clients::supabase::V1UpdatePostgresConfigArgs;
use crate::providers::supabase::clients::supabase::V1UpdatePostgrestServiceConfigArgs;
use crate::providers::supabase::clients::supabase::V1UpdateProjectApiKeyArgs;
use crate::providers::supabase::clients::supabase::V1UpdateProjectLegacyApiKeysArgs;
use crate::providers::supabase::clients::supabase::V1UpdateProjectSigningKeyArgs;
use crate::providers::supabase::clients::supabase::V1UpdateRealtimeConfigArgs;
use crate::providers::supabase::clients::supabase::V1UpdateSslEnforcementConfigArgs;
use crate::providers::supabase::clients::supabase::V1UpdateStorageConfigArgs;
use crate::providers::supabase::clients::supabase::V1UpgradePostgresVersionArgs;
use crate::providers::supabase::clients::supabase::V1UpsertAMigrationArgs;
use crate::providers::supabase::clients::supabase::V1VerifyDnsConfigArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// SupabaseProvider with automatic state tracking.
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
/// let provider = SupabaseProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct SupabaseProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> SupabaseProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new SupabaseProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// V1 update a branch config.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BranchResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_update_a_branch_config(
        &self,
        args: &V1UpdateABranchConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BranchResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_update_a_branch_config_builder(
            &self.http_client,
            &args.branch_id_or_ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_update_a_branch_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 delete a branch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BranchDeleteResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_delete_a_branch(
        &self,
        args: &V1DeleteABranchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BranchDeleteResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_delete_a_branch_builder(
            &self.http_client,
            &args.branch_id_or_ref,
            &args.force,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_delete_a_branch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 merge a branch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BranchUpdateResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_merge_a_branch(
        &self,
        args: &V1MergeABranchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BranchUpdateResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_merge_a_branch_builder(
            &self.http_client,
            &args.branch_id_or_ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_merge_a_branch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 push a branch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BranchUpdateResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_push_a_branch(
        &self,
        args: &V1PushABranchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BranchUpdateResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_push_a_branch_builder(
            &self.http_client,
            &args.branch_id_or_ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_push_a_branch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 reset a branch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BranchUpdateResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_reset_a_branch(
        &self,
        args: &V1ResetABranchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BranchUpdateResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_reset_a_branch_builder(
            &self.http_client,
            &args.branch_id_or_ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_reset_a_branch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 restore a branch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BranchRestoreResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_restore_a_branch(
        &self,
        args: &V1RestoreABranchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BranchRestoreResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_restore_a_branch_builder(
            &self.http_client,
            &args.branch_id_or_ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_restore_a_branch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 revoke token.
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
    pub fn v1_revoke_token(
        &self,
        args: &V1RevokeTokenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_revoke_token_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_revoke_token_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 exchange oauth token.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the OAuthTokenResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_exchange_oauth_token(
        &self,
        args: &V1ExchangeOauthTokenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<OAuthTokenResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_exchange_oauth_token_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_exchange_oauth_token_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 create an organization.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the OrganizationResponseV1 result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_create_an_organization(
        &self,
        args: &V1CreateAnOrganizationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<OrganizationResponseV1, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_create_an_organization_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_create_an_organization_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 claim project for organization.
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
    pub fn v1_claim_project_for_organization(
        &self,
        args: &V1ClaimProjectForOrganizationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_claim_project_for_organization_builder(
            &self.http_client,
            &args.slug,
            &args.token,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_claim_project_for_organization_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 create a project.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the V1ProjectResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_create_a_project(
        &self,
        args: &V1CreateAProjectArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<V1ProjectResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_create_a_project_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_create_a_project_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 update a project.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the V1ProjectRefResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_update_a_project(
        &self,
        args: &V1UpdateAProjectArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<V1ProjectRefResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_update_a_project_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_update_a_project_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 delete a project.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the V1ProjectRefResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_delete_a_project(
        &self,
        args: &V1DeleteAProjectArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<V1ProjectRefResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_delete_a_project_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_delete_a_project_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 update action run status.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UpdateRunStatusResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_update_action_run_status(
        &self,
        args: &V1UpdateActionRunStatusArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UpdateRunStatusResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_update_action_run_status_builder(
            &self.http_client,
            &args.ref,
            &args.run_id,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_update_action_run_status_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 create project api key.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApiKeyResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_create_project_api_key(
        &self,
        args: &V1CreateProjectApiKeyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApiKeyResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_create_project_api_key_builder(
            &self.http_client,
            &args.ref,
            &args.reveal,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_create_project_api_key_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 update project legacy api keys.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LegacyApiKeysResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_update_project_legacy_api_keys(
        &self,
        args: &V1UpdateProjectLegacyApiKeysArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LegacyApiKeysResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_update_project_legacy_api_keys_builder(
            &self.http_client,
            &args.ref,
            &args.enabled,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_update_project_legacy_api_keys_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 update project api key.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApiKeyResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_update_project_api_key(
        &self,
        args: &V1UpdateProjectApiKeyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApiKeyResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_update_project_api_key_builder(
            &self.http_client,
            &args.ref,
            &args.id,
            &args.reveal,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_update_project_api_key_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 delete project api key.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApiKeyResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_delete_project_api_key(
        &self,
        args: &V1DeleteProjectApiKeyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApiKeyResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_delete_project_api_key_builder(
            &self.http_client,
            &args.ref,
            &args.id,
            &args.reveal,
            &args.was_compromised,
            &args.reason,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_delete_project_api_key_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 apply project addon.
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
    pub fn v1_apply_project_addon(
        &self,
        args: &V1ApplyProjectAddonArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_apply_project_addon_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_apply_project_addon_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 remove project addon.
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
    pub fn v1_remove_project_addon(
        &self,
        args: &V1RemoveProjectAddonArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_remove_project_addon_builder(
            &self.http_client,
            &args.ref,
            &args.addon_variant,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_remove_project_addon_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 create a branch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BranchResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_create_a_branch(
        &self,
        args: &V1CreateABranchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BranchResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_create_a_branch_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_create_a_branch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 disable preview branching.
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
    pub fn v1_disable_preview_branching(
        &self,
        args: &V1DisablePreviewBranchingArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_disable_preview_branching_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_disable_preview_branching_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 create project claim token.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CreateProjectClaimTokenResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_create_project_claim_token(
        &self,
        args: &V1CreateProjectClaimTokenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CreateProjectClaimTokenResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_create_project_claim_token_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_create_project_claim_token_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 delete project claim token.
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
    pub fn v1_delete_project_claim_token(
        &self,
        args: &V1DeleteProjectClaimTokenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_delete_project_claim_token_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_delete_project_claim_token_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 create login role.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CreateRoleResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_create_login_role(
        &self,
        args: &V1CreateLoginRoleArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CreateRoleResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_create_login_role_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_create_login_role_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 delete login roles.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeleteRolesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_delete_login_roles(
        &self,
        args: &V1DeleteLoginRolesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeleteRolesResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_delete_login_roles_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_delete_login_roles_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 update auth service config.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AuthConfigResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_update_auth_service_config(
        &self,
        args: &V1UpdateAuthServiceConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AuthConfigResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_update_auth_service_config_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_update_auth_service_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 create project signing key.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SigningKeyResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_create_project_signing_key(
        &self,
        args: &V1CreateProjectSigningKeyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SigningKeyResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_create_project_signing_key_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_create_project_signing_key_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 create legacy signing key.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SigningKeyResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_create_legacy_signing_key(
        &self,
        args: &V1CreateLegacySigningKeyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SigningKeyResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_create_legacy_signing_key_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_create_legacy_signing_key_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 update project signing key.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SigningKeyResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_update_project_signing_key(
        &self,
        args: &V1UpdateProjectSigningKeyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SigningKeyResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_update_project_signing_key_builder(
            &self.http_client,
            &args.id,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_update_project_signing_key_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 remove project signing key.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SigningKeyResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_remove_project_signing_key(
        &self,
        args: &V1RemoveProjectSigningKeyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SigningKeyResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_remove_project_signing_key_builder(
            &self.http_client,
            &args.id,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_remove_project_signing_key_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 create a sso provider.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CreateProviderResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_create_a_sso_provider(
        &self,
        args: &V1CreateASsoProviderArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CreateProviderResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_create_a_sso_provider_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_create_a_sso_provider_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 update a sso provider.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UpdateProviderResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_update_a_sso_provider(
        &self,
        args: &V1UpdateASsoProviderArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UpdateProviderResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_update_a_sso_provider_builder(
            &self.http_client,
            &args.ref,
            &args.provider_id,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_update_a_sso_provider_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 delete a sso provider.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeleteProviderResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_delete_a_sso_provider(
        &self,
        args: &V1DeleteASsoProviderArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeleteProviderResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_delete_a_sso_provider_builder(
            &self.http_client,
            &args.ref,
            &args.provider_id,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_delete_a_sso_provider_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 create project tpa integration.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ThirdPartyAuth result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_create_project_tpa_integration(
        &self,
        args: &V1CreateProjectTpaIntegrationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ThirdPartyAuth, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_create_project_tpa_integration_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_create_project_tpa_integration_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 delete project tpa integration.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ThirdPartyAuth result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_delete_project_tpa_integration(
        &self,
        args: &V1DeleteProjectTpaIntegrationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ThirdPartyAuth, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_delete_project_tpa_integration_builder(
            &self.http_client,
            &args.ref,
            &args.tpa_id,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_delete_project_tpa_integration_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 update pooler config.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UpdateSupavisorConfigResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_update_pooler_config(
        &self,
        args: &V1UpdatePoolerConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UpdateSupavisorConfigResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_update_pooler_config_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_update_pooler_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 update postgres config.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PostgresConfigResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_update_postgres_config(
        &self,
        args: &V1UpdatePostgresConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PostgresConfigResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_update_postgres_config_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_update_postgres_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 modify database disk.
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
    pub fn v1_modify_database_disk(
        &self,
        args: &V1ModifyDatabaseDiskArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_modify_database_disk_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_modify_database_disk_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 update realtime config.
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
    pub fn v1_update_realtime_config(
        &self,
        args: &V1UpdateRealtimeConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_update_realtime_config_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_update_realtime_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 shutdown realtime.
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
    pub fn v1_shutdown_realtime(
        &self,
        args: &V1ShutdownRealtimeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_shutdown_realtime_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_shutdown_realtime_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 update storage config.
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
    pub fn v1_update_storage_config(
        &self,
        args: &V1UpdateStorageConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_update_storage_config_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_update_storage_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 delete hostname config.
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
    pub fn v1_delete hostname config(
        &self,
        args: &V1Delete hostname configArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_delete hostname config_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_delete hostname config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 activate custom hostname.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UpdateCustomHostnameResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_activate_custom_hostname(
        &self,
        args: &V1ActivateCustomHostnameArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UpdateCustomHostnameResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_activate_custom_hostname_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_activate_custom_hostname_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 update hostname config.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UpdateCustomHostnameResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_update_hostname_config(
        &self,
        args: &V1UpdateHostnameConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UpdateCustomHostnameResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_update_hostname_config_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_update_hostname_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 verify dns config.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UpdateCustomHostnameResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_verify_dns_config(
        &self,
        args: &V1VerifyDnsConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UpdateCustomHostnameResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_verify_dns_config_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_verify_dns_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 restore pitr backup.
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
    pub fn v1_restore_pitr_backup(
        &self,
        args: &V1RestorePitrBackupArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_restore_pitr_backup_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_restore_pitr_backup_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 create restore point.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the V1RestorePointResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_create_restore_point(
        &self,
        args: &V1CreateRestorePointArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<V1RestorePointResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_create_restore_point_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_create_restore_point_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 undo.
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
    pub fn v1_undo(
        &self,
        args: &V1UndoArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_undo_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_undo_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 authorize jit access.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the JitAuthorizeAccessResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_authorize_jit_access(
        &self,
        args: &V1AuthorizeJitAccessArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<JitAuthorizeAccessResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_authorize_jit_access_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_authorize_jit_access_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 update jit access.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the JitAccessResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_update_jit_access(
        &self,
        args: &V1UpdateJitAccessArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<JitAccessResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_update_jit_access_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_update_jit_access_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 delete jit access.
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
    pub fn v1_delete_jit_access(
        &self,
        args: &V1DeleteJitAccessArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_delete_jit_access_builder(
            &self.http_client,
            &args.ref,
            &args.user_id,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_delete_jit_access_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 apply a migration.
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
    pub fn v1_apply_a_migration(
        &self,
        args: &V1ApplyAMigrationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_apply_a_migration_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_apply_a_migration_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 upsert a migration.
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
    pub fn v1_upsert_a_migration(
        &self,
        args: &V1UpsertAMigrationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_upsert_a_migration_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_upsert_a_migration_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 rollback migrations.
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
    pub fn v1_rollback_migrations(
        &self,
        args: &V1RollbackMigrationsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_rollback_migrations_builder(
            &self.http_client,
            &args.ref,
            &args.gte,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_rollback_migrations_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 patch a migration.
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
    pub fn v1_patch_a_migration(
        &self,
        args: &V1PatchAMigrationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_patch_a_migration_builder(
            &self.http_client,
            &args.ref,
            &args.version,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_patch_a_migration_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 update database password.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the V1UpdatePasswordResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_update_database_password(
        &self,
        args: &V1UpdateDatabasePasswordArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<V1UpdatePasswordResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_update_database_password_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_update_database_password_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 run a query.
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
    pub fn v1_run_a_query(
        &self,
        args: &V1RunAQueryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_run_a_query_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_run_a_query_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 read only query.
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
    pub fn v1_read_only_query(
        &self,
        args: &V1ReadOnlyQueryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_read_only_query_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_read_only_query_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 enable database webhook.
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
    pub fn v1_enable_database_webhook(
        &self,
        args: &V1EnableDatabaseWebhookArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_enable_database_webhook_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_enable_database_webhook_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 create a function.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FunctionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_create_a_function(
        &self,
        args: &V1CreateAFunctionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FunctionResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_create_a_function_builder(
            &self.http_client,
            &args.ref,
            &args.slug,
            &args.name,
            &args.verify_jwt,
            &args.import_map,
            &args.entrypoint_path,
            &args.import_map_path,
            &args.ezbr_sha256,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_create_a_function_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 bulk update functions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BulkUpdateFunctionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_bulk_update_functions(
        &self,
        args: &V1BulkUpdateFunctionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BulkUpdateFunctionResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_bulk_update_functions_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_bulk_update_functions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 deploy a function.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeployFunctionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_deploy_a_function(
        &self,
        args: &V1DeployAFunctionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeployFunctionResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_deploy_a_function_builder(
            &self.http_client,
            &args.ref,
            &args.slug,
            &args.bundleOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_deploy_a_function_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 update a function.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FunctionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_update_a_function(
        &self,
        args: &V1UpdateAFunctionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FunctionResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_update_a_function_builder(
            &self.http_client,
            &args.ref,
            &args.function_slug,
            &args.slug,
            &args.name,
            &args.verify_jwt,
            &args.import_map,
            &args.entrypoint_path,
            &args.import_map_path,
            &args.ezbr_sha256,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_update_a_function_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 delete a function.
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
    pub fn v1_delete_a_function(
        &self,
        args: &V1DeleteAFunctionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_delete_a_function_builder(
            &self.http_client,
            &args.ref,
            &args.function_slug,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_delete_a_function_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 update jit access config.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the JitAccessResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_update_jit_access_config(
        &self,
        args: &V1UpdateJitAccessConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<JitAccessResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_update_jit_access_config_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_update_jit_access_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 delete network bans.
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
    pub fn v1_delete_network_bans(
        &self,
        args: &V1DeleteNetworkBansArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_delete_network_bans_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_delete_network_bans_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 list all network bans.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NetworkBanResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_list_all_network_bans(
        &self,
        args: &V1ListAllNetworkBansArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NetworkBanResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_list_all_network_bans_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_list_all_network_bans_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 list all network bans enriched.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NetworkBanResponseEnriched result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_list_all_network_bans_enriched(
        &self,
        args: &V1ListAllNetworkBansEnrichedArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NetworkBanResponseEnriched, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_list_all_network_bans_enriched_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_list_all_network_bans_enriched_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 patch network restrictions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NetworkRestrictionsV2Response result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_patch_network_restrictions(
        &self,
        args: &V1PatchNetworkRestrictionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NetworkRestrictionsV2Response, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_patch_network_restrictions_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_patch_network_restrictions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 update network restrictions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NetworkRestrictionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_update_network_restrictions(
        &self,
        args: &V1UpdateNetworkRestrictionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NetworkRestrictionsResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_update_network_restrictions_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_update_network_restrictions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 pause a project.
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
    pub fn v1_pause_a_project(
        &self,
        args: &V1PauseAProjectArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_pause_a_project_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_pause_a_project_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 update pgsodium config.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PgsodiumConfigResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_update_pgsodium_config(
        &self,
        args: &V1UpdatePgsodiumConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PgsodiumConfigResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_update_pgsodium_config_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_update_pgsodium_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 update postgrest service config.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the V1PostgrestConfigResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_update_postgrest_service_config(
        &self,
        args: &V1UpdatePostgrestServiceConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<V1PostgrestConfigResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_update_postgrest_service_config_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_update_postgrest_service_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 remove a read replica.
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
    pub fn v1_remove_a_read_replica(
        &self,
        args: &V1RemoveAReadReplicaArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_remove_a_read_replica_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_remove_a_read_replica_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 setup a read replica.
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
    pub fn v1_setup_a_read_replica(
        &self,
        args: &V1SetupAReadReplicaArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_setup_a_read_replica_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_setup_a_read_replica_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 disable readonly mode temporarily.
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
    pub fn v1_disable_readonly_mode_temporarily(
        &self,
        args: &V1DisableReadonlyModeTemporarilyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_disable_readonly_mode_temporarily_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_disable_readonly_mode_temporarily_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 restore a project.
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
    pub fn v1_restore_a_project(
        &self,
        args: &V1RestoreAProjectArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_restore_a_project_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_restore_a_project_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 cancel a project restoration.
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
    pub fn v1_cancel_a_project_restoration(
        &self,
        args: &V1CancelAProjectRestorationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_cancel_a_project_restoration_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_cancel_a_project_restoration_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 bulk create secrets.
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
    pub fn v1_bulk_create_secrets(
        &self,
        args: &V1BulkCreateSecretsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_bulk_create_secrets_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_bulk_create_secrets_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 bulk delete secrets.
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
    pub fn v1_bulk_delete_secrets(
        &self,
        args: &V1BulkDeleteSecretsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_bulk_delete_secrets_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_bulk_delete_secrets_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 update ssl enforcement config.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SslEnforcementResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_update_ssl_enforcement_config(
        &self,
        args: &V1UpdateSslEnforcementConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SslEnforcementResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_update_ssl_enforcement_config_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_update_ssl_enforcement_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 upgrade postgres version.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProjectUpgradeInitiateResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_upgrade_postgres_version(
        &self,
        args: &V1UpgradePostgresVersionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProjectUpgradeInitiateResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_upgrade_postgres_version_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_upgrade_postgres_version_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 deactivate vanity subdomain config.
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
    pub fn v1_deactivate_vanity_subdomain_config(
        &self,
        args: &V1DeactivateVanitySubdomainConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_deactivate_vanity_subdomain_config_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_deactivate_vanity_subdomain_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 activate vanity subdomain config.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ActivateVanitySubdomainResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_activate_vanity_subdomain_config(
        &self,
        args: &V1ActivateVanitySubdomainConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ActivateVanitySubdomainResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_activate_vanity_subdomain_config_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_activate_vanity_subdomain_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 check vanity subdomain availability.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SubdomainAvailabilityResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_check_vanity_subdomain_availability(
        &self,
        args: &V1CheckVanitySubdomainAvailabilityArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SubdomainAvailabilityResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_check_vanity_subdomain_availability_builder(
            &self.http_client,
            &args.ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_check_vanity_subdomain_availability_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
