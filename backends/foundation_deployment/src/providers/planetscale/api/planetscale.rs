//! PlanetscaleProvider - State-aware planetscale API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       planetscale API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "planetscale")]

use crate::providers::planetscale::clients::{
    list_organizations_builder, list_organizations_task,
    get_organization_builder, get_organization_task,
    update_organization_builder, update_organization_task,
    list_audit_logs_builder, list_audit_logs_task,
    list_cluster_size_skus_builder, list_cluster_size_skus_task,
    list_databases_builder, list_databases_task,
    create_database_builder, create_database_task,
    get_database_builder, get_database_task,
    update_database_settings_builder, update_database_settings_task,
    delete_database_builder, delete_database_task,
    list_backup_policies_builder, list_backup_policies_task,
    create_backup_policy_builder, create_backup_policy_task,
    get_backup_policy_builder, get_backup_policy_task,
    update_backup_policy_builder, update_backup_policy_task,
    delete_backup_policy_builder, delete_backup_policy_task,
    list_branches_builder, list_branches_task,
    create_branch_builder, create_branch_task,
    get_branch_builder, get_branch_task,
    update_branch_builder, update_branch_task,
    delete_branch_builder, delete_branch_task,
    list_backups_builder, list_backups_task,
    create_backup_builder, create_backup_task,
    get_backup_builder, get_backup_task,
    update_backup_builder, update_backup_task,
    delete_backup_builder, delete_backup_task,
    list_branch_bouncer_resize_requests_builder, list_branch_bouncer_resize_requests_task,
    list_bouncers_builder, list_bouncers_task,
    create_bouncer_builder, create_bouncer_task,
    get_bouncer_builder, get_bouncer_task,
    delete_bouncer_builder, delete_bouncer_task,
    list_bouncer_resize_requests_builder, list_bouncer_resize_requests_task,
    update_bouncer_resize_request_builder, update_bouncer_resize_request_task,
    cancel_bouncer_resize_request_builder, cancel_bouncer_resize_request_task,
    list_branch_change_requests_builder, list_branch_change_requests_task,
    update_branch_change_request_builder, update_branch_change_request_task,
    get_branch_change_request_builder, get_branch_change_request_task,
    update_branch_cluster_config_builder, update_branch_cluster_config_task,
    demote_branch_builder, demote_branch_task,
    list_extensions_builder, list_extensions_task,
    list_keyspaces_builder, list_keyspaces_task,
    create_keyspace_builder, create_keyspace_task,
    get_keyspace_builder, get_keyspace_task,
    update_keyspace_builder, update_keyspace_task,
    delete_keyspace_builder, delete_keyspace_task,
    get_keyspace_rollout_status_builder, get_keyspace_rollout_status_task,
    get_keyspace_vschema_builder, get_keyspace_vschema_task,
    update_keyspace_vschema_builder, update_keyspace_vschema_task,
    list_parameters_builder, list_parameters_task,
    list_passwords_builder, list_passwords_task,
    create_password_builder, create_password_task,
    get_password_builder, get_password_task,
    update_password_builder, update_password_task,
    delete_password_builder, delete_password_task,
    renew_password_builder, renew_password_task,
    promote_branch_builder, promote_branch_task,
    list_generated_query_patterns_reports_builder, list_generated_query_patterns_reports_task,
    create_query_patterns_report_builder, create_query_patterns_report_task,
    get_query_patterns_report_status_builder, get_query_patterns_report_status_task,
    delete_query_patterns_report_builder, delete_query_patterns_report_task,
    get_query_patterns_report_builder, get_query_patterns_report_task,
    cancel_branch_change_request_builder, cancel_branch_change_request_task,
    list_roles_builder, list_roles_task,
    create_role_builder, create_role_task,
    get_default_role_builder, get_default_role_task,
    reset_default_role_builder, reset_default_role_task,
    get_role_builder, get_role_task,
    update_role_builder, update_role_task,
    delete_role_builder, delete_role_task,
    reassign_role_objects_builder, reassign_role_objects_task,
    renew_role_builder, renew_role_task,
    reset_role_builder, reset_role_task,
    enable_safe_migrations_builder, enable_safe_migrations_task,
    disable_safe_migrations_builder, disable_safe_migrations_task,
    get_branch_schema_builder, get_branch_schema_task,
    lint_branch_schema_builder, lint_branch_schema_task,
    list_traffic_budgets_builder, list_traffic_budgets_task,
    create_traffic_budget_builder, create_traffic_budget_task,
    create_traffic_rule_builder, create_traffic_rule_task,
    delete_traffic_rule_builder, delete_traffic_rule_task,
    get_traffic_budget_builder, get_traffic_budget_task,
    update_traffic_budget_builder, update_traffic_budget_task,
    delete_traffic_budget_builder, delete_traffic_budget_task,
    list_database_postgres_cidrs_builder, list_database_postgres_cidrs_task,
    create_database_postgres_cidr_builder, create_database_postgres_cidr_task,
    get_database_postgres_cidr_builder, get_database_postgres_cidr_task,
    update_database_postgres_cidr_builder, update_database_postgres_cidr_task,
    delete_database_postgres_cidr_builder, delete_database_postgres_cidr_task,
    get_deploy_queue_builder, get_deploy_queue_task,
    list_deploy_requests_builder, list_deploy_requests_task,
    create_deploy_request_builder, create_deploy_request_task,
    get_deploy_request_builder, get_deploy_request_task,
    close_deploy_request_builder, close_deploy_request_task,
    complete_gated_deploy_request_builder, complete_gated_deploy_request_task,
    update_auto_apply_builder, update_auto_apply_task,
    update_auto_delete_branch_builder, update_auto_delete_branch_task,
    cancel_deploy_request_builder, cancel_deploy_request_task,
    complete_errored_deploy_builder, complete_errored_deploy_task,
    queue_deploy_request_builder, queue_deploy_request_task,
    get_deployment_builder, get_deployment_task,
    list_deploy_operations_builder, list_deploy_operations_task,
    complete_revert_builder, complete_revert_task,
    list_deploy_request_reviews_builder, list_deploy_request_reviews_task,
    review_deploy_request_builder, review_deploy_request_task,
    skip_revert_period_builder, skip_revert_period_task,
    check_deploy_request_storage_builder, check_deploy_request_storage_task,
    get_deploy_request_throttler_builder, get_deploy_request_throttler_task,
    update_deploy_request_throttler_builder, update_deploy_request_throttler_task,
    list_maintenance_schedules_builder, list_maintenance_schedules_task,
    get_maintenance_schedule_builder, get_maintenance_schedule_task,
    list_maintenance_windows_builder, list_maintenance_windows_task,
    list_read_only_regions_builder, list_read_only_regions_task,
    list_database_regions_builder, list_database_regions_task,
    list_schema_recommendations_builder, list_schema_recommendations_task,
    get_schema_recommendation_builder, get_schema_recommendation_task,
    dismiss_schema_recommendation_builder, dismiss_schema_recommendation_task,
    get_database_throttler_builder, get_database_throttler_task,
    update_database_throttler_builder, update_database_throttler_task,
    list_webhooks_builder, list_webhooks_task,
    create_webhook_builder, create_webhook_task,
    get_webhook_builder, get_webhook_task,
    update_webhook_builder, update_webhook_task,
    delete_webhook_builder, delete_webhook_task,
    test_webhook_builder, test_webhook_task,
    list_workflows_builder, list_workflows_task,
    create_workflow_builder, create_workflow_task,
    get_workflow_builder, get_workflow_task,
    workflow_cancel_builder, workflow_cancel_task,
    workflow_complete_builder, workflow_complete_task,
    workflow_cutover_builder, workflow_cutover_task,
    workflow_retry_builder, workflow_retry_task,
    workflow_reverse_cutover_builder, workflow_reverse_cutover_task,
    workflow_reverse_traffic_builder, workflow_reverse_traffic_task,
    workflow_switch_primaries_builder, workflow_switch_primaries_task,
    workflow_switch_replicas_builder, workflow_switch_replicas_task,
    verify_workflow_builder, verify_workflow_task,
    list_invoices_builder, list_invoices_task,
    get_invoice_builder, get_invoice_task,
    get_invoice_line_items_builder, get_invoice_line_items_task,
    list_organization_members_builder, list_organization_members_task,
    get_organization_membership_builder, get_organization_membership_task,
    update_organization_membership_builder, update_organization_membership_task,
    remove_organization_member_builder, remove_organization_member_task,
    list_oauth_applications_builder, list_oauth_applications_task,
    get_oauth_application_builder, get_oauth_application_task,
    list_oauth_tokens_builder, list_oauth_tokens_task,
    get_oauth_token_builder, get_oauth_token_task,
    delete_oauth_token_builder, delete_oauth_token_task,
    create_oauth_token_builder, create_oauth_token_task,
    list_regions_for_organization_builder, list_regions_for_organization_task,
    list_service_tokens_builder, list_service_tokens_task,
    create_service_token_builder, create_service_token_task,
    get_service_token_builder, get_service_token_task,
    delete_service_token_builder, delete_service_token_task,
    list_organization_teams_builder, list_organization_teams_task,
    create_organization_team_builder, create_organization_team_task,
    get_organization_team_builder, get_organization_team_task,
    update_organization_team_builder, update_organization_team_task,
    delete_organization_team_builder, delete_organization_team_task,
    list_organization_team_members_builder, list_organization_team_members_task,
    add_organization_team_member_builder, add_organization_team_member_task,
    get_organization_team_member_builder, get_organization_team_member_task,
    remove_organization_team_member_builder, remove_organization_team_member_task,
    list_public_regions_builder, list_public_regions_task,
    get_current_user_builder, get_current_user_task,
};
use crate::providers::planetscale::clients::types::{ApiError, ApiPending};
use crate::providers::planetscale::clients::AddOrganizationTeamMemberArgs;
use crate::providers::planetscale::clients::CancelBouncerResizeRequestArgs;
use crate::providers::planetscale::clients::CancelBranchChangeRequestArgs;
use crate::providers::planetscale::clients::CancelDeployRequestArgs;
use crate::providers::planetscale::clients::CheckDeployRequestStorageArgs;
use crate::providers::planetscale::clients::CloseDeployRequestArgs;
use crate::providers::planetscale::clients::CompleteErroredDeployArgs;
use crate::providers::planetscale::clients::CompleteGatedDeployRequestArgs;
use crate::providers::planetscale::clients::CompleteRevertArgs;
use crate::providers::planetscale::clients::CreateBackupArgs;
use crate::providers::planetscale::clients::CreateBackupPolicyArgs;
use crate::providers::planetscale::clients::CreateBouncerArgs;
use crate::providers::planetscale::clients::CreateBranchArgs;
use crate::providers::planetscale::clients::CreateDatabaseArgs;
use crate::providers::planetscale::clients::CreateDatabasePostgresCidrArgs;
use crate::providers::planetscale::clients::CreateDeployRequestArgs;
use crate::providers::planetscale::clients::CreateKeyspaceArgs;
use crate::providers::planetscale::clients::CreateOauthTokenArgs;
use crate::providers::planetscale::clients::CreateOrganizationTeamArgs;
use crate::providers::planetscale::clients::CreatePasswordArgs;
use crate::providers::planetscale::clients::CreateQueryPatternsReportArgs;
use crate::providers::planetscale::clients::CreateRoleArgs;
use crate::providers::planetscale::clients::CreateServiceTokenArgs;
use crate::providers::planetscale::clients::CreateTrafficBudgetArgs;
use crate::providers::planetscale::clients::CreateTrafficRuleArgs;
use crate::providers::planetscale::clients::CreateWebhookArgs;
use crate::providers::planetscale::clients::CreateWorkflowArgs;
use crate::providers::planetscale::clients::DeleteBackupArgs;
use crate::providers::planetscale::clients::DeleteBackupPolicyArgs;
use crate::providers::planetscale::clients::DeleteBouncerArgs;
use crate::providers::planetscale::clients::DeleteBranchArgs;
use crate::providers::planetscale::clients::DeleteDatabaseArgs;
use crate::providers::planetscale::clients::DeleteDatabasePostgresCidrArgs;
use crate::providers::planetscale::clients::DeleteKeyspaceArgs;
use crate::providers::planetscale::clients::DeleteOauthTokenArgs;
use crate::providers::planetscale::clients::DeleteOrganizationTeamArgs;
use crate::providers::planetscale::clients::DeletePasswordArgs;
use crate::providers::planetscale::clients::DeleteQueryPatternsReportArgs;
use crate::providers::planetscale::clients::DeleteRoleArgs;
use crate::providers::planetscale::clients::DeleteServiceTokenArgs;
use crate::providers::planetscale::clients::DeleteTrafficBudgetArgs;
use crate::providers::planetscale::clients::DeleteTrafficRuleArgs;
use crate::providers::planetscale::clients::DeleteWebhookArgs;
use crate::providers::planetscale::clients::DemoteBranchArgs;
use crate::providers::planetscale::clients::DisableSafeMigrationsArgs;
use crate::providers::planetscale::clients::DismissSchemaRecommendationArgs;
use crate::providers::planetscale::clients::EnableSafeMigrationsArgs;
use crate::providers::planetscale::clients::GetBackupArgs;
use crate::providers::planetscale::clients::GetBackupPolicyArgs;
use crate::providers::planetscale::clients::GetBouncerArgs;
use crate::providers::planetscale::clients::GetBranchArgs;
use crate::providers::planetscale::clients::GetBranchChangeRequestArgs;
use crate::providers::planetscale::clients::GetBranchSchemaArgs;
use crate::providers::planetscale::clients::GetDatabaseArgs;
use crate::providers::planetscale::clients::GetDatabasePostgresCidrArgs;
use crate::providers::planetscale::clients::GetDatabaseThrottlerArgs;
use crate::providers::planetscale::clients::GetDefaultRoleArgs;
use crate::providers::planetscale::clients::GetDeployQueueArgs;
use crate::providers::planetscale::clients::GetDeployRequestArgs;
use crate::providers::planetscale::clients::GetDeployRequestThrottlerArgs;
use crate::providers::planetscale::clients::GetDeploymentArgs;
use crate::providers::planetscale::clients::GetInvoiceArgs;
use crate::providers::planetscale::clients::GetInvoiceLineItemsArgs;
use crate::providers::planetscale::clients::GetKeyspaceArgs;
use crate::providers::planetscale::clients::GetKeyspaceRolloutStatusArgs;
use crate::providers::planetscale::clients::GetKeyspaceVschemaArgs;
use crate::providers::planetscale::clients::GetMaintenanceScheduleArgs;
use crate::providers::planetscale::clients::GetOauthApplicationArgs;
use crate::providers::planetscale::clients::GetOauthTokenArgs;
use crate::providers::planetscale::clients::GetOrganizationArgs;
use crate::providers::planetscale::clients::GetOrganizationMembershipArgs;
use crate::providers::planetscale::clients::GetOrganizationTeamArgs;
use crate::providers::planetscale::clients::GetOrganizationTeamMemberArgs;
use crate::providers::planetscale::clients::GetPasswordArgs;
use crate::providers::planetscale::clients::GetQueryPatternsReportArgs;
use crate::providers::planetscale::clients::GetQueryPatternsReportStatusArgs;
use crate::providers::planetscale::clients::GetRoleArgs;
use crate::providers::planetscale::clients::GetSchemaRecommendationArgs;
use crate::providers::planetscale::clients::GetServiceTokenArgs;
use crate::providers::planetscale::clients::GetTrafficBudgetArgs;
use crate::providers::planetscale::clients::GetWebhookArgs;
use crate::providers::planetscale::clients::GetWorkflowArgs;
use crate::providers::planetscale::clients::LintBranchSchemaArgs;
use crate::providers::planetscale::clients::ListAuditLogsArgs;
use crate::providers::planetscale::clients::ListBackupPoliciesArgs;
use crate::providers::planetscale::clients::ListBackupsArgs;
use crate::providers::planetscale::clients::ListBouncerResizeRequestsArgs;
use crate::providers::planetscale::clients::ListBouncersArgs;
use crate::providers::planetscale::clients::ListBranchBouncerResizeRequestsArgs;
use crate::providers::planetscale::clients::ListBranchChangeRequestsArgs;
use crate::providers::planetscale::clients::ListBranchesArgs;
use crate::providers::planetscale::clients::ListClusterSizeSkusArgs;
use crate::providers::planetscale::clients::ListDatabasePostgresCidrsArgs;
use crate::providers::planetscale::clients::ListDatabaseRegionsArgs;
use crate::providers::planetscale::clients::ListDatabasesArgs;
use crate::providers::planetscale::clients::ListDeployOperationsArgs;
use crate::providers::planetscale::clients::ListDeployRequestReviewsArgs;
use crate::providers::planetscale::clients::ListDeployRequestsArgs;
use crate::providers::planetscale::clients::ListExtensionsArgs;
use crate::providers::planetscale::clients::ListGeneratedQueryPatternsReportsArgs;
use crate::providers::planetscale::clients::ListInvoicesArgs;
use crate::providers::planetscale::clients::ListKeyspacesArgs;
use crate::providers::planetscale::clients::ListMaintenanceSchedulesArgs;
use crate::providers::planetscale::clients::ListMaintenanceWindowsArgs;
use crate::providers::planetscale::clients::ListOauthApplicationsArgs;
use crate::providers::planetscale::clients::ListOauthTokensArgs;
use crate::providers::planetscale::clients::ListOrganizationMembersArgs;
use crate::providers::planetscale::clients::ListOrganizationTeamMembersArgs;
use crate::providers::planetscale::clients::ListOrganizationTeamsArgs;
use crate::providers::planetscale::clients::ListOrganizationsArgs;
use crate::providers::planetscale::clients::ListParametersArgs;
use crate::providers::planetscale::clients::ListPasswordsArgs;
use crate::providers::planetscale::clients::ListPublicRegionsArgs;
use crate::providers::planetscale::clients::ListReadOnlyRegionsArgs;
use crate::providers::planetscale::clients::ListRegionsForOrganizationArgs;
use crate::providers::planetscale::clients::ListRolesArgs;
use crate::providers::planetscale::clients::ListSchemaRecommendationsArgs;
use crate::providers::planetscale::clients::ListServiceTokensArgs;
use crate::providers::planetscale::clients::ListTrafficBudgetsArgs;
use crate::providers::planetscale::clients::ListWebhooksArgs;
use crate::providers::planetscale::clients::ListWorkflowsArgs;
use crate::providers::planetscale::clients::PromoteBranchArgs;
use crate::providers::planetscale::clients::QueueDeployRequestArgs;
use crate::providers::planetscale::clients::ReassignRoleObjectsArgs;
use crate::providers::planetscale::clients::RemoveOrganizationMemberArgs;
use crate::providers::planetscale::clients::RemoveOrganizationTeamMemberArgs;
use crate::providers::planetscale::clients::RenewPasswordArgs;
use crate::providers::planetscale::clients::RenewRoleArgs;
use crate::providers::planetscale::clients::ResetDefaultRoleArgs;
use crate::providers::planetscale::clients::ResetRoleArgs;
use crate::providers::planetscale::clients::ReviewDeployRequestArgs;
use crate::providers::planetscale::clients::SkipRevertPeriodArgs;
use crate::providers::planetscale::clients::TestWebhookArgs;
use crate::providers::planetscale::clients::UpdateAutoApplyArgs;
use crate::providers::planetscale::clients::UpdateAutoDeleteBranchArgs;
use crate::providers::planetscale::clients::UpdateBackupArgs;
use crate::providers::planetscale::clients::UpdateBackupPolicyArgs;
use crate::providers::planetscale::clients::UpdateBouncerResizeRequestArgs;
use crate::providers::planetscale::clients::UpdateBranchArgs;
use crate::providers::planetscale::clients::UpdateBranchChangeRequestArgs;
use crate::providers::planetscale::clients::UpdateBranchClusterConfigArgs;
use crate::providers::planetscale::clients::UpdateDatabasePostgresCidrArgs;
use crate::providers::planetscale::clients::UpdateDatabaseSettingsArgs;
use crate::providers::planetscale::clients::UpdateDatabaseThrottlerArgs;
use crate::providers::planetscale::clients::UpdateDeployRequestThrottlerArgs;
use crate::providers::planetscale::clients::UpdateKeyspaceArgs;
use crate::providers::planetscale::clients::UpdateKeyspaceVschemaArgs;
use crate::providers::planetscale::clients::UpdateOrganizationArgs;
use crate::providers::planetscale::clients::UpdateOrganizationMembershipArgs;
use crate::providers::planetscale::clients::UpdateOrganizationTeamArgs;
use crate::providers::planetscale::clients::UpdatePasswordArgs;
use crate::providers::planetscale::clients::UpdateRoleArgs;
use crate::providers::planetscale::clients::UpdateTrafficBudgetArgs;
use crate::providers::planetscale::clients::UpdateWebhookArgs;
use crate::providers::planetscale::clients::VerifyWorkflowArgs;
use crate::providers::planetscale::clients::WorkflowCancelArgs;
use crate::providers::planetscale::clients::WorkflowCompleteArgs;
use crate::providers::planetscale::clients::WorkflowCutoverArgs;
use crate::providers::planetscale::clients::WorkflowRetryArgs;
use crate::providers::planetscale::clients::WorkflowReverseCutoverArgs;
use crate::providers::planetscale::clients::WorkflowReverseTrafficArgs;
use crate::providers::planetscale::clients::WorkflowSwitchPrimariesArgs;
use crate::providers::planetscale::clients::WorkflowSwitchReplicasArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// PlanetscaleProvider with automatic state tracking.
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
/// let provider = PlanetscaleProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct PlanetscaleProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> PlanetscaleProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new PlanetscaleProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new PlanetscaleProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// List organizations.
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
    pub fn list_organizations(
        &self,
        args: &ListOrganizationsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_organizations_builder(
            &self.http_client,
            &args.page,
            &args.per_page,
        )
        .map_err(ProviderError::Api)?;

        let task = list_organizations_task(builder)
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
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_organization(
        &self,
        args: &GetOrganizationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_organization_builder(
            &self.http_client,
            &args.organization,
        )
        .map_err(ProviderError::Api)?;

        let task = get_organization_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Update organization.
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
    pub fn update_organization(
        &self,
        args: &UpdateOrganizationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = update_organization_builder(
            &self.http_client,
            &args.organization,
        )
        .map_err(ProviderError::Api)?;

        let task = update_organization_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// List audit logs.
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
    pub fn list_audit_logs(
        &self,
        args: &ListAuditLogsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_audit_logs_builder(
            &self.http_client,
            &args.organization,
            &args.starting_after,
            &args.ending_before,
            &args.limit,
        )
        .map_err(ProviderError::Api)?;

        let task = list_audit_logs_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// List cluster size skus.
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
    pub fn list_cluster_size_skus(
        &self,
        args: &ListClusterSizeSkusArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_cluster_size_skus_builder(
            &self.http_client,
            &args.organization,
            &args.engine,
            &args.rates,
            &args.region,
        )
        .map_err(ProviderError::Api)?;

        let task = list_cluster_size_skus_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// List databases.
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
    pub fn list_databases(
        &self,
        args: &ListDatabasesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_databases_builder(
            &self.http_client,
            &args.organization,
            &args.q,
            &args.page,
            &args.per_page,
        )
        .map_err(ProviderError::Api)?;

        let task = list_databases_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Create database.
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
    pub fn create_database(
        &self,
        args: &CreateDatabaseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = create_database_builder(
            &self.http_client,
            &args.organization,
        )
        .map_err(ProviderError::Api)?;

        let task = create_database_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get database.
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
    pub fn get_database(
        &self,
        args: &GetDatabaseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_database_builder(
            &self.http_client,
            &args.organization,
            &args.database,
        )
        .map_err(ProviderError::Api)?;

        let task = get_database_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Update database settings.
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
    pub fn update_database_settings(
        &self,
        args: &UpdateDatabaseSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = update_database_settings_builder(
            &self.http_client,
            &args.organization,
            &args.database,
        )
        .map_err(ProviderError::Api)?;

        let task = update_database_settings_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete database.
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
    pub fn delete_database(
        &self,
        args: &DeleteDatabaseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_database_builder(
            &self.http_client,
            &args.organization,
            &args.database,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_database_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// List backup policies.
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
    pub fn list_backup_policies(
        &self,
        args: &ListBackupPoliciesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_backup_policies_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.page,
            &args.per_page,
        )
        .map_err(ProviderError::Api)?;

        let task = list_backup_policies_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Create backup policy.
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
    pub fn create_backup_policy(
        &self,
        args: &CreateBackupPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = create_backup_policy_builder(
            &self.http_client,
            &args.organization,
            &args.database,
        )
        .map_err(ProviderError::Api)?;

        let task = create_backup_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get backup policy.
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
    pub fn get_backup_policy(
        &self,
        args: &GetBackupPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_backup_policy_builder(
            &self.http_client,
            &args.id,
            &args.organization,
            &args.database,
        )
        .map_err(ProviderError::Api)?;

        let task = get_backup_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Update backup policy.
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
    pub fn update_backup_policy(
        &self,
        args: &UpdateBackupPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = update_backup_policy_builder(
            &self.http_client,
            &args.id,
            &args.organization,
            &args.database,
        )
        .map_err(ProviderError::Api)?;

        let task = update_backup_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete backup policy.
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
    pub fn delete_backup_policy(
        &self,
        args: &DeleteBackupPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_backup_policy_builder(
            &self.http_client,
            &args.id,
            &args.organization,
            &args.database,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_backup_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// List branches.
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
    pub fn list_branches(
        &self,
        args: &ListBranchesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_branches_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.q,
            &args.production,
            &args.safe_migrations,
            &args.order,
            &args.page,
            &args.per_page,
        )
        .map_err(ProviderError::Api)?;

        let task = list_branches_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Create branch.
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
    pub fn create_branch(
        &self,
        args: &CreateBranchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = create_branch_builder(
            &self.http_client,
            &args.organization,
            &args.database,
        )
        .map_err(ProviderError::Api)?;

        let task = create_branch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get branch.
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
    pub fn get_branch(
        &self,
        args: &GetBranchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_branch_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
        )
        .map_err(ProviderError::Api)?;

        let task = get_branch_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Update branch.
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
    pub fn update_branch(
        &self,
        args: &UpdateBranchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = update_branch_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
        )
        .map_err(ProviderError::Api)?;

        let task = update_branch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete branch.
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
    pub fn delete_branch(
        &self,
        args: &DeleteBranchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_branch_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
            &args.delete_descendants,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_branch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// List backups.
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
    pub fn list_backups(
        &self,
        args: &ListBackupsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_backups_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
            &args.all,
            &args.state,
            &args.policy,
            &args.from,
            &args.to,
            &args.running_at,
            &args.production,
            &args.page,
            &args.per_page,
        )
        .map_err(ProviderError::Api)?;

        let task = list_backups_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Create backup.
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
    pub fn create_backup(
        &self,
        args: &CreateBackupArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = create_backup_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
        )
        .map_err(ProviderError::Api)?;

        let task = create_backup_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get backup.
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
    pub fn get_backup(
        &self,
        args: &GetBackupArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_backup_builder(
            &self.http_client,
            &args.id,
            &args.organization,
            &args.database,
            &args.branch,
        )
        .map_err(ProviderError::Api)?;

        let task = get_backup_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Update backup.
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
    pub fn update_backup(
        &self,
        args: &UpdateBackupArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = update_backup_builder(
            &self.http_client,
            &args.id,
            &args.organization,
            &args.database,
            &args.branch,
        )
        .map_err(ProviderError::Api)?;

        let task = update_backup_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete backup.
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
    pub fn delete_backup(
        &self,
        args: &DeleteBackupArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_backup_builder(
            &self.http_client,
            &args.id,
            &args.organization,
            &args.database,
            &args.branch,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_backup_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// List branch bouncer resize requests.
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
    pub fn list_branch_bouncer_resize_requests(
        &self,
        args: &ListBranchBouncerResizeRequestsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_branch_bouncer_resize_requests_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
            &args.page,
            &args.per_page,
        )
        .map_err(ProviderError::Api)?;

        let task = list_branch_bouncer_resize_requests_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// List bouncers.
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
    pub fn list_bouncers(
        &self,
        args: &ListBouncersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_bouncers_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
            &args.page,
            &args.per_page,
        )
        .map_err(ProviderError::Api)?;

        let task = list_bouncers_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Create bouncer.
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
    pub fn create_bouncer(
        &self,
        args: &CreateBouncerArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = create_bouncer_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
        )
        .map_err(ProviderError::Api)?;

        let task = create_bouncer_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get bouncer.
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
    pub fn get_bouncer(
        &self,
        args: &GetBouncerArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_bouncer_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
            &args.bouncer,
        )
        .map_err(ProviderError::Api)?;

        let task = get_bouncer_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete bouncer.
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
    pub fn delete_bouncer(
        &self,
        args: &DeleteBouncerArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_bouncer_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
            &args.bouncer,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_bouncer_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// List bouncer resize requests.
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
    pub fn list_bouncer_resize_requests(
        &self,
        args: &ListBouncerResizeRequestsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_bouncer_resize_requests_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
            &args.bouncer,
            &args.page,
            &args.per_page,
        )
        .map_err(ProviderError::Api)?;

        let task = list_bouncer_resize_requests_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Update bouncer resize request.
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
    pub fn update_bouncer_resize_request(
        &self,
        args: &UpdateBouncerResizeRequestArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = update_bouncer_resize_request_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
            &args.bouncer,
        )
        .map_err(ProviderError::Api)?;

        let task = update_bouncer_resize_request_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cancel bouncer resize request.
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
    pub fn cancel_bouncer_resize_request(
        &self,
        args: &CancelBouncerResizeRequestArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cancel_bouncer_resize_request_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
            &args.bouncer,
        )
        .map_err(ProviderError::Api)?;

        let task = cancel_bouncer_resize_request_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// List branch change requests.
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
    pub fn list_branch_change_requests(
        &self,
        args: &ListBranchChangeRequestsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_branch_change_requests_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
            &args.page,
            &args.per_page,
        )
        .map_err(ProviderError::Api)?;

        let task = list_branch_change_requests_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Update branch change request.
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
    pub fn update_branch_change_request(
        &self,
        args: &UpdateBranchChangeRequestArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = update_branch_change_request_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
        )
        .map_err(ProviderError::Api)?;

        let task = update_branch_change_request_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get branch change request.
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
    pub fn get_branch_change_request(
        &self,
        args: &GetBranchChangeRequestArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_branch_change_request_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = get_branch_change_request_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Update branch cluster config.
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
    pub fn update_branch_cluster_config(
        &self,
        args: &UpdateBranchClusterConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = update_branch_cluster_config_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
        )
        .map_err(ProviderError::Api)?;

        let task = update_branch_cluster_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Demote branch.
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
    pub fn demote_branch(
        &self,
        args: &DemoteBranchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = demote_branch_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
        )
        .map_err(ProviderError::Api)?;

        let task = demote_branch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// List extensions.
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
    pub fn list_extensions(
        &self,
        args: &ListExtensionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_extensions_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
        )
        .map_err(ProviderError::Api)?;

        let task = list_extensions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// List keyspaces.
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
    pub fn list_keyspaces(
        &self,
        args: &ListKeyspacesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_keyspaces_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
            &args.page,
            &args.per_page,
        )
        .map_err(ProviderError::Api)?;

        let task = list_keyspaces_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Create keyspace.
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
    pub fn create_keyspace(
        &self,
        args: &CreateKeyspaceArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = create_keyspace_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
        )
        .map_err(ProviderError::Api)?;

        let task = create_keyspace_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get keyspace.
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
    pub fn get_keyspace(
        &self,
        args: &GetKeyspaceArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_keyspace_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
            &args.keyspace,
        )
        .map_err(ProviderError::Api)?;

        let task = get_keyspace_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Update keyspace.
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
    pub fn update_keyspace(
        &self,
        args: &UpdateKeyspaceArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = update_keyspace_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
            &args.keyspace,
        )
        .map_err(ProviderError::Api)?;

        let task = update_keyspace_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete keyspace.
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
    pub fn delete_keyspace(
        &self,
        args: &DeleteKeyspaceArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_keyspace_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
            &args.keyspace,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_keyspace_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get keyspace rollout status.
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
    pub fn get_keyspace_rollout_status(
        &self,
        args: &GetKeyspaceRolloutStatusArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_keyspace_rollout_status_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
            &args.keyspace,
        )
        .map_err(ProviderError::Api)?;

        let task = get_keyspace_rollout_status_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get keyspace vschema.
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
    pub fn get_keyspace_vschema(
        &self,
        args: &GetKeyspaceVschemaArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_keyspace_vschema_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
            &args.keyspace,
        )
        .map_err(ProviderError::Api)?;

        let task = get_keyspace_vschema_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Update keyspace vschema.
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
    pub fn update_keyspace_vschema(
        &self,
        args: &UpdateKeyspaceVschemaArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = update_keyspace_vschema_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
            &args.keyspace,
        )
        .map_err(ProviderError::Api)?;

        let task = update_keyspace_vschema_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// List parameters.
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
    pub fn list_parameters(
        &self,
        args: &ListParametersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_parameters_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
        )
        .map_err(ProviderError::Api)?;

        let task = list_parameters_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// List passwords.
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
    pub fn list_passwords(
        &self,
        args: &ListPasswordsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_passwords_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
            &args.read_only_region_id,
            &args.page,
            &args.per_page,
        )
        .map_err(ProviderError::Api)?;

        let task = list_passwords_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Create password.
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
    pub fn create_password(
        &self,
        args: &CreatePasswordArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = create_password_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
        )
        .map_err(ProviderError::Api)?;

        let task = create_password_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get password.
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
    pub fn get_password(
        &self,
        args: &GetPasswordArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_password_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = get_password_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Update password.
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
    pub fn update_password(
        &self,
        args: &UpdatePasswordArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = update_password_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = update_password_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete password.
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
    pub fn delete_password(
        &self,
        args: &DeletePasswordArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_password_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_password_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Renew password.
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
    pub fn renew_password(
        &self,
        args: &RenewPasswordArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = renew_password_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = renew_password_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Promote branch.
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
    pub fn promote_branch(
        &self,
        args: &PromoteBranchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = promote_branch_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
        )
        .map_err(ProviderError::Api)?;

        let task = promote_branch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// List generated query patterns reports.
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
    pub fn list_generated_query_patterns_reports(
        &self,
        args: &ListGeneratedQueryPatternsReportsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_generated_query_patterns_reports_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
            &args.starting_after,
            &args.ending_before,
            &args.limit,
        )
        .map_err(ProviderError::Api)?;

        let task = list_generated_query_patterns_reports_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Create query patterns report.
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
    pub fn create_query_patterns_report(
        &self,
        args: &CreateQueryPatternsReportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = create_query_patterns_report_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
        )
        .map_err(ProviderError::Api)?;

        let task = create_query_patterns_report_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get query patterns report status.
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
    pub fn get_query_patterns_report_status(
        &self,
        args: &GetQueryPatternsReportStatusArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_query_patterns_report_status_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = get_query_patterns_report_status_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete query patterns report.
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
    pub fn delete_query_patterns_report(
        &self,
        args: &DeleteQueryPatternsReportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_query_patterns_report_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_query_patterns_report_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get query patterns report.
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
    pub fn get_query_patterns_report(
        &self,
        args: &GetQueryPatternsReportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_query_patterns_report_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = get_query_patterns_report_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cancel branch change request.
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
    pub fn cancel_branch_change_request(
        &self,
        args: &CancelBranchChangeRequestArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cancel_branch_change_request_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
        )
        .map_err(ProviderError::Api)?;

        let task = cancel_branch_change_request_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// List roles.
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
    pub fn list_roles(
        &self,
        args: &ListRolesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_roles_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
            &args.page,
            &args.per_page,
        )
        .map_err(ProviderError::Api)?;

        let task = list_roles_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Create role.
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
    pub fn create_role(
        &self,
        args: &CreateRoleArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = create_role_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
        )
        .map_err(ProviderError::Api)?;

        let task = create_role_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get default role.
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
    pub fn get_default_role(
        &self,
        args: &GetDefaultRoleArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_default_role_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
        )
        .map_err(ProviderError::Api)?;

        let task = get_default_role_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Reset default role.
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
    pub fn reset_default_role(
        &self,
        args: &ResetDefaultRoleArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = reset_default_role_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
        )
        .map_err(ProviderError::Api)?;

        let task = reset_default_role_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get role.
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
    pub fn get_role(
        &self,
        args: &GetRoleArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_role_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = get_role_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Update role.
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
    pub fn update_role(
        &self,
        args: &UpdateRoleArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = update_role_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = update_role_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete role.
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
    pub fn delete_role(
        &self,
        args: &DeleteRoleArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_role_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
            &args.id,
            &args.successor,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_role_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Reassign role objects.
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
    pub fn reassign_role_objects(
        &self,
        args: &ReassignRoleObjectsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = reassign_role_objects_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = reassign_role_objects_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Renew role.
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
    pub fn renew_role(
        &self,
        args: &RenewRoleArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = renew_role_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = renew_role_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Reset role.
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
    pub fn reset_role(
        &self,
        args: &ResetRoleArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = reset_role_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = reset_role_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Enable safe migrations.
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
    pub fn enable_safe_migrations(
        &self,
        args: &EnableSafeMigrationsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = enable_safe_migrations_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
        )
        .map_err(ProviderError::Api)?;

        let task = enable_safe_migrations_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Disable safe migrations.
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
    pub fn disable_safe_migrations(
        &self,
        args: &DisableSafeMigrationsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = disable_safe_migrations_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
        )
        .map_err(ProviderError::Api)?;

        let task = disable_safe_migrations_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get branch schema.
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
    pub fn get_branch_schema(
        &self,
        args: &GetBranchSchemaArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_branch_schema_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
            &args.keyspace,
            &args.namespace,
        )
        .map_err(ProviderError::Api)?;

        let task = get_branch_schema_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Lint branch schema.
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
    pub fn lint_branch_schema(
        &self,
        args: &LintBranchSchemaArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = lint_branch_schema_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
            &args.page,
            &args.per_page,
        )
        .map_err(ProviderError::Api)?;

        let task = lint_branch_schema_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// List traffic budgets.
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
    pub fn list_traffic_budgets(
        &self,
        args: &ListTrafficBudgetsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_traffic_budgets_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
            &args.page,
            &args.per_page,
            &args.period,
            &args.created_at,
            &args.fingerprint,
        )
        .map_err(ProviderError::Api)?;

        let task = list_traffic_budgets_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Create traffic budget.
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
    pub fn create_traffic_budget(
        &self,
        args: &CreateTrafficBudgetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = create_traffic_budget_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
        )
        .map_err(ProviderError::Api)?;

        let task = create_traffic_budget_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Create traffic rule.
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
    pub fn create_traffic_rule(
        &self,
        args: &CreateTrafficRuleArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = create_traffic_rule_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
            &args.budget_id,
        )
        .map_err(ProviderError::Api)?;

        let task = create_traffic_rule_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete traffic rule.
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
    pub fn delete_traffic_rule(
        &self,
        args: &DeleteTrafficRuleArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_traffic_rule_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
            &args.budget_id,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_traffic_rule_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get traffic budget.
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
    pub fn get_traffic_budget(
        &self,
        args: &GetTrafficBudgetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_traffic_budget_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = get_traffic_budget_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Update traffic budget.
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
    pub fn update_traffic_budget(
        &self,
        args: &UpdateTrafficBudgetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = update_traffic_budget_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = update_traffic_budget_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete traffic budget.
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
    pub fn delete_traffic_budget(
        &self,
        args: &DeleteTrafficBudgetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_traffic_budget_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.branch,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_traffic_budget_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// List database postgres cidrs.
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
    pub fn list_database_postgres_cidrs(
        &self,
        args: &ListDatabasePostgresCidrsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_database_postgres_cidrs_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.page,
            &args.per_page,
        )
        .map_err(ProviderError::Api)?;

        let task = list_database_postgres_cidrs_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Create database postgres cidr.
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
    pub fn create_database_postgres_cidr(
        &self,
        args: &CreateDatabasePostgresCidrArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = create_database_postgres_cidr_builder(
            &self.http_client,
            &args.organization,
            &args.database,
        )
        .map_err(ProviderError::Api)?;

        let task = create_database_postgres_cidr_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get database postgres cidr.
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
    pub fn get_database_postgres_cidr(
        &self,
        args: &GetDatabasePostgresCidrArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_database_postgres_cidr_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = get_database_postgres_cidr_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Update database postgres cidr.
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
    pub fn update_database_postgres_cidr(
        &self,
        args: &UpdateDatabasePostgresCidrArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = update_database_postgres_cidr_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = update_database_postgres_cidr_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete database postgres cidr.
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
    pub fn delete_database_postgres_cidr(
        &self,
        args: &DeleteDatabasePostgresCidrArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_database_postgres_cidr_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_database_postgres_cidr_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get deploy queue.
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
    pub fn get_deploy_queue(
        &self,
        args: &GetDeployQueueArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_deploy_queue_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.page,
            &args.per_page,
        )
        .map_err(ProviderError::Api)?;

        let task = get_deploy_queue_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// List deploy requests.
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
    pub fn list_deploy_requests(
        &self,
        args: &ListDeployRequestsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_deploy_requests_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.state,
            &args.branch,
            &args.into_branch,
            &args.deployed_at,
            &args.running_at,
            &args.page,
            &args.per_page,
        )
        .map_err(ProviderError::Api)?;

        let task = list_deploy_requests_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Create deploy request.
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
    pub fn create_deploy_request(
        &self,
        args: &CreateDeployRequestArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = create_deploy_request_builder(
            &self.http_client,
            &args.organization,
            &args.database,
        )
        .map_err(ProviderError::Api)?;

        let task = create_deploy_request_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get deploy request.
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
    pub fn get_deploy_request(
        &self,
        args: &GetDeployRequestArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_deploy_request_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.number,
        )
        .map_err(ProviderError::Api)?;

        let task = get_deploy_request_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Close deploy request.
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
    pub fn close_deploy_request(
        &self,
        args: &CloseDeployRequestArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = close_deploy_request_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.number,
        )
        .map_err(ProviderError::Api)?;

        let task = close_deploy_request_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Complete gated deploy request.
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
    pub fn complete_gated_deploy_request(
        &self,
        args: &CompleteGatedDeployRequestArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = complete_gated_deploy_request_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.number,
        )
        .map_err(ProviderError::Api)?;

        let task = complete_gated_deploy_request_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Update auto apply.
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
    pub fn update_auto_apply(
        &self,
        args: &UpdateAutoApplyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = update_auto_apply_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.number,
        )
        .map_err(ProviderError::Api)?;

        let task = update_auto_apply_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Update auto delete branch.
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
    pub fn update_auto_delete_branch(
        &self,
        args: &UpdateAutoDeleteBranchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = update_auto_delete_branch_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.number,
        )
        .map_err(ProviderError::Api)?;

        let task = update_auto_delete_branch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cancel deploy request.
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
    pub fn cancel_deploy_request(
        &self,
        args: &CancelDeployRequestArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cancel_deploy_request_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.number,
        )
        .map_err(ProviderError::Api)?;

        let task = cancel_deploy_request_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Complete errored deploy.
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
    pub fn complete_errored_deploy(
        &self,
        args: &CompleteErroredDeployArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = complete_errored_deploy_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.number,
        )
        .map_err(ProviderError::Api)?;

        let task = complete_errored_deploy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Queue deploy request.
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
    pub fn queue_deploy_request(
        &self,
        args: &QueueDeployRequestArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = queue_deploy_request_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.number,
        )
        .map_err(ProviderError::Api)?;

        let task = queue_deploy_request_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get deployment.
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
    pub fn get_deployment(
        &self,
        args: &GetDeploymentArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_deployment_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.number,
        )
        .map_err(ProviderError::Api)?;

        let task = get_deployment_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// List deploy operations.
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
    pub fn list_deploy_operations(
        &self,
        args: &ListDeployOperationsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_deploy_operations_builder(
            &self.http_client,
            &args.number,
            &args.organization,
            &args.database,
            &args.page,
            &args.per_page,
        )
        .map_err(ProviderError::Api)?;

        let task = list_deploy_operations_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Complete revert.
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
    pub fn complete_revert(
        &self,
        args: &CompleteRevertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = complete_revert_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.number,
        )
        .map_err(ProviderError::Api)?;

        let task = complete_revert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// List deploy request reviews.
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
    pub fn list_deploy_request_reviews(
        &self,
        args: &ListDeployRequestReviewsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_deploy_request_reviews_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.number,
            &args.page,
            &args.per_page,
        )
        .map_err(ProviderError::Api)?;

        let task = list_deploy_request_reviews_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Review deploy request.
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
    pub fn review_deploy_request(
        &self,
        args: &ReviewDeployRequestArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = review_deploy_request_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.number,
        )
        .map_err(ProviderError::Api)?;

        let task = review_deploy_request_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Skip revert period.
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
    pub fn skip_revert_period(
        &self,
        args: &SkipRevertPeriodArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = skip_revert_period_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.number,
        )
        .map_err(ProviderError::Api)?;

        let task = skip_revert_period_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Check deploy request storage.
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
    pub fn check_deploy_request_storage(
        &self,
        args: &CheckDeployRequestStorageArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = check_deploy_request_storage_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.number,
        )
        .map_err(ProviderError::Api)?;

        let task = check_deploy_request_storage_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get deploy request throttler.
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
    pub fn get_deploy_request_throttler(
        &self,
        args: &GetDeployRequestThrottlerArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_deploy_request_throttler_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.number,
        )
        .map_err(ProviderError::Api)?;

        let task = get_deploy_request_throttler_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Update deploy request throttler.
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
    pub fn update_deploy_request_throttler(
        &self,
        args: &UpdateDeployRequestThrottlerArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = update_deploy_request_throttler_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.number,
        )
        .map_err(ProviderError::Api)?;

        let task = update_deploy_request_throttler_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// List maintenance schedules.
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
    pub fn list_maintenance_schedules(
        &self,
        args: &ListMaintenanceSchedulesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_maintenance_schedules_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.page,
            &args.per_page,
        )
        .map_err(ProviderError::Api)?;

        let task = list_maintenance_schedules_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get maintenance schedule.
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
    pub fn get_maintenance_schedule(
        &self,
        args: &GetMaintenanceScheduleArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_maintenance_schedule_builder(
            &self.http_client,
            &args.id,
            &args.organization,
            &args.database,
        )
        .map_err(ProviderError::Api)?;

        let task = get_maintenance_schedule_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// List maintenance windows.
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
    pub fn list_maintenance_windows(
        &self,
        args: &ListMaintenanceWindowsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_maintenance_windows_builder(
            &self.http_client,
            &args.id,
            &args.organization,
            &args.database,
            &args.page,
            &args.per_page,
        )
        .map_err(ProviderError::Api)?;

        let task = list_maintenance_windows_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// List read only regions.
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
    pub fn list_read_only_regions(
        &self,
        args: &ListReadOnlyRegionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_read_only_regions_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.page,
            &args.per_page,
        )
        .map_err(ProviderError::Api)?;

        let task = list_read_only_regions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// List database regions.
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
    pub fn list_database_regions(
        &self,
        args: &ListDatabaseRegionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_database_regions_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.page,
            &args.per_page,
        )
        .map_err(ProviderError::Api)?;

        let task = list_database_regions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// List schema recommendations.
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
    pub fn list_schema_recommendations(
        &self,
        args: &ListSchemaRecommendationsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_schema_recommendations_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.state,
            &args.page,
            &args.per_page,
        )
        .map_err(ProviderError::Api)?;

        let task = list_schema_recommendations_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get schema recommendation.
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
    pub fn get_schema_recommendation(
        &self,
        args: &GetSchemaRecommendationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_schema_recommendation_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.number,
        )
        .map_err(ProviderError::Api)?;

        let task = get_schema_recommendation_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dismiss schema recommendation.
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
    pub fn dismiss_schema_recommendation(
        &self,
        args: &DismissSchemaRecommendationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dismiss_schema_recommendation_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.number,
        )
        .map_err(ProviderError::Api)?;

        let task = dismiss_schema_recommendation_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get database throttler.
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
    pub fn get_database_throttler(
        &self,
        args: &GetDatabaseThrottlerArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_database_throttler_builder(
            &self.http_client,
            &args.organization,
            &args.database,
        )
        .map_err(ProviderError::Api)?;

        let task = get_database_throttler_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Update database throttler.
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
    pub fn update_database_throttler(
        &self,
        args: &UpdateDatabaseThrottlerArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = update_database_throttler_builder(
            &self.http_client,
            &args.organization,
            &args.database,
        )
        .map_err(ProviderError::Api)?;

        let task = update_database_throttler_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// List webhooks.
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
    pub fn list_webhooks(
        &self,
        args: &ListWebhooksArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_webhooks_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.page,
            &args.per_page,
        )
        .map_err(ProviderError::Api)?;

        let task = list_webhooks_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Create webhook.
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
    pub fn create_webhook(
        &self,
        args: &CreateWebhookArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = create_webhook_builder(
            &self.http_client,
            &args.organization,
            &args.database,
        )
        .map_err(ProviderError::Api)?;

        let task = create_webhook_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get webhook.
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
    pub fn get_webhook(
        &self,
        args: &GetWebhookArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_webhook_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = get_webhook_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Update webhook.
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
    pub fn update_webhook(
        &self,
        args: &UpdateWebhookArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = update_webhook_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = update_webhook_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete webhook.
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
    pub fn delete_webhook(
        &self,
        args: &DeleteWebhookArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_webhook_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_webhook_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Test webhook.
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
    pub fn test_webhook(
        &self,
        args: &TestWebhookArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = test_webhook_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = test_webhook_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// List workflows.
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
    pub fn list_workflows(
        &self,
        args: &ListWorkflowsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_workflows_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.between,
            &args.page,
            &args.per_page,
        )
        .map_err(ProviderError::Api)?;

        let task = list_workflows_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Create workflow.
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
    pub fn create_workflow(
        &self,
        args: &CreateWorkflowArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = create_workflow_builder(
            &self.http_client,
            &args.organization,
            &args.database,
        )
        .map_err(ProviderError::Api)?;

        let task = create_workflow_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get workflow.
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
    pub fn get_workflow(
        &self,
        args: &GetWorkflowArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_workflow_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.number,
        )
        .map_err(ProviderError::Api)?;

        let task = get_workflow_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workflow cancel.
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
    pub fn workflow_cancel(
        &self,
        args: &WorkflowCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workflow_cancel_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.number,
        )
        .map_err(ProviderError::Api)?;

        let task = workflow_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workflow complete.
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
    pub fn workflow_complete(
        &self,
        args: &WorkflowCompleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workflow_complete_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.number,
        )
        .map_err(ProviderError::Api)?;

        let task = workflow_complete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workflow cutover.
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
    pub fn workflow_cutover(
        &self,
        args: &WorkflowCutoverArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workflow_cutover_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.number,
        )
        .map_err(ProviderError::Api)?;

        let task = workflow_cutover_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workflow retry.
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
    pub fn workflow_retry(
        &self,
        args: &WorkflowRetryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workflow_retry_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.number,
        )
        .map_err(ProviderError::Api)?;

        let task = workflow_retry_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workflow reverse cutover.
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
    pub fn workflow_reverse_cutover(
        &self,
        args: &WorkflowReverseCutoverArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workflow_reverse_cutover_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.number,
        )
        .map_err(ProviderError::Api)?;

        let task = workflow_reverse_cutover_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workflow reverse traffic.
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
    pub fn workflow_reverse_traffic(
        &self,
        args: &WorkflowReverseTrafficArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workflow_reverse_traffic_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.number,
        )
        .map_err(ProviderError::Api)?;

        let task = workflow_reverse_traffic_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workflow switch primaries.
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
    pub fn workflow_switch_primaries(
        &self,
        args: &WorkflowSwitchPrimariesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workflow_switch_primaries_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.number,
        )
        .map_err(ProviderError::Api)?;

        let task = workflow_switch_primaries_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workflow switch replicas.
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
    pub fn workflow_switch_replicas(
        &self,
        args: &WorkflowSwitchReplicasArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workflow_switch_replicas_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.number,
        )
        .map_err(ProviderError::Api)?;

        let task = workflow_switch_replicas_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Verify workflow.
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
    pub fn verify_workflow(
        &self,
        args: &VerifyWorkflowArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = verify_workflow_builder(
            &self.http_client,
            &args.organization,
            &args.database,
            &args.number,
        )
        .map_err(ProviderError::Api)?;

        let task = verify_workflow_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// List invoices.
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
    pub fn list_invoices(
        &self,
        args: &ListInvoicesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_invoices_builder(
            &self.http_client,
            &args.organization,
            &args.page,
            &args.per_page,
        )
        .map_err(ProviderError::Api)?;

        let task = list_invoices_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get invoice.
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
    pub fn get_invoice(
        &self,
        args: &GetInvoiceArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_invoice_builder(
            &self.http_client,
            &args.organization,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = get_invoice_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get invoice line items.
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
    pub fn get_invoice_line_items(
        &self,
        args: &GetInvoiceLineItemsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_invoice_line_items_builder(
            &self.http_client,
            &args.organization,
            &args.id,
            &args.page,
            &args.per_page,
        )
        .map_err(ProviderError::Api)?;

        let task = get_invoice_line_items_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// List organization members.
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
    pub fn list_organization_members(
        &self,
        args: &ListOrganizationMembersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_organization_members_builder(
            &self.http_client,
            &args.organization,
            &args.q,
            &args.page,
            &args.per_page,
        )
        .map_err(ProviderError::Api)?;

        let task = list_organization_members_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get organization membership.
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
    pub fn get_organization_membership(
        &self,
        args: &GetOrganizationMembershipArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_organization_membership_builder(
            &self.http_client,
            &args.organization,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = get_organization_membership_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Update organization membership.
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
    pub fn update_organization_membership(
        &self,
        args: &UpdateOrganizationMembershipArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = update_organization_membership_builder(
            &self.http_client,
            &args.organization,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = update_organization_membership_task(builder)
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
    /// StreamIterator yielding the () result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn remove_organization_member(
        &self,
        args: &RemoveOrganizationMemberArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = remove_organization_member_builder(
            &self.http_client,
            &args.organization,
            &args.id,
            &args.delete_passwords,
            &args.delete_service_tokens,
        )
        .map_err(ProviderError::Api)?;

        let task = remove_organization_member_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// List oauth applications.
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
    pub fn list_oauth_applications(
        &self,
        args: &ListOauthApplicationsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_oauth_applications_builder(
            &self.http_client,
            &args.organization,
            &args.page,
            &args.per_page,
        )
        .map_err(ProviderError::Api)?;

        let task = list_oauth_applications_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get oauth application.
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
    pub fn get_oauth_application(
        &self,
        args: &GetOauthApplicationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_oauth_application_builder(
            &self.http_client,
            &args.organization,
            &args.application_id,
        )
        .map_err(ProviderError::Api)?;

        let task = get_oauth_application_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// List oauth tokens.
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
    pub fn list_oauth_tokens(
        &self,
        args: &ListOauthTokensArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_oauth_tokens_builder(
            &self.http_client,
            &args.organization,
            &args.application_id,
            &args.page,
            &args.per_page,
        )
        .map_err(ProviderError::Api)?;

        let task = list_oauth_tokens_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get oauth token.
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
    pub fn get_oauth_token(
        &self,
        args: &GetOauthTokenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_oauth_token_builder(
            &self.http_client,
            &args.organization,
            &args.application_id,
            &args.token_id,
        )
        .map_err(ProviderError::Api)?;

        let task = get_oauth_token_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete oauth token.
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
    pub fn delete_oauth_token(
        &self,
        args: &DeleteOauthTokenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_oauth_token_builder(
            &self.http_client,
            &args.organization,
            &args.application_id,
            &args.token_id,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_oauth_token_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Create oauth token.
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
    pub fn create_oauth_token(
        &self,
        args: &CreateOauthTokenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = create_oauth_token_builder(
            &self.http_client,
            &args.organization,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = create_oauth_token_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// List regions for organization.
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
    pub fn list_regions_for_organization(
        &self,
        args: &ListRegionsForOrganizationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_regions_for_organization_builder(
            &self.http_client,
            &args.organization,
            &args.page,
            &args.per_page,
        )
        .map_err(ProviderError::Api)?;

        let task = list_regions_for_organization_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// List service tokens.
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
    pub fn list_service_tokens(
        &self,
        args: &ListServiceTokensArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_service_tokens_builder(
            &self.http_client,
            &args.organization,
            &args.page,
            &args.per_page,
        )
        .map_err(ProviderError::Api)?;

        let task = list_service_tokens_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Create service token.
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
    pub fn create_service_token(
        &self,
        args: &CreateServiceTokenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = create_service_token_builder(
            &self.http_client,
            &args.organization,
        )
        .map_err(ProviderError::Api)?;

        let task = create_service_token_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get service token.
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
    pub fn get_service_token(
        &self,
        args: &GetServiceTokenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_service_token_builder(
            &self.http_client,
            &args.organization,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = get_service_token_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete service token.
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
    pub fn delete_service_token(
        &self,
        args: &DeleteServiceTokenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_service_token_builder(
            &self.http_client,
            &args.organization,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_service_token_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// List organization teams.
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
    pub fn list_organization_teams(
        &self,
        args: &ListOrganizationTeamsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_organization_teams_builder(
            &self.http_client,
            &args.organization,
            &args.q,
            &args.page,
            &args.per_page,
        )
        .map_err(ProviderError::Api)?;

        let task = list_organization_teams_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Create organization team.
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
    pub fn create_organization_team(
        &self,
        args: &CreateOrganizationTeamArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = create_organization_team_builder(
            &self.http_client,
            &args.organization,
        )
        .map_err(ProviderError::Api)?;

        let task = create_organization_team_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get organization team.
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
    pub fn get_organization_team(
        &self,
        args: &GetOrganizationTeamArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_organization_team_builder(
            &self.http_client,
            &args.organization,
            &args.team,
        )
        .map_err(ProviderError::Api)?;

        let task = get_organization_team_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Update organization team.
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
    pub fn update_organization_team(
        &self,
        args: &UpdateOrganizationTeamArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = update_organization_team_builder(
            &self.http_client,
            &args.organization,
            &args.team,
        )
        .map_err(ProviderError::Api)?;

        let task = update_organization_team_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete organization team.
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
    pub fn delete_organization_team(
        &self,
        args: &DeleteOrganizationTeamArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_organization_team_builder(
            &self.http_client,
            &args.organization,
            &args.team,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_organization_team_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// List organization team members.
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
    pub fn list_organization_team_members(
        &self,
        args: &ListOrganizationTeamMembersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_organization_team_members_builder(
            &self.http_client,
            &args.organization,
            &args.team,
            &args.page,
            &args.per_page,
        )
        .map_err(ProviderError::Api)?;

        let task = list_organization_team_members_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Add organization team member.
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
    pub fn add_organization_team_member(
        &self,
        args: &AddOrganizationTeamMemberArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = add_organization_team_member_builder(
            &self.http_client,
            &args.organization,
            &args.team,
        )
        .map_err(ProviderError::Api)?;

        let task = add_organization_team_member_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get organization team member.
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
    pub fn get_organization_team_member(
        &self,
        args: &GetOrganizationTeamMemberArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_organization_team_member_builder(
            &self.http_client,
            &args.organization,
            &args.team,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = get_organization_team_member_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Remove organization team member.
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
    pub fn remove_organization_team_member(
        &self,
        args: &RemoveOrganizationTeamMemberArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = remove_organization_team_member_builder(
            &self.http_client,
            &args.organization,
            &args.team,
            &args.id,
            &args.delete_passwords,
        )
        .map_err(ProviderError::Api)?;

        let task = remove_organization_team_member_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// List public regions.
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
    pub fn list_public_regions(
        &self,
        args: &ListPublicRegionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = list_public_regions_builder(
            &self.http_client,
            &args.page,
            &args.per_page,
        )
        .map_err(ProviderError::Api)?;

        let task = list_public_regions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get current user.
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
    pub fn get_current_user(
        &self,
        args: &GetCurrentUserArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::planetscale::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_current_user_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = get_current_user_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
