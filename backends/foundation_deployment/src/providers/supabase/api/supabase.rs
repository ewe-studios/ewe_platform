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

use crate::providers::supabase::clients::{
    v1_get_a_branch_config_builder, v1_get_a_branch_config_task,
    v1_update_a_branch_config_builder, v1_update_a_branch_config_task,
    v1_delete_a_branch_builder, v1_delete_a_branch_task,
    v1_diff_a_branch_builder, v1_diff_a_branch_task,
    v1_merge_a_branch_builder, v1_merge_a_branch_task,
    v1_push_a_branch_builder, v1_push_a_branch_task,
    v1_reset_a_branch_builder, v1_reset_a_branch_task,
    v1_restore_a_branch_builder, v1_restore_a_branch_task,
    v1_authorize_user_builder, v1_authorize_user_task,
    v1_oauth_authorize_project_claim_builder, v1_oauth_authorize_project_claim_task,
    v1_revoke_token_builder, v1_revoke_token_task,
    v1_exchange_oauth_token_builder, v1_exchange_oauth_token_task,
    v1_list_all_organizations_builder, v1_list_all_organizations_task,
    v1_create_an_organization_builder, v1_create_an_organization_task,
    v1_get_an_organization_builder, v1_get_an_organization_task,
    v1_get_organization_entitlements_builder, v1_get_organization_entitlements_task,
    v1_list_organization_members_builder, v1_list_organization_members_task,
    v1_get_organization_project_claim_builder, v1_get_organization_project_claim_task,
    v1_claim_project_for_organization_builder, v1_claim_project_for_organization_task,
    v1_get_all_projects_for_organization_builder, v1_get_all_projects_for_organization_task,
    v1_get_profile_builder, v1_get_profile_task,
    v1_list_all_projects_builder, v1_list_all_projects_task,
    v1_create_a_project_builder, v1_create_a_project_task,
    v1_get_available_regions_builder, v1_get_available_regions_task,
    v1_get_project_builder, v1_get_project_task,
    v1_update_a_project_builder, v1_update_a_project_task,
    v1_delete_a_project_builder, v1_delete_a_project_task,
    v1_list_action_runs_builder, v1_list_action_runs_task,
    v1_get_action_run_builder, v1_get_action_run_task,
    v1_get_action_run_logs_builder, v1_get_action_run_logs_task,
    v1_update_action_run_status_builder, v1_update_action_run_status_task,
    v1_get_performance_advisors_builder, v1_get_performance_advisors_task,
    v1_get_security_advisors_builder, v1_get_security_advisors_task,
    v1_get_project_function_combined_stats_builder, v1_get_project_function_combined_stats_task,
    v1_get_project_logs_builder, v1_get_project_logs_task,
    v1_get_project_usage_api_count_builder, v1_get_project_usage_api_count_task,
    v1_get_project_usage_request_count_builder, v1_get_project_usage_request_count_task,
    v1_get_project_api_keys_builder, v1_get_project_api_keys_task,
    v1_create_project_api_key_builder, v1_create_project_api_key_task,
    v1_get_project_legacy_api_keys_builder, v1_get_project_legacy_api_keys_task,
    v1_update_project_legacy_api_keys_builder, v1_update_project_legacy_api_keys_task,
    v1_get_project_api_key_builder, v1_get_project_api_key_task,
    v1_update_project_api_key_builder, v1_update_project_api_key_task,
    v1_delete_project_api_key_builder, v1_delete_project_api_key_task,
    v1_list_project_addons_builder, v1_list_project_addons_task,
    v1_apply_project_addon_builder, v1_apply_project_addon_task,
    v1_remove_project_addon_builder, v1_remove_project_addon_task,
    v1_list_all_branches_builder, v1_list_all_branches_task,
    v1_create_a_branch_builder, v1_create_a_branch_task,
    v1_disable_preview_branching_builder, v1_disable_preview_branching_task,
    v1_get_a_branch_builder, v1_get_a_branch_task,
    v1_get_project_claim_token_builder, v1_get_project_claim_token_task,
    v1_create_project_claim_token_builder, v1_create_project_claim_token_task,
    v1_delete_project_claim_token_builder, v1_delete_project_claim_token_task,
    v1_create_login_role_builder, v1_create_login_role_task,
    v1_delete_login_roles_builder, v1_delete_login_roles_task,
    v1_get_auth_service_config_builder, v1_get_auth_service_config_task,
    v1_update_auth_service_config_builder, v1_update_auth_service_config_task,
    v1_get_project_signing_keys_builder, v1_get_project_signing_keys_task,
    v1_create_project_signing_key_builder, v1_create_project_signing_key_task,
    v1_get_legacy_signing_key_builder, v1_get_legacy_signing_key_task,
    v1_create_legacy_signing_key_builder, v1_create_legacy_signing_key_task,
    v1_get_project_signing_key_builder, v1_get_project_signing_key_task,
    v1_update_project_signing_key_builder, v1_update_project_signing_key_task,
    v1_remove_project_signing_key_builder, v1_remove_project_signing_key_task,
    v1_list_all_sso_provider_builder, v1_list_all_sso_provider_task,
    v1_create_a_sso_provider_builder, v1_create_a_sso_provider_task,
    v1_get_a_sso_provider_builder, v1_get_a_sso_provider_task,
    v1_update_a_sso_provider_builder, v1_update_a_sso_provider_task,
    v1_delete_a_sso_provider_builder, v1_delete_a_sso_provider_task,
    v1_list_project_tpa_integrations_builder, v1_list_project_tpa_integrations_task,
    v1_create_project_tpa_integration_builder, v1_create_project_tpa_integration_task,
    v1_get_project_tpa_integration_builder, v1_get_project_tpa_integration_task,
    v1_delete_project_tpa_integration_builder, v1_delete_project_tpa_integration_task,
    v1_get_project_pgbouncer_config_builder, v1_get_project_pgbouncer_config_task,
    v1_get_pooler_config_builder, v1_get_pooler_config_task,
    v1_update_pooler_config_builder, v1_update_pooler_config_task,
    v1_get_postgres_config_builder, v1_get_postgres_config_task,
    v1_update_postgres_config_builder, v1_update_postgres_config_task,
    v1_get_database_disk_builder, v1_get_database_disk_task,
    v1_modify_database_disk_builder, v1_modify_database_disk_task,
    v1_get_project_disk_autoscale_config_builder, v1_get_project_disk_autoscale_config_task,
    v1_get_disk_utilization_builder, v1_get_disk_utilization_task,
    v1_get_realtime_config_builder, v1_get_realtime_config_task,
    v1_update_realtime_config_builder, v1_update_realtime_config_task,
    v1_shutdown_realtime_builder, v1_shutdown_realtime_task,
    v1_get_storage_config_builder, v1_get_storage_config_task,
    v1_update_storage_config_builder, v1_update_storage_config_task,
    v1_get_hostname_config_builder, v1_get_hostname_config_task,
    v1_delete hostname config_builder, v1_delete hostname config_task,
    v1_activate_custom_hostname_builder, v1_activate_custom_hostname_task,
    v1_update_hostname_config_builder, v1_update_hostname_config_task,
    v1_verify_dns_config_builder, v1_verify_dns_config_task,
    v1_list_all_backups_builder, v1_list_all_backups_task,
    v1_restore_pitr_backup_builder, v1_restore_pitr_backup_task,
    v1_get_restore_point_builder, v1_get_restore_point_task,
    v1_create_restore_point_builder, v1_create_restore_point_task,
    v1_undo_builder, v1_undo_task,
    v1_get_database_metadata_builder, v1_get_database_metadata_task,
    v1_get_jit_access_builder, v1_get_jit_access_task,
    v1_authorize_jit_access_builder, v1_authorize_jit_access_task,
    v1_update_jit_access_builder, v1_update_jit_access_task,
    v1_list_jit_access_builder, v1_list_jit_access_task,
    v1_delete_jit_access_builder, v1_delete_jit_access_task,
    v1_list_migration_history_builder, v1_list_migration_history_task,
    v1_apply_a_migration_builder, v1_apply_a_migration_task,
    v1_upsert_a_migration_builder, v1_upsert_a_migration_task,
    v1_rollback_migrations_builder, v1_rollback_migrations_task,
    v1_get_a_migration_builder, v1_get_a_migration_task,
    v1_patch_a_migration_builder, v1_patch_a_migration_task,
    v1_get_database_openapi_builder, v1_get_database_openapi_task,
    v1_update_database_password_builder, v1_update_database_password_task,
    v1_run_a_query_builder, v1_run_a_query_task,
    v1_read_only_query_builder, v1_read_only_query_task,
    v1_enable_database_webhook_builder, v1_enable_database_webhook_task,
    v1_list_all_functions_builder, v1_list_all_functions_task,
    v1_create_a_function_builder, v1_create_a_function_task,
    v1_bulk_update_functions_builder, v1_bulk_update_functions_task,
    v1_deploy_a_function_builder, v1_deploy_a_function_task,
    v1_get_a_function_builder, v1_get_a_function_task,
    v1_update_a_function_builder, v1_update_a_function_task,
    v1_delete_a_function_builder, v1_delete_a_function_task,
    v1_get_a_function_body_builder, v1_get_a_function_body_task,
    v1_get_services_health_builder, v1_get_services_health_task,
    v1_get_jit_access_config_builder, v1_get_jit_access_config_task,
    v1_update_jit_access_config_builder, v1_update_jit_access_config_task,
    v1_delete_network_bans_builder, v1_delete_network_bans_task,
    v1_list_all_network_bans_builder, v1_list_all_network_bans_task,
    v1_list_all_network_bans_enriched_builder, v1_list_all_network_bans_enriched_task,
    v1_get_network_restrictions_builder, v1_get_network_restrictions_task,
    v1_patch_network_restrictions_builder, v1_patch_network_restrictions_task,
    v1_update_network_restrictions_builder, v1_update_network_restrictions_task,
    v1_pause_a_project_builder, v1_pause_a_project_task,
    v1_get_pgsodium_config_builder, v1_get_pgsodium_config_task,
    v1_update_pgsodium_config_builder, v1_update_pgsodium_config_task,
    v1_get_postgrest_service_config_builder, v1_get_postgrest_service_config_task,
    v1_update_postgrest_service_config_builder, v1_update_postgrest_service_config_task,
    v1_remove_a_read_replica_builder, v1_remove_a_read_replica_task,
    v1_setup_a_read_replica_builder, v1_setup_a_read_replica_task,
    v1_get_readonly_mode_status_builder, v1_get_readonly_mode_status_task,
    v1_disable_readonly_mode_temporarily_builder, v1_disable_readonly_mode_temporarily_task,
    v1_list_available_restore_versions_builder, v1_list_available_restore_versions_task,
    v1_restore_a_project_builder, v1_restore_a_project_task,
    v1_cancel_a_project_restoration_builder, v1_cancel_a_project_restoration_task,
    v1_list_all_secrets_builder, v1_list_all_secrets_task,
    v1_bulk_create_secrets_builder, v1_bulk_create_secrets_task,
    v1_bulk_delete_secrets_builder, v1_bulk_delete_secrets_task,
    v1_get_ssl_enforcement_config_builder, v1_get_ssl_enforcement_config_task,
    v1_update_ssl_enforcement_config_builder, v1_update_ssl_enforcement_config_task,
    v1_list_all_buckets_builder, v1_list_all_buckets_task,
    v1_generate_typescript_types_builder, v1_generate_typescript_types_task,
    v1_upgrade_postgres_version_builder, v1_upgrade_postgres_version_task,
    v1_get_postgres_upgrade_eligibility_builder, v1_get_postgres_upgrade_eligibility_task,
    v1_get_postgres_upgrade_status_builder, v1_get_postgres_upgrade_status_task,
    v1_get_vanity_subdomain_config_builder, v1_get_vanity_subdomain_config_task,
    v1_deactivate_vanity_subdomain_config_builder, v1_deactivate_vanity_subdomain_config_task,
    v1_activate_vanity_subdomain_config_builder, v1_activate_vanity_subdomain_config_task,
    v1_check_vanity_subdomain_availability_builder, v1_check_vanity_subdomain_availability_task,
    v1_list_all_snippets_builder, v1_list_all_snippets_task,
    v1_get_a_snippet_builder, v1_get_a_snippet_task,
};
use crate::providers::supabase::clients::types::{ApiError, ApiPending};
use crate::providers::supabase::clients::ActionRunResponse;
use crate::providers::supabase::clients::ActivateVanitySubdomainResponse;
use crate::providers::supabase::clients::AnalyticsResponse;
use crate::providers::supabase::clients::ApiKeyResponse;
use crate::providers::supabase::clients::AuthConfigResponse;
use crate::providers::supabase::clients::BranchDeleteResponse;
use crate::providers::supabase::clients::BranchDetailResponse;
use crate::providers::supabase::clients::BranchResponse;
use crate::providers::supabase::clients::BranchRestoreResponse;
use crate::providers::supabase::clients::BranchUpdateResponse;
use crate::providers::supabase::clients::BulkUpdateFunctionResponse;
use crate::providers::supabase::clients::CreateProjectClaimTokenResponse;
use crate::providers::supabase::clients::CreateProviderResponse;
use crate::providers::supabase::clients::CreateRoleResponse;
use crate::providers::supabase::clients::DatabaseUpgradeStatusResponse;
use crate::providers::supabase::clients::DeleteProviderResponse;
use crate::providers::supabase::clients::DeleteRolesResponse;
use crate::providers::supabase::clients::DeployFunctionResponse;
use crate::providers::supabase::clients::DiskAutoscaleConfig;
use crate::providers::supabase::clients::DiskResponse;
use crate::providers::supabase::clients::DiskUtilMetricsResponse;
use crate::providers::supabase::clients::FunctionResponse;
use crate::providers::supabase::clients::FunctionSlugResponse;
use crate::providers::supabase::clients::GetProjectAvailableRestoreVersionsResponse;
use crate::providers::supabase::clients::GetProjectDbMetadataResponse;
use crate::providers::supabase::clients::GetProviderResponse;
use crate::providers::supabase::clients::JitAccessResponse;
use crate::providers::supabase::clients::JitAuthorizeAccessResponse;
use crate::providers::supabase::clients::JitListAccessResponse;
use crate::providers::supabase::clients::LegacyApiKeysResponse;
use crate::providers::supabase::clients::ListActionRunResponse;
use crate::providers::supabase::clients::ListProjectAddonsResponse;
use crate::providers::supabase::clients::ListProvidersResponse;
use crate::providers::supabase::clients::NetworkBanResponse;
use crate::providers::supabase::clients::NetworkBanResponseEnriched;
use crate::providers::supabase::clients::NetworkRestrictionsResponse;
use crate::providers::supabase::clients::NetworkRestrictionsV2Response;
use crate::providers::supabase::clients::OAuthTokenResponse;
use crate::providers::supabase::clients::OrganizationProjectClaimResponse;
use crate::providers::supabase::clients::OrganizationProjectsResponse;
use crate::providers::supabase::clients::OrganizationResponseV1;
use crate::providers::supabase::clients::PgsodiumConfigResponse;
use crate::providers::supabase::clients::PostgresConfigResponse;
use crate::providers::supabase::clients::PostgrestConfigWithJWTSecretResponse;
use crate::providers::supabase::clients::ProjectClaimTokenResponse;
use crate::providers::supabase::clients::ProjectUpgradeEligibilityResponse;
use crate::providers::supabase::clients::ProjectUpgradeInitiateResponse;
use crate::providers::supabase::clients::ReadOnlyStatusResponse;
use crate::providers::supabase::clients::RealtimeConfigResponse;
use crate::providers::supabase::clients::RegionsInfo;
use crate::providers::supabase::clients::SigningKeyResponse;
use crate::providers::supabase::clients::SigningKeysResponse;
use crate::providers::supabase::clients::SnippetList;
use crate::providers::supabase::clients::SnippetResponse;
use crate::providers::supabase::clients::SslEnforcementResponse;
use crate::providers::supabase::clients::StorageConfigResponse;
use crate::providers::supabase::clients::StreamableFile;
use crate::providers::supabase::clients::SubdomainAvailabilityResponse;
use crate::providers::supabase::clients::ThirdPartyAuth;
use crate::providers::supabase::clients::TypescriptResponse;
use crate::providers::supabase::clients::UpdateCustomHostnameResponse;
use crate::providers::supabase::clients::UpdateProviderResponse;
use crate::providers::supabase::clients::UpdateRunStatusResponse;
use crate::providers::supabase::clients::UpdateSupavisorConfigResponse;
use crate::providers::supabase::clients::V1BackupsResponse;
use crate::providers::supabase::clients::V1GetMigrationResponse;
use crate::providers::supabase::clients::V1GetUsageApiCountResponse;
use crate::providers::supabase::clients::V1GetUsageApiRequestsCountResponse;
use crate::providers::supabase::clients::V1ListEntitlementsResponse;
use crate::providers::supabase::clients::V1ListMigrationsResponse;
use crate::providers::supabase::clients::V1OrganizationSlugResponse;
use crate::providers::supabase::clients::V1PgbouncerConfigResponse;
use crate::providers::supabase::clients::V1PostgrestConfigResponse;
use crate::providers::supabase::clients::V1ProfileResponse;
use crate::providers::supabase::clients::V1ProjectAdvisorsResponse;
use crate::providers::supabase::clients::V1ProjectRefResponse;
use crate::providers::supabase::clients::V1ProjectResponse;
use crate::providers::supabase::clients::V1ProjectWithDatabaseResponse;
use crate::providers::supabase::clients::V1RestorePointResponse;
use crate::providers::supabase::clients::V1UpdatePasswordResponse;
use crate::providers::supabase::clients::VanitySubdomainConfigResponse;
use crate::providers::supabase::clients::V1ActivateCustomHostnameArgs;
use crate::providers::supabase::clients::V1ActivateVanitySubdomainConfigArgs;
use crate::providers::supabase::clients::V1ApplyAMigrationArgs;
use crate::providers::supabase::clients::V1ApplyProjectAddonArgs;
use crate::providers::supabase::clients::V1AuthorizeJitAccessArgs;
use crate::providers::supabase::clients::V1AuthorizeUserArgs;
use crate::providers::supabase::clients::V1BulkCreateSecretsArgs;
use crate::providers::supabase::clients::V1BulkDeleteSecretsArgs;
use crate::providers::supabase::clients::V1BulkUpdateFunctionsArgs;
use crate::providers::supabase::clients::V1CancelAProjectRestorationArgs;
use crate::providers::supabase::clients::V1CheckVanitySubdomainAvailabilityArgs;
use crate::providers::supabase::clients::V1ClaimProjectForOrganizationArgs;
use crate::providers::supabase::clients::V1CreateABranchArgs;
use crate::providers::supabase::clients::V1CreateAFunctionArgs;
use crate::providers::supabase::clients::V1CreateAProjectArgs;
use crate::providers::supabase::clients::V1CreateASsoProviderArgs;
use crate::providers::supabase::clients::V1CreateAnOrganizationArgs;
use crate::providers::supabase::clients::V1CreateLegacySigningKeyArgs;
use crate::providers::supabase::clients::V1CreateLoginRoleArgs;
use crate::providers::supabase::clients::V1CreateProjectApiKeyArgs;
use crate::providers::supabase::clients::V1CreateProjectClaimTokenArgs;
use crate::providers::supabase::clients::V1CreateProjectSigningKeyArgs;
use crate::providers::supabase::clients::V1CreateProjectTpaIntegrationArgs;
use crate::providers::supabase::clients::V1CreateRestorePointArgs;
use crate::providers::supabase::clients::V1DeactivateVanitySubdomainConfigArgs;
use crate::providers::supabase::clients::V1Delete hostname configArgs;
use crate::providers::supabase::clients::V1DeleteABranchArgs;
use crate::providers::supabase::clients::V1DeleteAFunctionArgs;
use crate::providers::supabase::clients::V1DeleteAProjectArgs;
use crate::providers::supabase::clients::V1DeleteASsoProviderArgs;
use crate::providers::supabase::clients::V1DeleteJitAccessArgs;
use crate::providers::supabase::clients::V1DeleteLoginRolesArgs;
use crate::providers::supabase::clients::V1DeleteNetworkBansArgs;
use crate::providers::supabase::clients::V1DeleteProjectApiKeyArgs;
use crate::providers::supabase::clients::V1DeleteProjectClaimTokenArgs;
use crate::providers::supabase::clients::V1DeleteProjectTpaIntegrationArgs;
use crate::providers::supabase::clients::V1DeployAFunctionArgs;
use crate::providers::supabase::clients::V1DiffABranchArgs;
use crate::providers::supabase::clients::V1DisablePreviewBranchingArgs;
use crate::providers::supabase::clients::V1DisableReadonlyModeTemporarilyArgs;
use crate::providers::supabase::clients::V1EnableDatabaseWebhookArgs;
use crate::providers::supabase::clients::V1GenerateTypescriptTypesArgs;
use crate::providers::supabase::clients::V1GetABranchArgs;
use crate::providers::supabase::clients::V1GetABranchConfigArgs;
use crate::providers::supabase::clients::V1GetAFunctionArgs;
use crate::providers::supabase::clients::V1GetAFunctionBodyArgs;
use crate::providers::supabase::clients::V1GetAMigrationArgs;
use crate::providers::supabase::clients::V1GetASnippetArgs;
use crate::providers::supabase::clients::V1GetASsoProviderArgs;
use crate::providers::supabase::clients::V1GetActionRunArgs;
use crate::providers::supabase::clients::V1GetActionRunLogsArgs;
use crate::providers::supabase::clients::V1GetAllProjectsForOrganizationArgs;
use crate::providers::supabase::clients::V1GetAnOrganizationArgs;
use crate::providers::supabase::clients::V1GetAuthServiceConfigArgs;
use crate::providers::supabase::clients::V1GetAvailableRegionsArgs;
use crate::providers::supabase::clients::V1GetDatabaseDiskArgs;
use crate::providers::supabase::clients::V1GetDatabaseMetadataArgs;
use crate::providers::supabase::clients::V1GetDatabaseOpenapiArgs;
use crate::providers::supabase::clients::V1GetDiskUtilizationArgs;
use crate::providers::supabase::clients::V1GetHostnameConfigArgs;
use crate::providers::supabase::clients::V1GetJitAccessArgs;
use crate::providers::supabase::clients::V1GetJitAccessConfigArgs;
use crate::providers::supabase::clients::V1GetLegacySigningKeyArgs;
use crate::providers::supabase::clients::V1GetNetworkRestrictionsArgs;
use crate::providers::supabase::clients::V1GetOrganizationEntitlementsArgs;
use crate::providers::supabase::clients::V1GetOrganizationProjectClaimArgs;
use crate::providers::supabase::clients::V1GetPerformanceAdvisorsArgs;
use crate::providers::supabase::clients::V1GetPgsodiumConfigArgs;
use crate::providers::supabase::clients::V1GetPoolerConfigArgs;
use crate::providers::supabase::clients::V1GetPostgresConfigArgs;
use crate::providers::supabase::clients::V1GetPostgresUpgradeEligibilityArgs;
use crate::providers::supabase::clients::V1GetPostgresUpgradeStatusArgs;
use crate::providers::supabase::clients::V1GetPostgrestServiceConfigArgs;
use crate::providers::supabase::clients::V1GetProjectApiKeyArgs;
use crate::providers::supabase::clients::V1GetProjectApiKeysArgs;
use crate::providers::supabase::clients::V1GetProjectArgs;
use crate::providers::supabase::clients::V1GetProjectClaimTokenArgs;
use crate::providers::supabase::clients::V1GetProjectDiskAutoscaleConfigArgs;
use crate::providers::supabase::clients::V1GetProjectFunctionCombinedStatsArgs;
use crate::providers::supabase::clients::V1GetProjectLegacyApiKeysArgs;
use crate::providers::supabase::clients::V1GetProjectLogsArgs;
use crate::providers::supabase::clients::V1GetProjectPgbouncerConfigArgs;
use crate::providers::supabase::clients::V1GetProjectSigningKeyArgs;
use crate::providers::supabase::clients::V1GetProjectSigningKeysArgs;
use crate::providers::supabase::clients::V1GetProjectTpaIntegrationArgs;
use crate::providers::supabase::clients::V1GetProjectUsageApiCountArgs;
use crate::providers::supabase::clients::V1GetProjectUsageRequestCountArgs;
use crate::providers::supabase::clients::V1GetReadonlyModeStatusArgs;
use crate::providers::supabase::clients::V1GetRealtimeConfigArgs;
use crate::providers::supabase::clients::V1GetRestorePointArgs;
use crate::providers::supabase::clients::V1GetSecurityAdvisorsArgs;
use crate::providers::supabase::clients::V1GetServicesHealthArgs;
use crate::providers::supabase::clients::V1GetSslEnforcementConfigArgs;
use crate::providers::supabase::clients::V1GetStorageConfigArgs;
use crate::providers::supabase::clients::V1GetVanitySubdomainConfigArgs;
use crate::providers::supabase::clients::V1ListActionRunsArgs;
use crate::providers::supabase::clients::V1ListAllBackupsArgs;
use crate::providers::supabase::clients::V1ListAllBranchesArgs;
use crate::providers::supabase::clients::V1ListAllBucketsArgs;
use crate::providers::supabase::clients::V1ListAllFunctionsArgs;
use crate::providers::supabase::clients::V1ListAllNetworkBansArgs;
use crate::providers::supabase::clients::V1ListAllNetworkBansEnrichedArgs;
use crate::providers::supabase::clients::V1ListAllSecretsArgs;
use crate::providers::supabase::clients::V1ListAllSnippetsArgs;
use crate::providers::supabase::clients::V1ListAllSsoProviderArgs;
use crate::providers::supabase::clients::V1ListAvailableRestoreVersionsArgs;
use crate::providers::supabase::clients::V1ListJitAccessArgs;
use crate::providers::supabase::clients::V1ListMigrationHistoryArgs;
use crate::providers::supabase::clients::V1ListOrganizationMembersArgs;
use crate::providers::supabase::clients::V1ListProjectAddonsArgs;
use crate::providers::supabase::clients::V1ListProjectTpaIntegrationsArgs;
use crate::providers::supabase::clients::V1MergeABranchArgs;
use crate::providers::supabase::clients::V1ModifyDatabaseDiskArgs;
use crate::providers::supabase::clients::V1OauthAuthorizeProjectClaimArgs;
use crate::providers::supabase::clients::V1PatchAMigrationArgs;
use crate::providers::supabase::clients::V1PatchNetworkRestrictionsArgs;
use crate::providers::supabase::clients::V1PauseAProjectArgs;
use crate::providers::supabase::clients::V1PushABranchArgs;
use crate::providers::supabase::clients::V1ReadOnlyQueryArgs;
use crate::providers::supabase::clients::V1RemoveAReadReplicaArgs;
use crate::providers::supabase::clients::V1RemoveProjectAddonArgs;
use crate::providers::supabase::clients::V1RemoveProjectSigningKeyArgs;
use crate::providers::supabase::clients::V1ResetABranchArgs;
use crate::providers::supabase::clients::V1RestoreABranchArgs;
use crate::providers::supabase::clients::V1RestoreAProjectArgs;
use crate::providers::supabase::clients::V1RestorePitrBackupArgs;
use crate::providers::supabase::clients::V1RevokeTokenArgs;
use crate::providers::supabase::clients::V1RollbackMigrationsArgs;
use crate::providers::supabase::clients::V1RunAQueryArgs;
use crate::providers::supabase::clients::V1SetupAReadReplicaArgs;
use crate::providers::supabase::clients::V1ShutdownRealtimeArgs;
use crate::providers::supabase::clients::V1UndoArgs;
use crate::providers::supabase::clients::V1UpdateABranchConfigArgs;
use crate::providers::supabase::clients::V1UpdateAFunctionArgs;
use crate::providers::supabase::clients::V1UpdateAProjectArgs;
use crate::providers::supabase::clients::V1UpdateASsoProviderArgs;
use crate::providers::supabase::clients::V1UpdateActionRunStatusArgs;
use crate::providers::supabase::clients::V1UpdateAuthServiceConfigArgs;
use crate::providers::supabase::clients::V1UpdateDatabasePasswordArgs;
use crate::providers::supabase::clients::V1UpdateHostnameConfigArgs;
use crate::providers::supabase::clients::V1UpdateJitAccessArgs;
use crate::providers::supabase::clients::V1UpdateJitAccessConfigArgs;
use crate::providers::supabase::clients::V1UpdateNetworkRestrictionsArgs;
use crate::providers::supabase::clients::V1UpdatePgsodiumConfigArgs;
use crate::providers::supabase::clients::V1UpdatePoolerConfigArgs;
use crate::providers::supabase::clients::V1UpdatePostgresConfigArgs;
use crate::providers::supabase::clients::V1UpdatePostgrestServiceConfigArgs;
use crate::providers::supabase::clients::V1UpdateProjectApiKeyArgs;
use crate::providers::supabase::clients::V1UpdateProjectLegacyApiKeysArgs;
use crate::providers::supabase::clients::V1UpdateProjectSigningKeyArgs;
use crate::providers::supabase::clients::V1UpdateRealtimeConfigArgs;
use crate::providers::supabase::clients::V1UpdateSslEnforcementConfigArgs;
use crate::providers::supabase::clients::V1UpdateStorageConfigArgs;
use crate::providers::supabase::clients::V1UpgradePostgresVersionArgs;
use crate::providers::supabase::clients::V1UpsertAMigrationArgs;
use crate::providers::supabase::clients::V1VerifyDnsConfigArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// SupabaseProvider with automatic state tracking.
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
/// let provider = SupabaseProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct SupabaseProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> SupabaseProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new SupabaseProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new SupabaseProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// V1 get a branch config.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BranchDetailResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_a_branch_config(
        &self,
        args: &V1GetABranchConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BranchDetailResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_a_branch_config_builder(
            &self.http_client,
            &args.branch_id_or_ref,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_a_branch_config_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// V1 diff a branch.
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
    pub fn v1_diff_a_branch(
        &self,
        args: &V1DiffABranchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_diff_a_branch_builder(
            &self.http_client,
            &args.branch_id_or_ref,
            &args.included_schemas,
            &args.pgdelta,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_diff_a_branch_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// V1 authorize user.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn v1_authorize_user(
        &self,
        args: &V1AuthorizeUserArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_authorize_user_builder(
            &self.http_client,
            &args.client_id,
            &args.response_type,
            &args.redirect_uri,
            &args.scope,
            &args.state,
            &args.response_mode,
            &args.code_challenge,
            &args.code_challenge_method,
            &args.organization_slug,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_authorize_user_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 oauth authorize project claim.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn v1_oauth_authorize_project_claim(
        &self,
        args: &V1OauthAuthorizeProjectClaimArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_oauth_authorize_project_claim_builder(
            &self.http_client,
            &args.project_ref,
            &args.client_id,
            &args.response_type,
            &args.redirect_uri,
            &args.state,
            &args.response_mode,
            &args.code_challenge,
            &args.code_challenge_method,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_oauth_authorize_project_claim_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// V1 list all organizations.
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
    pub fn v1_list_all_organizations(
        &self,
        args: &V1ListAllOrganizationsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_list_all_organizations_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_list_all_organizations_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// V1 get an organization.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the V1OrganizationSlugResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_an_organization(
        &self,
        args: &V1GetAnOrganizationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<V1OrganizationSlugResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_an_organization_builder(
            &self.http_client,
            &args.slug,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_an_organization_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 get organization entitlements.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the V1ListEntitlementsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_organization_entitlements(
        &self,
        args: &V1GetOrganizationEntitlementsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<V1ListEntitlementsResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_organization_entitlements_builder(
            &self.http_client,
            &args.slug,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_organization_entitlements_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 list organization members.
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
    pub fn v1_list_organization_members(
        &self,
        args: &V1ListOrganizationMembersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_list_organization_members_builder(
            &self.http_client,
            &args.slug,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_list_organization_members_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 get organization project claim.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the OrganizationProjectClaimResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_organization_project_claim(
        &self,
        args: &V1GetOrganizationProjectClaimArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<OrganizationProjectClaimResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_organization_project_claim_builder(
            &self.http_client,
            &args.slug,
            &args.token,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_organization_project_claim_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// V1 get all projects for organization.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the OrganizationProjectsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_all_projects_for_organization(
        &self,
        args: &V1GetAllProjectsForOrganizationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<OrganizationProjectsResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_all_projects_for_organization_builder(
            &self.http_client,
            &args.slug,
            &args.offset,
            &args.limit,
            &args.search,
            &args.sort,
            &args.statuses,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_all_projects_for_organization_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 get profile.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the V1ProfileResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_profile(
        &self,
        args: &V1GetProfileArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<V1ProfileResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_profile_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_profile_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 list all projects.
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
    pub fn v1_list_all_projects(
        &self,
        args: &V1ListAllProjectsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_list_all_projects_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_list_all_projects_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// V1 get available regions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RegionsInfo result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_available_regions(
        &self,
        args: &V1GetAvailableRegionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RegionsInfo, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_available_regions_builder(
            &self.http_client,
            &args.organization_slug,
            &args.continent,
            &args.desired_instance_size,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_available_regions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 get project.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the V1ProjectWithDatabaseResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_project(
        &self,
        args: &V1GetProjectArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<V1ProjectWithDatabaseResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_project_builder(
            &self.http_client,
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_project_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
            &args.ref_rs,
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
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_delete_a_project_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 list action runs.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListActionRunResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn v1_list_action_runs(
        &self,
        args: &V1ListActionRunsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListActionRunResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_list_action_runs_builder(
            &self.http_client,
            &args.ref_rs,
            &args.offset,
            &args.limit,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_list_action_runs_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 get action run.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ActionRunResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_action_run(
        &self,
        args: &V1GetActionRunArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ActionRunResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_action_run_builder(
            &self.http_client,
            &args.ref_rs,
            &args.run_id,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_action_run_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 get action run logs.
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
    pub fn v1_get_action_run_logs(
        &self,
        args: &V1GetActionRunLogsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_action_run_logs_builder(
            &self.http_client,
            &args.ref_rs,
            &args.run_id,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_action_run_logs_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
            &args.ref_rs,
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

    /// V1 get performance advisors.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the V1ProjectAdvisorsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_performance_advisors(
        &self,
        args: &V1GetPerformanceAdvisorsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<V1ProjectAdvisorsResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_performance_advisors_builder(
            &self.http_client,
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_performance_advisors_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 get security advisors.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the V1ProjectAdvisorsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_security_advisors(
        &self,
        args: &V1GetSecurityAdvisorsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<V1ProjectAdvisorsResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_security_advisors_builder(
            &self.http_client,
            &args.ref_rs,
            &args.lint_type,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_security_advisors_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 get project function combined stats.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AnalyticsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_project_function_combined_stats(
        &self,
        args: &V1GetProjectFunctionCombinedStatsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AnalyticsResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_project_function_combined_stats_builder(
            &self.http_client,
            &args.ref_rs,
            &args.interval,
            &args.function_id,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_project_function_combined_stats_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 get project logs.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AnalyticsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_project_logs(
        &self,
        args: &V1GetProjectLogsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AnalyticsResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_project_logs_builder(
            &self.http_client,
            &args.ref_rs,
            &args.sql,
            &args.iso_timestamp_start,
            &args.iso_timestamp_end,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_project_logs_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 get project usage api count.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the V1GetUsageApiCountResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_project_usage_api_count(
        &self,
        args: &V1GetProjectUsageApiCountArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<V1GetUsageApiCountResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_project_usage_api_count_builder(
            &self.http_client,
            &args.ref_rs,
            &args.interval,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_project_usage_api_count_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 get project usage request count.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the V1GetUsageApiRequestsCountResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_project_usage_request_count(
        &self,
        args: &V1GetProjectUsageRequestCountArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<V1GetUsageApiRequestsCountResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_project_usage_request_count_builder(
            &self.http_client,
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_project_usage_request_count_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 get project api keys.
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
    pub fn v1_get_project_api_keys(
        &self,
        args: &V1GetProjectApiKeysArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_project_api_keys_builder(
            &self.http_client,
            &args.ref_rs,
            &args.reveal,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_project_api_keys_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
            &args.ref_rs,
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

    /// V1 get project legacy api keys.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_project_legacy_api_keys(
        &self,
        args: &V1GetProjectLegacyApiKeysArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LegacyApiKeysResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_project_legacy_api_keys_builder(
            &self.http_client,
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_project_legacy_api_keys_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
            &args.ref_rs,
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

    /// V1 get project api key.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_project_api_key(
        &self,
        args: &V1GetProjectApiKeyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApiKeyResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_project_api_key_builder(
            &self.http_client,
            &args.ref_rs,
            &args.id,
            &args.reveal,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_project_api_key_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
            &args.ref_rs,
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
            &args.ref_rs,
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

    /// V1 list project addons.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListProjectAddonsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn v1_list_project_addons(
        &self,
        args: &V1ListProjectAddonsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListProjectAddonsResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_list_project_addons_builder(
            &self.http_client,
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_list_project_addons_task(builder)
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
            &args.ref_rs,
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
            &args.ref_rs,
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

    /// V1 list all branches.
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
    pub fn v1_list_all_branches(
        &self,
        args: &V1ListAllBranchesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_list_all_branches_builder(
            &self.http_client,
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_list_all_branches_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
            &args.ref_rs,
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
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_disable_preview_branching_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 get a branch.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_a_branch(
        &self,
        args: &V1GetABranchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BranchResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_a_branch_builder(
            &self.http_client,
            &args.ref_rs,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_a_branch_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 get project claim token.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProjectClaimTokenResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_project_claim_token(
        &self,
        args: &V1GetProjectClaimTokenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProjectClaimTokenResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_project_claim_token_builder(
            &self.http_client,
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_project_claim_token_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
            &args.ref_rs,
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
            &args.ref_rs,
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
            &args.ref_rs,
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
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_delete_login_roles_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 get auth service config.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_auth_service_config(
        &self,
        args: &V1GetAuthServiceConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AuthConfigResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_auth_service_config_builder(
            &self.http_client,
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_auth_service_config_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_update_auth_service_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 get project signing keys.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SigningKeysResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_project_signing_keys(
        &self,
        args: &V1GetProjectSigningKeysArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SigningKeysResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_project_signing_keys_builder(
            &self.http_client,
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_project_signing_keys_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_create_project_signing_key_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 get legacy signing key.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_legacy_signing_key(
        &self,
        args: &V1GetLegacySigningKeyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SigningKeyResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_legacy_signing_key_builder(
            &self.http_client,
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_legacy_signing_key_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_create_legacy_signing_key_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 get project signing key.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_project_signing_key(
        &self,
        args: &V1GetProjectSigningKeyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SigningKeyResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_project_signing_key_builder(
            &self.http_client,
            &args.id,
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_project_signing_key_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
            &args.ref_rs,
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
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_remove_project_signing_key_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 list all sso provider.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListProvidersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn v1_list_all_sso_provider(
        &self,
        args: &V1ListAllSsoProviderArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListProvidersResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_list_all_sso_provider_builder(
            &self.http_client,
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_list_all_sso_provider_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_create_a_sso_provider_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 get a sso provider.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GetProviderResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_a_sso_provider(
        &self,
        args: &V1GetASsoProviderArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GetProviderResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_a_sso_provider_builder(
            &self.http_client,
            &args.ref_rs,
            &args.provider_id,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_a_sso_provider_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
            &args.ref_rs,
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
            &args.ref_rs,
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

    /// V1 list project tpa integrations.
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
    pub fn v1_list_project_tpa_integrations(
        &self,
        args: &V1ListProjectTpaIntegrationsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_list_project_tpa_integrations_builder(
            &self.http_client,
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_list_project_tpa_integrations_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_create_project_tpa_integration_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 get project tpa integration.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_project_tpa_integration(
        &self,
        args: &V1GetProjectTpaIntegrationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ThirdPartyAuth, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_project_tpa_integration_builder(
            &self.http_client,
            &args.ref_rs,
            &args.tpa_id,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_project_tpa_integration_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
            &args.ref_rs,
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

    /// V1 get project pgbouncer config.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the V1PgbouncerConfigResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_project_pgbouncer_config(
        &self,
        args: &V1GetProjectPgbouncerConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<V1PgbouncerConfigResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_project_pgbouncer_config_builder(
            &self.http_client,
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_project_pgbouncer_config_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 get pooler config.
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
    pub fn v1_get_pooler_config(
        &self,
        args: &V1GetPoolerConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_pooler_config_builder(
            &self.http_client,
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_pooler_config_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_update_pooler_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 get postgres config.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_postgres_config(
        &self,
        args: &V1GetPostgresConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PostgresConfigResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_postgres_config_builder(
            &self.http_client,
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_postgres_config_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_update_postgres_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 get database disk.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DiskResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_database_disk(
        &self,
        args: &V1GetDatabaseDiskArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DiskResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_database_disk_builder(
            &self.http_client,
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_database_disk_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_modify_database_disk_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 get project disk autoscale config.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DiskAutoscaleConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_project_disk_autoscale_config(
        &self,
        args: &V1GetProjectDiskAutoscaleConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DiskAutoscaleConfig, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_project_disk_autoscale_config_builder(
            &self.http_client,
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_project_disk_autoscale_config_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 get disk utilization.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DiskUtilMetricsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_disk_utilization(
        &self,
        args: &V1GetDiskUtilizationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DiskUtilMetricsResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_disk_utilization_builder(
            &self.http_client,
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_disk_utilization_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 get realtime config.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RealtimeConfigResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_realtime_config(
        &self,
        args: &V1GetRealtimeConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RealtimeConfigResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_realtime_config_builder(
            &self.http_client,
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_realtime_config_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
            &args.ref_rs,
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
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_shutdown_realtime_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 get storage config.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the StorageConfigResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_storage_config(
        &self,
        args: &V1GetStorageConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<StorageConfigResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_storage_config_builder(
            &self.http_client,
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_storage_config_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_update_storage_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 get hostname config.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_hostname_config(
        &self,
        args: &V1GetHostnameConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UpdateCustomHostnameResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_hostname_config_builder(
            &self.http_client,
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_hostname_config_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
            &args.ref_rs,
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
            &args.ref_rs,
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
            &args.ref_rs,
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
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_verify_dns_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 list all backups.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the V1BackupsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn v1_list_all_backups(
        &self,
        args: &V1ListAllBackupsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<V1BackupsResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_list_all_backups_builder(
            &self.http_client,
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_list_all_backups_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_restore_pitr_backup_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 get restore point.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_restore_point(
        &self,
        args: &V1GetRestorePointArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<V1RestorePointResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_restore_point_builder(
            &self.http_client,
            &args.ref_rs,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_restore_point_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
            &args.ref_rs,
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
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_undo_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 get database metadata.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GetProjectDbMetadataResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_database_metadata(
        &self,
        args: &V1GetDatabaseMetadataArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GetProjectDbMetadataResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_database_metadata_builder(
            &self.http_client,
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_database_metadata_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 get jit access.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_jit_access(
        &self,
        args: &V1GetJitAccessArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<JitAccessResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_jit_access_builder(
            &self.http_client,
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_jit_access_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
            &args.ref_rs,
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
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_update_jit_access_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 list jit access.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the JitListAccessResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn v1_list_jit_access(
        &self,
        args: &V1ListJitAccessArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<JitListAccessResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_list_jit_access_builder(
            &self.http_client,
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_list_jit_access_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
            &args.ref_rs,
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

    /// V1 list migration history.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the V1ListMigrationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn v1_list_migration_history(
        &self,
        args: &V1ListMigrationHistoryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<V1ListMigrationsResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_list_migration_history_builder(
            &self.http_client,
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_list_migration_history_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
            &args.ref_rs,
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
            &args.ref_rs,
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
            &args.ref_rs,
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

    /// V1 get a migration.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the V1GetMigrationResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_a_migration(
        &self,
        args: &V1GetAMigrationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<V1GetMigrationResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_a_migration_builder(
            &self.http_client,
            &args.ref_rs,
            &args.version,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_a_migration_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
            &args.ref_rs,
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

    /// V1 get database openapi.
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
    pub fn v1_get_database_openapi(
        &self,
        args: &V1GetDatabaseOpenapiArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_database_openapi_builder(
            &self.http_client,
            &args.ref_rs,
            &args.schema,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_database_openapi_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
            &args.ref_rs,
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
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_run_a_query_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 read only query.
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
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_read_only_query_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_enable_database_webhook_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 list all functions.
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
    pub fn v1_list_all_functions(
        &self,
        args: &V1ListAllFunctionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_list_all_functions_builder(
            &self.http_client,
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_list_all_functions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
            &args.ref_rs,
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
            &args.ref_rs,
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
            &args.ref_rs,
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

    /// V1 get a function.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FunctionSlugResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_a_function(
        &self,
        args: &V1GetAFunctionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FunctionSlugResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_a_function_builder(
            &self.http_client,
            &args.ref_rs,
            &args.function_slug,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_a_function_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
            &args.ref_rs,
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
            &args.ref_rs,
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

    /// V1 get a function body.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the StreamableFile result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_a_function_body(
        &self,
        args: &V1GetAFunctionBodyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<StreamableFile, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_a_function_body_builder(
            &self.http_client,
            &args.ref_rs,
            &args.function_slug,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_a_function_body_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 get services health.
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
    pub fn v1_get_services_health(
        &self,
        args: &V1GetServicesHealthArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_services_health_builder(
            &self.http_client,
            &args.ref_rs,
            &args.services,
            &args.timeout_ms,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_services_health_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 get jit access config.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_jit_access_config(
        &self,
        args: &V1GetJitAccessConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<JitAccessResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_jit_access_config_builder(
            &self.http_client,
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_jit_access_config_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
            &args.ref_rs,
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
            &args.ref_rs,
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
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_list_all_network_bans_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 list all network bans enriched.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_list_all_network_bans_enriched_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 get network restrictions.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_network_restrictions(
        &self,
        args: &V1GetNetworkRestrictionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NetworkRestrictionsResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_network_restrictions_builder(
            &self.http_client,
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_network_restrictions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
            &args.ref_rs,
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
            &args.ref_rs,
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
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_pause_a_project_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 get pgsodium config.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_pgsodium_config(
        &self,
        args: &V1GetPgsodiumConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PgsodiumConfigResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_pgsodium_config_builder(
            &self.http_client,
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_pgsodium_config_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_update_pgsodium_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 get postgrest service config.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PostgrestConfigWithJWTSecretResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_postgrest_service_config(
        &self,
        args: &V1GetPostgrestServiceConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PostgrestConfigWithJWTSecretResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_postgrest_service_config_builder(
            &self.http_client,
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_postgrest_service_config_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
            &args.ref_rs,
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
            &args.ref_rs,
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
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_setup_a_read_replica_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 get readonly mode status.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ReadOnlyStatusResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_readonly_mode_status(
        &self,
        args: &V1GetReadonlyModeStatusArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ReadOnlyStatusResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_readonly_mode_status_builder(
            &self.http_client,
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_readonly_mode_status_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_disable_readonly_mode_temporarily_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 list available restore versions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GetProjectAvailableRestoreVersionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn v1_list_available_restore_versions(
        &self,
        args: &V1ListAvailableRestoreVersionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GetProjectAvailableRestoreVersionsResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_list_available_restore_versions_builder(
            &self.http_client,
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_list_available_restore_versions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
            &args.ref_rs,
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
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_cancel_a_project_restoration_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 list all secrets.
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
    pub fn v1_list_all_secrets(
        &self,
        args: &V1ListAllSecretsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_list_all_secrets_builder(
            &self.http_client,
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_list_all_secrets_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
            &args.ref_rs,
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
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_bulk_delete_secrets_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 get ssl enforcement config.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_ssl_enforcement_config(
        &self,
        args: &V1GetSslEnforcementConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SslEnforcementResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_ssl_enforcement_config_builder(
            &self.http_client,
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_ssl_enforcement_config_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_update_ssl_enforcement_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 list all buckets.
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
    pub fn v1_list_all_buckets(
        &self,
        args: &V1ListAllBucketsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_list_all_buckets_builder(
            &self.http_client,
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_list_all_buckets_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 generate typescript types.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TypescriptResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn v1_generate_typescript_types(
        &self,
        args: &V1GenerateTypescriptTypesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TypescriptResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_generate_typescript_types_builder(
            &self.http_client,
            &args.ref_rs,
            &args.included_schemas,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_generate_typescript_types_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_upgrade_postgres_version_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 get postgres upgrade eligibility.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProjectUpgradeEligibilityResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_postgres_upgrade_eligibility(
        &self,
        args: &V1GetPostgresUpgradeEligibilityArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProjectUpgradeEligibilityResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_postgres_upgrade_eligibility_builder(
            &self.http_client,
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_postgres_upgrade_eligibility_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 get postgres upgrade status.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DatabaseUpgradeStatusResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_postgres_upgrade_status(
        &self,
        args: &V1GetPostgresUpgradeStatusArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DatabaseUpgradeStatusResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_postgres_upgrade_status_builder(
            &self.http_client,
            &args.ref_rs,
            &args.tracking_id,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_postgres_upgrade_status_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 get vanity subdomain config.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the VanitySubdomainConfigResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_vanity_subdomain_config(
        &self,
        args: &V1GetVanitySubdomainConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<VanitySubdomainConfigResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_vanity_subdomain_config_builder(
            &self.http_client,
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_vanity_subdomain_config_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
            &args.ref_rs,
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
            &args.ref_rs,
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
            &args.ref_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_check_vanity_subdomain_availability_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 list all snippets.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SnippetList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn v1_list_all_snippets(
        &self,
        args: &V1ListAllSnippetsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SnippetList, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_list_all_snippets_builder(
            &self.http_client,
            &args.project_ref,
            &args.cursor,
            &args.limit,
            &args.sort_by,
            &args.sort_order,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_list_all_snippets_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// V1 get a snippet.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SnippetResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn v1_get_a_snippet(
        &self,
        args: &V1GetASnippetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SnippetResponse, ProviderError<ApiError>>,
            P = crate::providers::supabase::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = v1_get_a_snippet_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = v1_get_a_snippet_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
