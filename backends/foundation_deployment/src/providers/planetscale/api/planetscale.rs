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

use crate::providers::planetscale::clients::planetscale::{
    update_organization_builder, update_organization_task,
    create_database_builder, create_database_task,
    update_database_settings_builder, update_database_settings_task,
    delete_database_builder, delete_database_task,
    create_backup_policy_builder, create_backup_policy_task,
    update_backup_policy_builder, update_backup_policy_task,
    delete_backup_policy_builder, delete_backup_policy_task,
    create_branch_builder, create_branch_task,
    update_branch_builder, update_branch_task,
    delete_branch_builder, delete_branch_task,
    create_backup_builder, create_backup_task,
    update_backup_builder, update_backup_task,
    delete_backup_builder, delete_backup_task,
    create_bouncer_builder, create_bouncer_task,
    delete_bouncer_builder, delete_bouncer_task,
    update_bouncer_resize_request_builder, update_bouncer_resize_request_task,
    cancel_bouncer_resize_request_builder, cancel_bouncer_resize_request_task,
    update_branch_change_request_builder, update_branch_change_request_task,
    update_branch_cluster_config_builder, update_branch_cluster_config_task,
    demote_branch_builder, demote_branch_task,
    create_keyspace_builder, create_keyspace_task,
    update_keyspace_builder, update_keyspace_task,
    delete_keyspace_builder, delete_keyspace_task,
    update_keyspace_vschema_builder, update_keyspace_vschema_task,
    create_password_builder, create_password_task,
    update_password_builder, update_password_task,
    delete_password_builder, delete_password_task,
    renew_password_builder, renew_password_task,
    promote_branch_builder, promote_branch_task,
    create_query_patterns_report_builder, create_query_patterns_report_task,
    delete_query_patterns_report_builder, delete_query_patterns_report_task,
    cancel_branch_change_request_builder, cancel_branch_change_request_task,
    create_role_builder, create_role_task,
    reset_default_role_builder, reset_default_role_task,
    update_role_builder, update_role_task,
    delete_role_builder, delete_role_task,
    reassign_role_objects_builder, reassign_role_objects_task,
    renew_role_builder, renew_role_task,
    reset_role_builder, reset_role_task,
    enable_safe_migrations_builder, enable_safe_migrations_task,
    disable_safe_migrations_builder, disable_safe_migrations_task,
    create_traffic_budget_builder, create_traffic_budget_task,
    create_traffic_rule_builder, create_traffic_rule_task,
    delete_traffic_rule_builder, delete_traffic_rule_task,
    update_traffic_budget_builder, update_traffic_budget_task,
    delete_traffic_budget_builder, delete_traffic_budget_task,
    create_database_postgres_cidr_builder, create_database_postgres_cidr_task,
    update_database_postgres_cidr_builder, update_database_postgres_cidr_task,
    delete_database_postgres_cidr_builder, delete_database_postgres_cidr_task,
    create_deploy_request_builder, create_deploy_request_task,
    close_deploy_request_builder, close_deploy_request_task,
    complete_gated_deploy_request_builder, complete_gated_deploy_request_task,
    update_auto_apply_builder, update_auto_apply_task,
    update_auto_delete_branch_builder, update_auto_delete_branch_task,
    cancel_deploy_request_builder, cancel_deploy_request_task,
    complete_errored_deploy_builder, complete_errored_deploy_task,
    queue_deploy_request_builder, queue_deploy_request_task,
    complete_revert_builder, complete_revert_task,
    review_deploy_request_builder, review_deploy_request_task,
    skip_revert_period_builder, skip_revert_period_task,
    update_deploy_request_throttler_builder, update_deploy_request_throttler_task,
    dismiss_schema_recommendation_builder, dismiss_schema_recommendation_task,
    update_database_throttler_builder, update_database_throttler_task,
    create_webhook_builder, create_webhook_task,
    update_webhook_builder, update_webhook_task,
    delete_webhook_builder, delete_webhook_task,
    test_webhook_builder, test_webhook_task,
    create_workflow_builder, create_workflow_task,
    workflow_cancel_builder, workflow_cancel_task,
    workflow_complete_builder, workflow_complete_task,
    workflow_cutover_builder, workflow_cutover_task,
    workflow_retry_builder, workflow_retry_task,
    workflow_reverse_cutover_builder, workflow_reverse_cutover_task,
    workflow_reverse_traffic_builder, workflow_reverse_traffic_task,
    workflow_switch_primaries_builder, workflow_switch_primaries_task,
    workflow_switch_replicas_builder, workflow_switch_replicas_task,
    verify_workflow_builder, verify_workflow_task,
    update_organization_membership_builder, update_organization_membership_task,
    remove_organization_member_builder, remove_organization_member_task,
    delete_oauth_token_builder, delete_oauth_token_task,
    create_oauth_token_builder, create_oauth_token_task,
    create_service_token_builder, create_service_token_task,
    delete_service_token_builder, delete_service_token_task,
    create_organization_team_builder, create_organization_team_task,
    update_organization_team_builder, update_organization_team_task,
    delete_organization_team_builder, delete_organization_team_task,
    add_organization_team_member_builder, add_organization_team_member_task,
    remove_organization_team_member_builder, remove_organization_team_member_task,
};
use crate::providers::planetscale::clients::types::{ApiError, ApiPending};
use crate::providers::planetscale::clients::planetscale::AddOrganizationTeamMemberArgs;
use crate::providers::planetscale::clients::planetscale::CancelBouncerResizeRequestArgs;
use crate::providers::planetscale::clients::planetscale::CancelBranchChangeRequestArgs;
use crate::providers::planetscale::clients::planetscale::CancelDeployRequestArgs;
use crate::providers::planetscale::clients::planetscale::CloseDeployRequestArgs;
use crate::providers::planetscale::clients::planetscale::CompleteErroredDeployArgs;
use crate::providers::planetscale::clients::planetscale::CompleteGatedDeployRequestArgs;
use crate::providers::planetscale::clients::planetscale::CompleteRevertArgs;
use crate::providers::planetscale::clients::planetscale::CreateBackupArgs;
use crate::providers::planetscale::clients::planetscale::CreateBackupPolicyArgs;
use crate::providers::planetscale::clients::planetscale::CreateBouncerArgs;
use crate::providers::planetscale::clients::planetscale::CreateBranchArgs;
use crate::providers::planetscale::clients::planetscale::CreateDatabaseArgs;
use crate::providers::planetscale::clients::planetscale::CreateDatabasePostgresCidrArgs;
use crate::providers::planetscale::clients::planetscale::CreateDeployRequestArgs;
use crate::providers::planetscale::clients::planetscale::CreateKeyspaceArgs;
use crate::providers::planetscale::clients::planetscale::CreateOauthTokenArgs;
use crate::providers::planetscale::clients::planetscale::CreateOrganizationTeamArgs;
use crate::providers::planetscale::clients::planetscale::CreatePasswordArgs;
use crate::providers::planetscale::clients::planetscale::CreateQueryPatternsReportArgs;
use crate::providers::planetscale::clients::planetscale::CreateRoleArgs;
use crate::providers::planetscale::clients::planetscale::CreateServiceTokenArgs;
use crate::providers::planetscale::clients::planetscale::CreateTrafficBudgetArgs;
use crate::providers::planetscale::clients::planetscale::CreateTrafficRuleArgs;
use crate::providers::planetscale::clients::planetscale::CreateWebhookArgs;
use crate::providers::planetscale::clients::planetscale::CreateWorkflowArgs;
use crate::providers::planetscale::clients::planetscale::DeleteBackupArgs;
use crate::providers::planetscale::clients::planetscale::DeleteBackupPolicyArgs;
use crate::providers::planetscale::clients::planetscale::DeleteBouncerArgs;
use crate::providers::planetscale::clients::planetscale::DeleteBranchArgs;
use crate::providers::planetscale::clients::planetscale::DeleteDatabaseArgs;
use crate::providers::planetscale::clients::planetscale::DeleteDatabasePostgresCidrArgs;
use crate::providers::planetscale::clients::planetscale::DeleteKeyspaceArgs;
use crate::providers::planetscale::clients::planetscale::DeleteOauthTokenArgs;
use crate::providers::planetscale::clients::planetscale::DeleteOrganizationTeamArgs;
use crate::providers::planetscale::clients::planetscale::DeletePasswordArgs;
use crate::providers::planetscale::clients::planetscale::DeleteQueryPatternsReportArgs;
use crate::providers::planetscale::clients::planetscale::DeleteRoleArgs;
use crate::providers::planetscale::clients::planetscale::DeleteServiceTokenArgs;
use crate::providers::planetscale::clients::planetscale::DeleteTrafficBudgetArgs;
use crate::providers::planetscale::clients::planetscale::DeleteTrafficRuleArgs;
use crate::providers::planetscale::clients::planetscale::DeleteWebhookArgs;
use crate::providers::planetscale::clients::planetscale::DemoteBranchArgs;
use crate::providers::planetscale::clients::planetscale::DisableSafeMigrationsArgs;
use crate::providers::planetscale::clients::planetscale::DismissSchemaRecommendationArgs;
use crate::providers::planetscale::clients::planetscale::EnableSafeMigrationsArgs;
use crate::providers::planetscale::clients::planetscale::PromoteBranchArgs;
use crate::providers::planetscale::clients::planetscale::QueueDeployRequestArgs;
use crate::providers::planetscale::clients::planetscale::ReassignRoleObjectsArgs;
use crate::providers::planetscale::clients::planetscale::RemoveOrganizationMemberArgs;
use crate::providers::planetscale::clients::planetscale::RemoveOrganizationTeamMemberArgs;
use crate::providers::planetscale::clients::planetscale::RenewPasswordArgs;
use crate::providers::planetscale::clients::planetscale::RenewRoleArgs;
use crate::providers::planetscale::clients::planetscale::ResetDefaultRoleArgs;
use crate::providers::planetscale::clients::planetscale::ResetRoleArgs;
use crate::providers::planetscale::clients::planetscale::ReviewDeployRequestArgs;
use crate::providers::planetscale::clients::planetscale::SkipRevertPeriodArgs;
use crate::providers::planetscale::clients::planetscale::TestWebhookArgs;
use crate::providers::planetscale::clients::planetscale::UpdateAutoApplyArgs;
use crate::providers::planetscale::clients::planetscale::UpdateAutoDeleteBranchArgs;
use crate::providers::planetscale::clients::planetscale::UpdateBackupArgs;
use crate::providers::planetscale::clients::planetscale::UpdateBackupPolicyArgs;
use crate::providers::planetscale::clients::planetscale::UpdateBouncerResizeRequestArgs;
use crate::providers::planetscale::clients::planetscale::UpdateBranchArgs;
use crate::providers::planetscale::clients::planetscale::UpdateBranchChangeRequestArgs;
use crate::providers::planetscale::clients::planetscale::UpdateBranchClusterConfigArgs;
use crate::providers::planetscale::clients::planetscale::UpdateDatabasePostgresCidrArgs;
use crate::providers::planetscale::clients::planetscale::UpdateDatabaseSettingsArgs;
use crate::providers::planetscale::clients::planetscale::UpdateDatabaseThrottlerArgs;
use crate::providers::planetscale::clients::planetscale::UpdateDeployRequestThrottlerArgs;
use crate::providers::planetscale::clients::planetscale::UpdateKeyspaceArgs;
use crate::providers::planetscale::clients::planetscale::UpdateKeyspaceVschemaArgs;
use crate::providers::planetscale::clients::planetscale::UpdateOrganizationArgs;
use crate::providers::planetscale::clients::planetscale::UpdateOrganizationMembershipArgs;
use crate::providers::planetscale::clients::planetscale::UpdateOrganizationTeamArgs;
use crate::providers::planetscale::clients::planetscale::UpdatePasswordArgs;
use crate::providers::planetscale::clients::planetscale::UpdateRoleArgs;
use crate::providers::planetscale::clients::planetscale::UpdateTrafficBudgetArgs;
use crate::providers::planetscale::clients::planetscale::UpdateWebhookArgs;
use crate::providers::planetscale::clients::planetscale::VerifyWorkflowArgs;
use crate::providers::planetscale::clients::planetscale::WorkflowCancelArgs;
use crate::providers::planetscale::clients::planetscale::WorkflowCompleteArgs;
use crate::providers::planetscale::clients::planetscale::WorkflowCutoverArgs;
use crate::providers::planetscale::clients::planetscale::WorkflowRetryArgs;
use crate::providers::planetscale::clients::planetscale::WorkflowReverseCutoverArgs;
use crate::providers::planetscale::clients::planetscale::WorkflowReverseTrafficArgs;
use crate::providers::planetscale::clients::planetscale::WorkflowSwitchPrimariesArgs;
use crate::providers::planetscale::clients::planetscale::WorkflowSwitchReplicasArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// PlanetscaleProvider with automatic state tracking.
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
/// let provider = PlanetscaleProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct PlanetscaleProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> PlanetscaleProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new PlanetscaleProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
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

    /// Delete query patterns report.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Update traffic budget.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete traffic budget.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

}
