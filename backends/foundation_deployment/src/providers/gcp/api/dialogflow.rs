//! DialogflowProvider - State-aware dialogflow API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       dialogflow API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::dialogflow::{
    dialogflow_projects_locations_agents_create_builder, dialogflow_projects_locations_agents_create_task,
    dialogflow_projects_locations_agents_delete_builder, dialogflow_projects_locations_agents_delete_task,
    dialogflow_projects_locations_agents_export_builder, dialogflow_projects_locations_agents_export_task,
    dialogflow_projects_locations_agents_patch_builder, dialogflow_projects_locations_agents_patch_task,
    dialogflow_projects_locations_agents_restore_builder, dialogflow_projects_locations_agents_restore_task,
    dialogflow_projects_locations_agents_update_generative_settings_builder, dialogflow_projects_locations_agents_update_generative_settings_task,
    dialogflow_projects_locations_agents_validate_builder, dialogflow_projects_locations_agents_validate_task,
    dialogflow_projects_locations_agents_entity_types_create_builder, dialogflow_projects_locations_agents_entity_types_create_task,
    dialogflow_projects_locations_agents_entity_types_delete_builder, dialogflow_projects_locations_agents_entity_types_delete_task,
    dialogflow_projects_locations_agents_entity_types_export_builder, dialogflow_projects_locations_agents_entity_types_export_task,
    dialogflow_projects_locations_agents_entity_types_import_builder, dialogflow_projects_locations_agents_entity_types_import_task,
    dialogflow_projects_locations_agents_entity_types_patch_builder, dialogflow_projects_locations_agents_entity_types_patch_task,
    dialogflow_projects_locations_agents_environments_create_builder, dialogflow_projects_locations_agents_environments_create_task,
    dialogflow_projects_locations_agents_environments_delete_builder, dialogflow_projects_locations_agents_environments_delete_task,
    dialogflow_projects_locations_agents_environments_deploy_flow_builder, dialogflow_projects_locations_agents_environments_deploy_flow_task,
    dialogflow_projects_locations_agents_environments_patch_builder, dialogflow_projects_locations_agents_environments_patch_task,
    dialogflow_projects_locations_agents_environments_run_continuous_test_builder, dialogflow_projects_locations_agents_environments_run_continuous_test_task,
    dialogflow_projects_locations_agents_environments_experiments_create_builder, dialogflow_projects_locations_agents_environments_experiments_create_task,
    dialogflow_projects_locations_agents_environments_experiments_delete_builder, dialogflow_projects_locations_agents_environments_experiments_delete_task,
    dialogflow_projects_locations_agents_environments_experiments_patch_builder, dialogflow_projects_locations_agents_environments_experiments_patch_task,
    dialogflow_projects_locations_agents_environments_experiments_start_builder, dialogflow_projects_locations_agents_environments_experiments_start_task,
    dialogflow_projects_locations_agents_environments_experiments_stop_builder, dialogflow_projects_locations_agents_environments_experiments_stop_task,
    dialogflow_projects_locations_agents_environments_sessions_detect_intent_builder, dialogflow_projects_locations_agents_environments_sessions_detect_intent_task,
    dialogflow_projects_locations_agents_environments_sessions_fulfill_intent_builder, dialogflow_projects_locations_agents_environments_sessions_fulfill_intent_task,
    dialogflow_projects_locations_agents_environments_sessions_match_intent_builder, dialogflow_projects_locations_agents_environments_sessions_match_intent_task,
    dialogflow_projects_locations_agents_environments_sessions_server_streaming_detect_intent_builder, dialogflow_projects_locations_agents_environments_sessions_server_streaming_detect_intent_task,
    dialogflow_projects_locations_agents_environments_sessions_entity_types_create_builder, dialogflow_projects_locations_agents_environments_sessions_entity_types_create_task,
    dialogflow_projects_locations_agents_environments_sessions_entity_types_delete_builder, dialogflow_projects_locations_agents_environments_sessions_entity_types_delete_task,
    dialogflow_projects_locations_agents_environments_sessions_entity_types_patch_builder, dialogflow_projects_locations_agents_environments_sessions_entity_types_patch_task,
    dialogflow_projects_locations_agents_flows_create_builder, dialogflow_projects_locations_agents_flows_create_task,
    dialogflow_projects_locations_agents_flows_delete_builder, dialogflow_projects_locations_agents_flows_delete_task,
    dialogflow_projects_locations_agents_flows_export_builder, dialogflow_projects_locations_agents_flows_export_task,
    dialogflow_projects_locations_agents_flows_import_builder, dialogflow_projects_locations_agents_flows_import_task,
    dialogflow_projects_locations_agents_flows_patch_builder, dialogflow_projects_locations_agents_flows_patch_task,
    dialogflow_projects_locations_agents_flows_train_builder, dialogflow_projects_locations_agents_flows_train_task,
    dialogflow_projects_locations_agents_flows_validate_builder, dialogflow_projects_locations_agents_flows_validate_task,
    dialogflow_projects_locations_agents_flows_pages_create_builder, dialogflow_projects_locations_agents_flows_pages_create_task,
    dialogflow_projects_locations_agents_flows_pages_delete_builder, dialogflow_projects_locations_agents_flows_pages_delete_task,
    dialogflow_projects_locations_agents_flows_pages_patch_builder, dialogflow_projects_locations_agents_flows_pages_patch_task,
    dialogflow_projects_locations_agents_flows_transition_route_groups_create_builder, dialogflow_projects_locations_agents_flows_transition_route_groups_create_task,
    dialogflow_projects_locations_agents_flows_transition_route_groups_delete_builder, dialogflow_projects_locations_agents_flows_transition_route_groups_delete_task,
    dialogflow_projects_locations_agents_flows_transition_route_groups_patch_builder, dialogflow_projects_locations_agents_flows_transition_route_groups_patch_task,
    dialogflow_projects_locations_agents_flows_versions_compare_versions_builder, dialogflow_projects_locations_agents_flows_versions_compare_versions_task,
    dialogflow_projects_locations_agents_flows_versions_create_builder, dialogflow_projects_locations_agents_flows_versions_create_task,
    dialogflow_projects_locations_agents_flows_versions_delete_builder, dialogflow_projects_locations_agents_flows_versions_delete_task,
    dialogflow_projects_locations_agents_flows_versions_load_builder, dialogflow_projects_locations_agents_flows_versions_load_task,
    dialogflow_projects_locations_agents_flows_versions_patch_builder, dialogflow_projects_locations_agents_flows_versions_patch_task,
    dialogflow_projects_locations_agents_generators_create_builder, dialogflow_projects_locations_agents_generators_create_task,
    dialogflow_projects_locations_agents_generators_delete_builder, dialogflow_projects_locations_agents_generators_delete_task,
    dialogflow_projects_locations_agents_generators_patch_builder, dialogflow_projects_locations_agents_generators_patch_task,
    dialogflow_projects_locations_agents_intents_create_builder, dialogflow_projects_locations_agents_intents_create_task,
    dialogflow_projects_locations_agents_intents_delete_builder, dialogflow_projects_locations_agents_intents_delete_task,
    dialogflow_projects_locations_agents_intents_export_builder, dialogflow_projects_locations_agents_intents_export_task,
    dialogflow_projects_locations_agents_intents_import_builder, dialogflow_projects_locations_agents_intents_import_task,
    dialogflow_projects_locations_agents_intents_patch_builder, dialogflow_projects_locations_agents_intents_patch_task,
    dialogflow_projects_locations_agents_playbooks_create_builder, dialogflow_projects_locations_agents_playbooks_create_task,
    dialogflow_projects_locations_agents_playbooks_delete_builder, dialogflow_projects_locations_agents_playbooks_delete_task,
    dialogflow_projects_locations_agents_playbooks_export_builder, dialogflow_projects_locations_agents_playbooks_export_task,
    dialogflow_projects_locations_agents_playbooks_import_builder, dialogflow_projects_locations_agents_playbooks_import_task,
    dialogflow_projects_locations_agents_playbooks_patch_builder, dialogflow_projects_locations_agents_playbooks_patch_task,
    dialogflow_projects_locations_agents_playbooks_examples_create_builder, dialogflow_projects_locations_agents_playbooks_examples_create_task,
    dialogflow_projects_locations_agents_playbooks_examples_delete_builder, dialogflow_projects_locations_agents_playbooks_examples_delete_task,
    dialogflow_projects_locations_agents_playbooks_examples_patch_builder, dialogflow_projects_locations_agents_playbooks_examples_patch_task,
    dialogflow_projects_locations_agents_playbooks_versions_create_builder, dialogflow_projects_locations_agents_playbooks_versions_create_task,
    dialogflow_projects_locations_agents_playbooks_versions_delete_builder, dialogflow_projects_locations_agents_playbooks_versions_delete_task,
    dialogflow_projects_locations_agents_playbooks_versions_restore_builder, dialogflow_projects_locations_agents_playbooks_versions_restore_task,
    dialogflow_projects_locations_agents_sessions_detect_intent_builder, dialogflow_projects_locations_agents_sessions_detect_intent_task,
    dialogflow_projects_locations_agents_sessions_fulfill_intent_builder, dialogflow_projects_locations_agents_sessions_fulfill_intent_task,
    dialogflow_projects_locations_agents_sessions_match_intent_builder, dialogflow_projects_locations_agents_sessions_match_intent_task,
    dialogflow_projects_locations_agents_sessions_server_streaming_detect_intent_builder, dialogflow_projects_locations_agents_sessions_server_streaming_detect_intent_task,
    dialogflow_projects_locations_agents_sessions_submit_answer_feedback_builder, dialogflow_projects_locations_agents_sessions_submit_answer_feedback_task,
    dialogflow_projects_locations_agents_sessions_entity_types_create_builder, dialogflow_projects_locations_agents_sessions_entity_types_create_task,
    dialogflow_projects_locations_agents_sessions_entity_types_delete_builder, dialogflow_projects_locations_agents_sessions_entity_types_delete_task,
    dialogflow_projects_locations_agents_sessions_entity_types_patch_builder, dialogflow_projects_locations_agents_sessions_entity_types_patch_task,
    dialogflow_projects_locations_agents_test_cases_batch_delete_builder, dialogflow_projects_locations_agents_test_cases_batch_delete_task,
    dialogflow_projects_locations_agents_test_cases_batch_run_builder, dialogflow_projects_locations_agents_test_cases_batch_run_task,
    dialogflow_projects_locations_agents_test_cases_create_builder, dialogflow_projects_locations_agents_test_cases_create_task,
    dialogflow_projects_locations_agents_test_cases_export_builder, dialogflow_projects_locations_agents_test_cases_export_task,
    dialogflow_projects_locations_agents_test_cases_import_builder, dialogflow_projects_locations_agents_test_cases_import_task,
    dialogflow_projects_locations_agents_test_cases_patch_builder, dialogflow_projects_locations_agents_test_cases_patch_task,
    dialogflow_projects_locations_agents_test_cases_run_builder, dialogflow_projects_locations_agents_test_cases_run_task,
    dialogflow_projects_locations_agents_tools_create_builder, dialogflow_projects_locations_agents_tools_create_task,
    dialogflow_projects_locations_agents_tools_delete_builder, dialogflow_projects_locations_agents_tools_delete_task,
    dialogflow_projects_locations_agents_tools_patch_builder, dialogflow_projects_locations_agents_tools_patch_task,
    dialogflow_projects_locations_agents_tools_versions_create_builder, dialogflow_projects_locations_agents_tools_versions_create_task,
    dialogflow_projects_locations_agents_tools_versions_delete_builder, dialogflow_projects_locations_agents_tools_versions_delete_task,
    dialogflow_projects_locations_agents_tools_versions_restore_builder, dialogflow_projects_locations_agents_tools_versions_restore_task,
    dialogflow_projects_locations_agents_transition_route_groups_create_builder, dialogflow_projects_locations_agents_transition_route_groups_create_task,
    dialogflow_projects_locations_agents_transition_route_groups_delete_builder, dialogflow_projects_locations_agents_transition_route_groups_delete_task,
    dialogflow_projects_locations_agents_transition_route_groups_patch_builder, dialogflow_projects_locations_agents_transition_route_groups_patch_task,
    dialogflow_projects_locations_agents_webhooks_create_builder, dialogflow_projects_locations_agents_webhooks_create_task,
    dialogflow_projects_locations_agents_webhooks_delete_builder, dialogflow_projects_locations_agents_webhooks_delete_task,
    dialogflow_projects_locations_agents_webhooks_patch_builder, dialogflow_projects_locations_agents_webhooks_patch_task,
    dialogflow_projects_locations_operations_cancel_builder, dialogflow_projects_locations_operations_cancel_task,
    dialogflow_projects_locations_security_settings_create_builder, dialogflow_projects_locations_security_settings_create_task,
    dialogflow_projects_locations_security_settings_delete_builder, dialogflow_projects_locations_security_settings_delete_task,
    dialogflow_projects_locations_security_settings_patch_builder, dialogflow_projects_locations_security_settings_patch_task,
    dialogflow_projects_operations_cancel_builder, dialogflow_projects_operations_cancel_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::dialogflow::GoogleCloudDialogflowCxV3Agent;
use crate::providers::gcp::clients::dialogflow::GoogleCloudDialogflowCxV3AgentValidationResult;
use crate::providers::gcp::clients::dialogflow::GoogleCloudDialogflowCxV3AnswerFeedback;
use crate::providers::gcp::clients::dialogflow::GoogleCloudDialogflowCxV3CompareVersionsResponse;
use crate::providers::gcp::clients::dialogflow::GoogleCloudDialogflowCxV3DetectIntentResponse;
use crate::providers::gcp::clients::dialogflow::GoogleCloudDialogflowCxV3EntityType;
use crate::providers::gcp::clients::dialogflow::GoogleCloudDialogflowCxV3Example;
use crate::providers::gcp::clients::dialogflow::GoogleCloudDialogflowCxV3Experiment;
use crate::providers::gcp::clients::dialogflow::GoogleCloudDialogflowCxV3Flow;
use crate::providers::gcp::clients::dialogflow::GoogleCloudDialogflowCxV3FlowValidationResult;
use crate::providers::gcp::clients::dialogflow::GoogleCloudDialogflowCxV3FulfillIntentResponse;
use crate::providers::gcp::clients::dialogflow::GoogleCloudDialogflowCxV3GenerativeSettings;
use crate::providers::gcp::clients::dialogflow::GoogleCloudDialogflowCxV3Generator;
use crate::providers::gcp::clients::dialogflow::GoogleCloudDialogflowCxV3Intent;
use crate::providers::gcp::clients::dialogflow::GoogleCloudDialogflowCxV3MatchIntentResponse;
use crate::providers::gcp::clients::dialogflow::GoogleCloudDialogflowCxV3Page;
use crate::providers::gcp::clients::dialogflow::GoogleCloudDialogflowCxV3Playbook;
use crate::providers::gcp::clients::dialogflow::GoogleCloudDialogflowCxV3PlaybookVersion;
use crate::providers::gcp::clients::dialogflow::GoogleCloudDialogflowCxV3RestorePlaybookVersionResponse;
use crate::providers::gcp::clients::dialogflow::GoogleCloudDialogflowCxV3RestoreToolVersionResponse;
use crate::providers::gcp::clients::dialogflow::GoogleCloudDialogflowCxV3SecuritySettings;
use crate::providers::gcp::clients::dialogflow::GoogleCloudDialogflowCxV3SessionEntityType;
use crate::providers::gcp::clients::dialogflow::GoogleCloudDialogflowCxV3TestCase;
use crate::providers::gcp::clients::dialogflow::GoogleCloudDialogflowCxV3Tool;
use crate::providers::gcp::clients::dialogflow::GoogleCloudDialogflowCxV3ToolVersion;
use crate::providers::gcp::clients::dialogflow::GoogleCloudDialogflowCxV3TransitionRouteGroup;
use crate::providers::gcp::clients::dialogflow::GoogleCloudDialogflowCxV3Version;
use crate::providers::gcp::clients::dialogflow::GoogleCloudDialogflowCxV3Webhook;
use crate::providers::gcp::clients::dialogflow::GoogleLongrunningOperation;
use crate::providers::gcp::clients::dialogflow::GoogleProtobufEmpty;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsCreateArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsDeleteArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsEntityTypesCreateArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsEntityTypesDeleteArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsEntityTypesExportArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsEntityTypesImportArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsEntityTypesPatchArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsEnvironmentsCreateArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsEnvironmentsDeleteArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsEnvironmentsDeployFlowArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsEnvironmentsExperimentsCreateArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsEnvironmentsExperimentsDeleteArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsEnvironmentsExperimentsPatchArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsEnvironmentsExperimentsStartArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsEnvironmentsExperimentsStopArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsEnvironmentsPatchArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsEnvironmentsRunContinuousTestArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsEnvironmentsSessionsDetectIntentArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsEnvironmentsSessionsEntityTypesCreateArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsEnvironmentsSessionsEntityTypesDeleteArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsEnvironmentsSessionsEntityTypesPatchArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsEnvironmentsSessionsFulfillIntentArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsEnvironmentsSessionsMatchIntentArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsEnvironmentsSessionsServerStreamingDetectIntentArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsExportArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsFlowsCreateArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsFlowsDeleteArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsFlowsExportArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsFlowsImportArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsFlowsPagesCreateArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsFlowsPagesDeleteArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsFlowsPagesPatchArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsFlowsPatchArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsFlowsTrainArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsFlowsTransitionRouteGroupsCreateArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsFlowsTransitionRouteGroupsDeleteArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsFlowsTransitionRouteGroupsPatchArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsFlowsValidateArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsFlowsVersionsCompareVersionsArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsFlowsVersionsCreateArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsFlowsVersionsDeleteArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsFlowsVersionsLoadArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsFlowsVersionsPatchArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsGeneratorsCreateArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsGeneratorsDeleteArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsGeneratorsPatchArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsIntentsCreateArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsIntentsDeleteArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsIntentsExportArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsIntentsImportArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsIntentsPatchArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsPatchArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsPlaybooksCreateArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsPlaybooksDeleteArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsPlaybooksExamplesCreateArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsPlaybooksExamplesDeleteArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsPlaybooksExamplesPatchArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsPlaybooksExportArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsPlaybooksImportArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsPlaybooksPatchArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsPlaybooksVersionsCreateArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsPlaybooksVersionsDeleteArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsPlaybooksVersionsRestoreArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsRestoreArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsSessionsDetectIntentArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsSessionsEntityTypesCreateArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsSessionsEntityTypesDeleteArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsSessionsEntityTypesPatchArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsSessionsFulfillIntentArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsSessionsMatchIntentArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsSessionsServerStreamingDetectIntentArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsSessionsSubmitAnswerFeedbackArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsTestCasesBatchDeleteArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsTestCasesBatchRunArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsTestCasesCreateArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsTestCasesExportArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsTestCasesImportArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsTestCasesPatchArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsTestCasesRunArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsToolsCreateArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsToolsDeleteArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsToolsPatchArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsToolsVersionsCreateArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsToolsVersionsDeleteArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsToolsVersionsRestoreArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsTransitionRouteGroupsCreateArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsTransitionRouteGroupsDeleteArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsTransitionRouteGroupsPatchArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsUpdateGenerativeSettingsArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsValidateArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsWebhooksCreateArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsWebhooksDeleteArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsAgentsWebhooksPatchArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsSecuritySettingsCreateArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsSecuritySettingsDeleteArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsLocationsSecuritySettingsPatchArgs;
use crate::providers::gcp::clients::dialogflow::DialogflowProjectsOperationsCancelArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// DialogflowProvider with automatic state tracking.
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
/// let provider = DialogflowProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct DialogflowProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> DialogflowProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new DialogflowProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Dialogflow projects locations agents create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3Agent result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_create(
        &self,
        args: &DialogflowProjectsLocationsAgentsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3Agent, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents delete.
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
    pub fn dialogflow_projects_locations_agents_delete(
        &self,
        args: &DialogflowProjectsLocationsAgentsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents export.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_export(
        &self,
        args: &DialogflowProjectsLocationsAgentsExportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_export_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_export_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3Agent result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_patch(
        &self,
        args: &DialogflowProjectsLocationsAgentsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3Agent, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents restore.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_restore(
        &self,
        args: &DialogflowProjectsLocationsAgentsRestoreArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_restore_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_restore_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents update generative settings.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3GenerativeSettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_update_generative_settings(
        &self,
        args: &DialogflowProjectsLocationsAgentsUpdateGenerativeSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3GenerativeSettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_update_generative_settings_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_update_generative_settings_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents validate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3AgentValidationResult result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_validate(
        &self,
        args: &DialogflowProjectsLocationsAgentsValidateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3AgentValidationResult, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_validate_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_validate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents entity types create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3EntityType result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_entity_types_create(
        &self,
        args: &DialogflowProjectsLocationsAgentsEntityTypesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3EntityType, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_entity_types_create_builder(
            &self.http_client,
            &args.parent,
            &args.languageCode,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_entity_types_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents entity types delete.
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
    pub fn dialogflow_projects_locations_agents_entity_types_delete(
        &self,
        args: &DialogflowProjectsLocationsAgentsEntityTypesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_entity_types_delete_builder(
            &self.http_client,
            &args.name,
            &args.force,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_entity_types_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents entity types export.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_entity_types_export(
        &self,
        args: &DialogflowProjectsLocationsAgentsEntityTypesExportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_entity_types_export_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_entity_types_export_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents entity types import.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_entity_types_import(
        &self,
        args: &DialogflowProjectsLocationsAgentsEntityTypesImportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_entity_types_import_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_entity_types_import_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents entity types patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3EntityType result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_entity_types_patch(
        &self,
        args: &DialogflowProjectsLocationsAgentsEntityTypesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3EntityType, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_entity_types_patch_builder(
            &self.http_client,
            &args.name,
            &args.languageCode,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_entity_types_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents environments create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_environments_create(
        &self,
        args: &DialogflowProjectsLocationsAgentsEnvironmentsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_environments_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_environments_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents environments delete.
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
    pub fn dialogflow_projects_locations_agents_environments_delete(
        &self,
        args: &DialogflowProjectsLocationsAgentsEnvironmentsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_environments_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_environments_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents environments deploy flow.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_environments_deploy_flow(
        &self,
        args: &DialogflowProjectsLocationsAgentsEnvironmentsDeployFlowArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_environments_deploy_flow_builder(
            &self.http_client,
            &args.environment,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_environments_deploy_flow_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents environments patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_environments_patch(
        &self,
        args: &DialogflowProjectsLocationsAgentsEnvironmentsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_environments_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_environments_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents environments run continuous test.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_environments_run_continuous_test(
        &self,
        args: &DialogflowProjectsLocationsAgentsEnvironmentsRunContinuousTestArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_environments_run_continuous_test_builder(
            &self.http_client,
            &args.environment,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_environments_run_continuous_test_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents environments experiments create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3Experiment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_environments_experiments_create(
        &self,
        args: &DialogflowProjectsLocationsAgentsEnvironmentsExperimentsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3Experiment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_environments_experiments_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_environments_experiments_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents environments experiments delete.
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
    pub fn dialogflow_projects_locations_agents_environments_experiments_delete(
        &self,
        args: &DialogflowProjectsLocationsAgentsEnvironmentsExperimentsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_environments_experiments_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_environments_experiments_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents environments experiments patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3Experiment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_environments_experiments_patch(
        &self,
        args: &DialogflowProjectsLocationsAgentsEnvironmentsExperimentsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3Experiment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_environments_experiments_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_environments_experiments_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents environments experiments start.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3Experiment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_environments_experiments_start(
        &self,
        args: &DialogflowProjectsLocationsAgentsEnvironmentsExperimentsStartArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3Experiment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_environments_experiments_start_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_environments_experiments_start_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents environments experiments stop.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3Experiment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_environments_experiments_stop(
        &self,
        args: &DialogflowProjectsLocationsAgentsEnvironmentsExperimentsStopArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3Experiment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_environments_experiments_stop_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_environments_experiments_stop_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents environments sessions detect intent.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3DetectIntentResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_environments_sessions_detect_intent(
        &self,
        args: &DialogflowProjectsLocationsAgentsEnvironmentsSessionsDetectIntentArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3DetectIntentResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_environments_sessions_detect_intent_builder(
            &self.http_client,
            &args.session,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_environments_sessions_detect_intent_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents environments sessions fulfill intent.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3FulfillIntentResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_environments_sessions_fulfill_intent(
        &self,
        args: &DialogflowProjectsLocationsAgentsEnvironmentsSessionsFulfillIntentArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3FulfillIntentResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_environments_sessions_fulfill_intent_builder(
            &self.http_client,
            &args.session,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_environments_sessions_fulfill_intent_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents environments sessions match intent.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3MatchIntentResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_environments_sessions_match_intent(
        &self,
        args: &DialogflowProjectsLocationsAgentsEnvironmentsSessionsMatchIntentArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3MatchIntentResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_environments_sessions_match_intent_builder(
            &self.http_client,
            &args.session,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_environments_sessions_match_intent_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents environments sessions server streaming detect intent.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3DetectIntentResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_environments_sessions_server_streaming_detect_intent(
        &self,
        args: &DialogflowProjectsLocationsAgentsEnvironmentsSessionsServerStreamingDetectIntentArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3DetectIntentResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_environments_sessions_server_streaming_detect_intent_builder(
            &self.http_client,
            &args.session,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_environments_sessions_server_streaming_detect_intent_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents environments sessions entity types create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3SessionEntityType result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_environments_sessions_entity_types_create(
        &self,
        args: &DialogflowProjectsLocationsAgentsEnvironmentsSessionsEntityTypesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3SessionEntityType, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_environments_sessions_entity_types_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_environments_sessions_entity_types_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents environments sessions entity types delete.
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
    pub fn dialogflow_projects_locations_agents_environments_sessions_entity_types_delete(
        &self,
        args: &DialogflowProjectsLocationsAgentsEnvironmentsSessionsEntityTypesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_environments_sessions_entity_types_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_environments_sessions_entity_types_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents environments sessions entity types patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3SessionEntityType result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_environments_sessions_entity_types_patch(
        &self,
        args: &DialogflowProjectsLocationsAgentsEnvironmentsSessionsEntityTypesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3SessionEntityType, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_environments_sessions_entity_types_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_environments_sessions_entity_types_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents flows create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3Flow result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_flows_create(
        &self,
        args: &DialogflowProjectsLocationsAgentsFlowsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3Flow, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_flows_create_builder(
            &self.http_client,
            &args.parent,
            &args.languageCode,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_flows_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents flows delete.
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
    pub fn dialogflow_projects_locations_agents_flows_delete(
        &self,
        args: &DialogflowProjectsLocationsAgentsFlowsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_flows_delete_builder(
            &self.http_client,
            &args.name,
            &args.force,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_flows_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents flows export.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_flows_export(
        &self,
        args: &DialogflowProjectsLocationsAgentsFlowsExportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_flows_export_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_flows_export_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents flows import.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_flows_import(
        &self,
        args: &DialogflowProjectsLocationsAgentsFlowsImportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_flows_import_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_flows_import_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents flows patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3Flow result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_flows_patch(
        &self,
        args: &DialogflowProjectsLocationsAgentsFlowsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3Flow, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_flows_patch_builder(
            &self.http_client,
            &args.name,
            &args.languageCode,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_flows_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents flows train.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_flows_train(
        &self,
        args: &DialogflowProjectsLocationsAgentsFlowsTrainArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_flows_train_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_flows_train_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents flows validate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3FlowValidationResult result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_flows_validate(
        &self,
        args: &DialogflowProjectsLocationsAgentsFlowsValidateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3FlowValidationResult, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_flows_validate_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_flows_validate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents flows pages create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3Page result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_flows_pages_create(
        &self,
        args: &DialogflowProjectsLocationsAgentsFlowsPagesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3Page, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_flows_pages_create_builder(
            &self.http_client,
            &args.parent,
            &args.languageCode,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_flows_pages_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents flows pages delete.
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
    pub fn dialogflow_projects_locations_agents_flows_pages_delete(
        &self,
        args: &DialogflowProjectsLocationsAgentsFlowsPagesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_flows_pages_delete_builder(
            &self.http_client,
            &args.name,
            &args.force,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_flows_pages_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents flows pages patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3Page result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_flows_pages_patch(
        &self,
        args: &DialogflowProjectsLocationsAgentsFlowsPagesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3Page, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_flows_pages_patch_builder(
            &self.http_client,
            &args.name,
            &args.languageCode,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_flows_pages_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents flows transition route groups create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3TransitionRouteGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_flows_transition_route_groups_create(
        &self,
        args: &DialogflowProjectsLocationsAgentsFlowsTransitionRouteGroupsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3TransitionRouteGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_flows_transition_route_groups_create_builder(
            &self.http_client,
            &args.parent,
            &args.languageCode,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_flows_transition_route_groups_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents flows transition route groups delete.
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
    pub fn dialogflow_projects_locations_agents_flows_transition_route_groups_delete(
        &self,
        args: &DialogflowProjectsLocationsAgentsFlowsTransitionRouteGroupsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_flows_transition_route_groups_delete_builder(
            &self.http_client,
            &args.name,
            &args.force,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_flows_transition_route_groups_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents flows transition route groups patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3TransitionRouteGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_flows_transition_route_groups_patch(
        &self,
        args: &DialogflowProjectsLocationsAgentsFlowsTransitionRouteGroupsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3TransitionRouteGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_flows_transition_route_groups_patch_builder(
            &self.http_client,
            &args.name,
            &args.languageCode,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_flows_transition_route_groups_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents flows versions compare versions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3CompareVersionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_flows_versions_compare_versions(
        &self,
        args: &DialogflowProjectsLocationsAgentsFlowsVersionsCompareVersionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3CompareVersionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_flows_versions_compare_versions_builder(
            &self.http_client,
            &args.baseVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_flows_versions_compare_versions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents flows versions create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_flows_versions_create(
        &self,
        args: &DialogflowProjectsLocationsAgentsFlowsVersionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_flows_versions_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_flows_versions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents flows versions delete.
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
    pub fn dialogflow_projects_locations_agents_flows_versions_delete(
        &self,
        args: &DialogflowProjectsLocationsAgentsFlowsVersionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_flows_versions_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_flows_versions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents flows versions load.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_flows_versions_load(
        &self,
        args: &DialogflowProjectsLocationsAgentsFlowsVersionsLoadArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_flows_versions_load_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_flows_versions_load_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents flows versions patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3Version result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_flows_versions_patch(
        &self,
        args: &DialogflowProjectsLocationsAgentsFlowsVersionsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3Version, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_flows_versions_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_flows_versions_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents generators create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3Generator result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_generators_create(
        &self,
        args: &DialogflowProjectsLocationsAgentsGeneratorsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3Generator, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_generators_create_builder(
            &self.http_client,
            &args.parent,
            &args.languageCode,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_generators_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents generators delete.
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
    pub fn dialogflow_projects_locations_agents_generators_delete(
        &self,
        args: &DialogflowProjectsLocationsAgentsGeneratorsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_generators_delete_builder(
            &self.http_client,
            &args.name,
            &args.force,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_generators_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents generators patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3Generator result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_generators_patch(
        &self,
        args: &DialogflowProjectsLocationsAgentsGeneratorsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3Generator, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_generators_patch_builder(
            &self.http_client,
            &args.name,
            &args.languageCode,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_generators_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents intents create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3Intent result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_intents_create(
        &self,
        args: &DialogflowProjectsLocationsAgentsIntentsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3Intent, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_intents_create_builder(
            &self.http_client,
            &args.parent,
            &args.languageCode,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_intents_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents intents delete.
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
    pub fn dialogflow_projects_locations_agents_intents_delete(
        &self,
        args: &DialogflowProjectsLocationsAgentsIntentsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_intents_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_intents_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents intents export.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_intents_export(
        &self,
        args: &DialogflowProjectsLocationsAgentsIntentsExportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_intents_export_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_intents_export_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents intents import.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_intents_import(
        &self,
        args: &DialogflowProjectsLocationsAgentsIntentsImportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_intents_import_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_intents_import_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents intents patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3Intent result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_intents_patch(
        &self,
        args: &DialogflowProjectsLocationsAgentsIntentsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3Intent, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_intents_patch_builder(
            &self.http_client,
            &args.name,
            &args.languageCode,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_intents_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents playbooks create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3Playbook result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_playbooks_create(
        &self,
        args: &DialogflowProjectsLocationsAgentsPlaybooksCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3Playbook, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_playbooks_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_playbooks_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents playbooks delete.
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
    pub fn dialogflow_projects_locations_agents_playbooks_delete(
        &self,
        args: &DialogflowProjectsLocationsAgentsPlaybooksDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_playbooks_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_playbooks_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents playbooks export.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_playbooks_export(
        &self,
        args: &DialogflowProjectsLocationsAgentsPlaybooksExportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_playbooks_export_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_playbooks_export_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents playbooks import.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_playbooks_import(
        &self,
        args: &DialogflowProjectsLocationsAgentsPlaybooksImportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_playbooks_import_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_playbooks_import_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents playbooks patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3Playbook result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_playbooks_patch(
        &self,
        args: &DialogflowProjectsLocationsAgentsPlaybooksPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3Playbook, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_playbooks_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_playbooks_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents playbooks examples create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3Example result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_playbooks_examples_create(
        &self,
        args: &DialogflowProjectsLocationsAgentsPlaybooksExamplesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3Example, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_playbooks_examples_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_playbooks_examples_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents playbooks examples delete.
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
    pub fn dialogflow_projects_locations_agents_playbooks_examples_delete(
        &self,
        args: &DialogflowProjectsLocationsAgentsPlaybooksExamplesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_playbooks_examples_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_playbooks_examples_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents playbooks examples patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3Example result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_playbooks_examples_patch(
        &self,
        args: &DialogflowProjectsLocationsAgentsPlaybooksExamplesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3Example, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_playbooks_examples_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_playbooks_examples_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents playbooks versions create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3PlaybookVersion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_playbooks_versions_create(
        &self,
        args: &DialogflowProjectsLocationsAgentsPlaybooksVersionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3PlaybookVersion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_playbooks_versions_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_playbooks_versions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents playbooks versions delete.
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
    pub fn dialogflow_projects_locations_agents_playbooks_versions_delete(
        &self,
        args: &DialogflowProjectsLocationsAgentsPlaybooksVersionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_playbooks_versions_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_playbooks_versions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents playbooks versions restore.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3RestorePlaybookVersionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_playbooks_versions_restore(
        &self,
        args: &DialogflowProjectsLocationsAgentsPlaybooksVersionsRestoreArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3RestorePlaybookVersionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_playbooks_versions_restore_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_playbooks_versions_restore_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents sessions detect intent.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3DetectIntentResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_sessions_detect_intent(
        &self,
        args: &DialogflowProjectsLocationsAgentsSessionsDetectIntentArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3DetectIntentResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_sessions_detect_intent_builder(
            &self.http_client,
            &args.session,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_sessions_detect_intent_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents sessions fulfill intent.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3FulfillIntentResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_sessions_fulfill_intent(
        &self,
        args: &DialogflowProjectsLocationsAgentsSessionsFulfillIntentArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3FulfillIntentResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_sessions_fulfill_intent_builder(
            &self.http_client,
            &args.session,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_sessions_fulfill_intent_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents sessions match intent.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3MatchIntentResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_sessions_match_intent(
        &self,
        args: &DialogflowProjectsLocationsAgentsSessionsMatchIntentArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3MatchIntentResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_sessions_match_intent_builder(
            &self.http_client,
            &args.session,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_sessions_match_intent_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents sessions server streaming detect intent.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3DetectIntentResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_sessions_server_streaming_detect_intent(
        &self,
        args: &DialogflowProjectsLocationsAgentsSessionsServerStreamingDetectIntentArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3DetectIntentResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_sessions_server_streaming_detect_intent_builder(
            &self.http_client,
            &args.session,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_sessions_server_streaming_detect_intent_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents sessions submit answer feedback.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3AnswerFeedback result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_sessions_submit_answer_feedback(
        &self,
        args: &DialogflowProjectsLocationsAgentsSessionsSubmitAnswerFeedbackArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3AnswerFeedback, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_sessions_submit_answer_feedback_builder(
            &self.http_client,
            &args.session,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_sessions_submit_answer_feedback_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents sessions entity types create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3SessionEntityType result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_sessions_entity_types_create(
        &self,
        args: &DialogflowProjectsLocationsAgentsSessionsEntityTypesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3SessionEntityType, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_sessions_entity_types_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_sessions_entity_types_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents sessions entity types delete.
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
    pub fn dialogflow_projects_locations_agents_sessions_entity_types_delete(
        &self,
        args: &DialogflowProjectsLocationsAgentsSessionsEntityTypesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_sessions_entity_types_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_sessions_entity_types_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents sessions entity types patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3SessionEntityType result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_sessions_entity_types_patch(
        &self,
        args: &DialogflowProjectsLocationsAgentsSessionsEntityTypesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3SessionEntityType, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_sessions_entity_types_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_sessions_entity_types_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents test cases batch delete.
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
    pub fn dialogflow_projects_locations_agents_test_cases_batch_delete(
        &self,
        args: &DialogflowProjectsLocationsAgentsTestCasesBatchDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_test_cases_batch_delete_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_test_cases_batch_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents test cases batch run.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_test_cases_batch_run(
        &self,
        args: &DialogflowProjectsLocationsAgentsTestCasesBatchRunArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_test_cases_batch_run_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_test_cases_batch_run_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents test cases create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3TestCase result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_test_cases_create(
        &self,
        args: &DialogflowProjectsLocationsAgentsTestCasesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3TestCase, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_test_cases_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_test_cases_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents test cases export.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_test_cases_export(
        &self,
        args: &DialogflowProjectsLocationsAgentsTestCasesExportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_test_cases_export_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_test_cases_export_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents test cases import.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_test_cases_import(
        &self,
        args: &DialogflowProjectsLocationsAgentsTestCasesImportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_test_cases_import_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_test_cases_import_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents test cases patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3TestCase result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_test_cases_patch(
        &self,
        args: &DialogflowProjectsLocationsAgentsTestCasesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3TestCase, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_test_cases_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_test_cases_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents test cases run.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_test_cases_run(
        &self,
        args: &DialogflowProjectsLocationsAgentsTestCasesRunArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_test_cases_run_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_test_cases_run_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents tools create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3Tool result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_tools_create(
        &self,
        args: &DialogflowProjectsLocationsAgentsToolsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3Tool, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_tools_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_tools_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents tools delete.
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
    pub fn dialogflow_projects_locations_agents_tools_delete(
        &self,
        args: &DialogflowProjectsLocationsAgentsToolsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_tools_delete_builder(
            &self.http_client,
            &args.name,
            &args.force,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_tools_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents tools patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3Tool result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_tools_patch(
        &self,
        args: &DialogflowProjectsLocationsAgentsToolsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3Tool, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_tools_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_tools_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents tools versions create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3ToolVersion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_tools_versions_create(
        &self,
        args: &DialogflowProjectsLocationsAgentsToolsVersionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3ToolVersion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_tools_versions_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_tools_versions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents tools versions delete.
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
    pub fn dialogflow_projects_locations_agents_tools_versions_delete(
        &self,
        args: &DialogflowProjectsLocationsAgentsToolsVersionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_tools_versions_delete_builder(
            &self.http_client,
            &args.name,
            &args.force,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_tools_versions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents tools versions restore.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3RestoreToolVersionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_tools_versions_restore(
        &self,
        args: &DialogflowProjectsLocationsAgentsToolsVersionsRestoreArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3RestoreToolVersionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_tools_versions_restore_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_tools_versions_restore_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents transition route groups create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3TransitionRouteGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_transition_route_groups_create(
        &self,
        args: &DialogflowProjectsLocationsAgentsTransitionRouteGroupsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3TransitionRouteGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_transition_route_groups_create_builder(
            &self.http_client,
            &args.parent,
            &args.languageCode,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_transition_route_groups_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents transition route groups delete.
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
    pub fn dialogflow_projects_locations_agents_transition_route_groups_delete(
        &self,
        args: &DialogflowProjectsLocationsAgentsTransitionRouteGroupsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_transition_route_groups_delete_builder(
            &self.http_client,
            &args.name,
            &args.force,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_transition_route_groups_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents transition route groups patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3TransitionRouteGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_transition_route_groups_patch(
        &self,
        args: &DialogflowProjectsLocationsAgentsTransitionRouteGroupsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3TransitionRouteGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_transition_route_groups_patch_builder(
            &self.http_client,
            &args.name,
            &args.languageCode,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_transition_route_groups_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents webhooks create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3Webhook result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_webhooks_create(
        &self,
        args: &DialogflowProjectsLocationsAgentsWebhooksCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3Webhook, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_webhooks_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_webhooks_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents webhooks delete.
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
    pub fn dialogflow_projects_locations_agents_webhooks_delete(
        &self,
        args: &DialogflowProjectsLocationsAgentsWebhooksDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_webhooks_delete_builder(
            &self.http_client,
            &args.name,
            &args.force,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_webhooks_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations agents webhooks patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3Webhook result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_agents_webhooks_patch(
        &self,
        args: &DialogflowProjectsLocationsAgentsWebhooksPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3Webhook, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_agents_webhooks_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_agents_webhooks_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations operations cancel.
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
    pub fn dialogflow_projects_locations_operations_cancel(
        &self,
        args: &DialogflowProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations security settings create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3SecuritySettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_security_settings_create(
        &self,
        args: &DialogflowProjectsLocationsSecuritySettingsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3SecuritySettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_security_settings_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_security_settings_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations security settings delete.
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
    pub fn dialogflow_projects_locations_security_settings_delete(
        &self,
        args: &DialogflowProjectsLocationsSecuritySettingsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_security_settings_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_security_settings_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects locations security settings patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDialogflowCxV3SecuritySettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dialogflow_projects_locations_security_settings_patch(
        &self,
        args: &DialogflowProjectsLocationsSecuritySettingsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDialogflowCxV3SecuritySettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_locations_security_settings_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_locations_security_settings_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dialogflow projects operations cancel.
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
    pub fn dialogflow_projects_operations_cancel(
        &self,
        args: &DialogflowProjectsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dialogflow_projects_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dialogflow_projects_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
