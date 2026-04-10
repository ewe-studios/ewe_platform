//! SecuritycenterProvider - State-aware securitycenter API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       securitycenter API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::securitycenter::{
    securitycenter_folders_assets_group_builder, securitycenter_folders_assets_group_task,
    securitycenter_folders_assets_update_security_marks_builder, securitycenter_folders_assets_update_security_marks_task,
    securitycenter_folders_big_query_exports_create_builder, securitycenter_folders_big_query_exports_create_task,
    securitycenter_folders_big_query_exports_delete_builder, securitycenter_folders_big_query_exports_delete_task,
    securitycenter_folders_big_query_exports_patch_builder, securitycenter_folders_big_query_exports_patch_task,
    securitycenter_folders_event_threat_detection_settings_validate_custom_module_builder, securitycenter_folders_event_threat_detection_settings_validate_custom_module_task,
    securitycenter_folders_event_threat_detection_settings_custom_modules_create_builder, securitycenter_folders_event_threat_detection_settings_custom_modules_create_task,
    securitycenter_folders_event_threat_detection_settings_custom_modules_delete_builder, securitycenter_folders_event_threat_detection_settings_custom_modules_delete_task,
    securitycenter_folders_event_threat_detection_settings_custom_modules_patch_builder, securitycenter_folders_event_threat_detection_settings_custom_modules_patch_task,
    securitycenter_folders_findings_bulk_mute_builder, securitycenter_folders_findings_bulk_mute_task,
    securitycenter_folders_locations_mute_configs_delete_builder, securitycenter_folders_locations_mute_configs_delete_task,
    securitycenter_folders_locations_mute_configs_patch_builder, securitycenter_folders_locations_mute_configs_patch_task,
    securitycenter_folders_mute_configs_create_builder, securitycenter_folders_mute_configs_create_task,
    securitycenter_folders_mute_configs_delete_builder, securitycenter_folders_mute_configs_delete_task,
    securitycenter_folders_mute_configs_patch_builder, securitycenter_folders_mute_configs_patch_task,
    securitycenter_folders_notification_configs_create_builder, securitycenter_folders_notification_configs_create_task,
    securitycenter_folders_notification_configs_delete_builder, securitycenter_folders_notification_configs_delete_task,
    securitycenter_folders_notification_configs_patch_builder, securitycenter_folders_notification_configs_patch_task,
    securitycenter_folders_security_health_analytics_settings_custom_modules_create_builder, securitycenter_folders_security_health_analytics_settings_custom_modules_create_task,
    securitycenter_folders_security_health_analytics_settings_custom_modules_delete_builder, securitycenter_folders_security_health_analytics_settings_custom_modules_delete_task,
    securitycenter_folders_security_health_analytics_settings_custom_modules_patch_builder, securitycenter_folders_security_health_analytics_settings_custom_modules_patch_task,
    securitycenter_folders_security_health_analytics_settings_custom_modules_simulate_builder, securitycenter_folders_security_health_analytics_settings_custom_modules_simulate_task,
    securitycenter_folders_sources_findings_group_builder, securitycenter_folders_sources_findings_group_task,
    securitycenter_folders_sources_findings_patch_builder, securitycenter_folders_sources_findings_patch_task,
    securitycenter_folders_sources_findings_set_mute_builder, securitycenter_folders_sources_findings_set_mute_task,
    securitycenter_folders_sources_findings_set_state_builder, securitycenter_folders_sources_findings_set_state_task,
    securitycenter_folders_sources_findings_update_security_marks_builder, securitycenter_folders_sources_findings_update_security_marks_task,
    securitycenter_folders_sources_findings_external_systems_patch_builder, securitycenter_folders_sources_findings_external_systems_patch_task,
    securitycenter_organizations_update_organization_settings_builder, securitycenter_organizations_update_organization_settings_task,
    securitycenter_organizations_assets_group_builder, securitycenter_organizations_assets_group_task,
    securitycenter_organizations_assets_run_discovery_builder, securitycenter_organizations_assets_run_discovery_task,
    securitycenter_organizations_assets_update_security_marks_builder, securitycenter_organizations_assets_update_security_marks_task,
    securitycenter_organizations_big_query_exports_create_builder, securitycenter_organizations_big_query_exports_create_task,
    securitycenter_organizations_big_query_exports_delete_builder, securitycenter_organizations_big_query_exports_delete_task,
    securitycenter_organizations_big_query_exports_patch_builder, securitycenter_organizations_big_query_exports_patch_task,
    securitycenter_organizations_event_threat_detection_settings_validate_custom_module_builder, securitycenter_organizations_event_threat_detection_settings_validate_custom_module_task,
    securitycenter_organizations_event_threat_detection_settings_custom_modules_create_builder, securitycenter_organizations_event_threat_detection_settings_custom_modules_create_task,
    securitycenter_organizations_event_threat_detection_settings_custom_modules_delete_builder, securitycenter_organizations_event_threat_detection_settings_custom_modules_delete_task,
    securitycenter_organizations_event_threat_detection_settings_custom_modules_patch_builder, securitycenter_organizations_event_threat_detection_settings_custom_modules_patch_task,
    securitycenter_organizations_findings_bulk_mute_builder, securitycenter_organizations_findings_bulk_mute_task,
    securitycenter_organizations_locations_mute_configs_delete_builder, securitycenter_organizations_locations_mute_configs_delete_task,
    securitycenter_organizations_locations_mute_configs_patch_builder, securitycenter_organizations_locations_mute_configs_patch_task,
    securitycenter_organizations_mute_configs_create_builder, securitycenter_organizations_mute_configs_create_task,
    securitycenter_organizations_mute_configs_delete_builder, securitycenter_organizations_mute_configs_delete_task,
    securitycenter_organizations_mute_configs_patch_builder, securitycenter_organizations_mute_configs_patch_task,
    securitycenter_organizations_notification_configs_create_builder, securitycenter_organizations_notification_configs_create_task,
    securitycenter_organizations_notification_configs_delete_builder, securitycenter_organizations_notification_configs_delete_task,
    securitycenter_organizations_notification_configs_patch_builder, securitycenter_organizations_notification_configs_patch_task,
    securitycenter_organizations_operations_cancel_builder, securitycenter_organizations_operations_cancel_task,
    securitycenter_organizations_operations_delete_builder, securitycenter_organizations_operations_delete_task,
    securitycenter_organizations_resource_value_configs_batch_create_builder, securitycenter_organizations_resource_value_configs_batch_create_task,
    securitycenter_organizations_resource_value_configs_delete_builder, securitycenter_organizations_resource_value_configs_delete_task,
    securitycenter_organizations_resource_value_configs_patch_builder, securitycenter_organizations_resource_value_configs_patch_task,
    securitycenter_organizations_security_health_analytics_settings_custom_modules_create_builder, securitycenter_organizations_security_health_analytics_settings_custom_modules_create_task,
    securitycenter_organizations_security_health_analytics_settings_custom_modules_delete_builder, securitycenter_organizations_security_health_analytics_settings_custom_modules_delete_task,
    securitycenter_organizations_security_health_analytics_settings_custom_modules_patch_builder, securitycenter_organizations_security_health_analytics_settings_custom_modules_patch_task,
    securitycenter_organizations_security_health_analytics_settings_custom_modules_simulate_builder, securitycenter_organizations_security_health_analytics_settings_custom_modules_simulate_task,
    securitycenter_organizations_sources_create_builder, securitycenter_organizations_sources_create_task,
    securitycenter_organizations_sources_get_iam_policy_builder, securitycenter_organizations_sources_get_iam_policy_task,
    securitycenter_organizations_sources_patch_builder, securitycenter_organizations_sources_patch_task,
    securitycenter_organizations_sources_set_iam_policy_builder, securitycenter_organizations_sources_set_iam_policy_task,
    securitycenter_organizations_sources_test_iam_permissions_builder, securitycenter_organizations_sources_test_iam_permissions_task,
    securitycenter_organizations_sources_findings_create_builder, securitycenter_organizations_sources_findings_create_task,
    securitycenter_organizations_sources_findings_group_builder, securitycenter_organizations_sources_findings_group_task,
    securitycenter_organizations_sources_findings_patch_builder, securitycenter_organizations_sources_findings_patch_task,
    securitycenter_organizations_sources_findings_set_mute_builder, securitycenter_organizations_sources_findings_set_mute_task,
    securitycenter_organizations_sources_findings_set_state_builder, securitycenter_organizations_sources_findings_set_state_task,
    securitycenter_organizations_sources_findings_update_security_marks_builder, securitycenter_organizations_sources_findings_update_security_marks_task,
    securitycenter_organizations_sources_findings_external_systems_patch_builder, securitycenter_organizations_sources_findings_external_systems_patch_task,
    securitycenter_projects_assets_group_builder, securitycenter_projects_assets_group_task,
    securitycenter_projects_assets_update_security_marks_builder, securitycenter_projects_assets_update_security_marks_task,
    securitycenter_projects_big_query_exports_create_builder, securitycenter_projects_big_query_exports_create_task,
    securitycenter_projects_big_query_exports_delete_builder, securitycenter_projects_big_query_exports_delete_task,
    securitycenter_projects_big_query_exports_patch_builder, securitycenter_projects_big_query_exports_patch_task,
    securitycenter_projects_event_threat_detection_settings_validate_custom_module_builder, securitycenter_projects_event_threat_detection_settings_validate_custom_module_task,
    securitycenter_projects_event_threat_detection_settings_custom_modules_create_builder, securitycenter_projects_event_threat_detection_settings_custom_modules_create_task,
    securitycenter_projects_event_threat_detection_settings_custom_modules_delete_builder, securitycenter_projects_event_threat_detection_settings_custom_modules_delete_task,
    securitycenter_projects_event_threat_detection_settings_custom_modules_patch_builder, securitycenter_projects_event_threat_detection_settings_custom_modules_patch_task,
    securitycenter_projects_findings_bulk_mute_builder, securitycenter_projects_findings_bulk_mute_task,
    securitycenter_projects_locations_mute_configs_delete_builder, securitycenter_projects_locations_mute_configs_delete_task,
    securitycenter_projects_locations_mute_configs_patch_builder, securitycenter_projects_locations_mute_configs_patch_task,
    securitycenter_projects_mute_configs_create_builder, securitycenter_projects_mute_configs_create_task,
    securitycenter_projects_mute_configs_delete_builder, securitycenter_projects_mute_configs_delete_task,
    securitycenter_projects_mute_configs_patch_builder, securitycenter_projects_mute_configs_patch_task,
    securitycenter_projects_notification_configs_create_builder, securitycenter_projects_notification_configs_create_task,
    securitycenter_projects_notification_configs_delete_builder, securitycenter_projects_notification_configs_delete_task,
    securitycenter_projects_notification_configs_patch_builder, securitycenter_projects_notification_configs_patch_task,
    securitycenter_projects_security_health_analytics_settings_custom_modules_create_builder, securitycenter_projects_security_health_analytics_settings_custom_modules_create_task,
    securitycenter_projects_security_health_analytics_settings_custom_modules_delete_builder, securitycenter_projects_security_health_analytics_settings_custom_modules_delete_task,
    securitycenter_projects_security_health_analytics_settings_custom_modules_patch_builder, securitycenter_projects_security_health_analytics_settings_custom_modules_patch_task,
    securitycenter_projects_security_health_analytics_settings_custom_modules_simulate_builder, securitycenter_projects_security_health_analytics_settings_custom_modules_simulate_task,
    securitycenter_projects_sources_findings_group_builder, securitycenter_projects_sources_findings_group_task,
    securitycenter_projects_sources_findings_patch_builder, securitycenter_projects_sources_findings_patch_task,
    securitycenter_projects_sources_findings_set_mute_builder, securitycenter_projects_sources_findings_set_mute_task,
    securitycenter_projects_sources_findings_set_state_builder, securitycenter_projects_sources_findings_set_state_task,
    securitycenter_projects_sources_findings_update_security_marks_builder, securitycenter_projects_sources_findings_update_security_marks_task,
    securitycenter_projects_sources_findings_external_systems_patch_builder, securitycenter_projects_sources_findings_external_systems_patch_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::securitycenter::BatchCreateResourceValueConfigsResponse;
use crate::providers::gcp::clients::securitycenter::Empty;
use crate::providers::gcp::clients::securitycenter::EventThreatDetectionCustomModule;
use crate::providers::gcp::clients::securitycenter::Finding;
use crate::providers::gcp::clients::securitycenter::GoogleCloudSecuritycenterV1BigQueryExport;
use crate::providers::gcp::clients::securitycenter::GoogleCloudSecuritycenterV1ExternalSystem;
use crate::providers::gcp::clients::securitycenter::GoogleCloudSecuritycenterV1MuteConfig;
use crate::providers::gcp::clients::securitycenter::GoogleCloudSecuritycenterV1ResourceValueConfig;
use crate::providers::gcp::clients::securitycenter::GoogleCloudSecuritycenterV1SecurityHealthAnalyticsCustomModule;
use crate::providers::gcp::clients::securitycenter::GroupAssetsResponse;
use crate::providers::gcp::clients::securitycenter::GroupFindingsResponse;
use crate::providers::gcp::clients::securitycenter::NotificationConfig;
use crate::providers::gcp::clients::securitycenter::Operation;
use crate::providers::gcp::clients::securitycenter::OrganizationSettings;
use crate::providers::gcp::clients::securitycenter::Policy;
use crate::providers::gcp::clients::securitycenter::SecurityMarks;
use crate::providers::gcp::clients::securitycenter::SimulateSecurityHealthAnalyticsCustomModuleResponse;
use crate::providers::gcp::clients::securitycenter::Source;
use crate::providers::gcp::clients::securitycenter::TestIamPermissionsResponse;
use crate::providers::gcp::clients::securitycenter::ValidateEventThreatDetectionCustomModuleResponse;
use crate::providers::gcp::clients::securitycenter::SecuritycenterFoldersAssetsGroupArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterFoldersAssetsUpdateSecurityMarksArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterFoldersBigQueryExportsCreateArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterFoldersBigQueryExportsDeleteArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterFoldersBigQueryExportsPatchArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterFoldersEventThreatDetectionSettingsCustomModulesCreateArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterFoldersEventThreatDetectionSettingsCustomModulesDeleteArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterFoldersEventThreatDetectionSettingsCustomModulesPatchArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterFoldersEventThreatDetectionSettingsValidateCustomModuleArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterFoldersFindingsBulkMuteArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterFoldersLocationsMuteConfigsDeleteArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterFoldersLocationsMuteConfigsPatchArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterFoldersMuteConfigsCreateArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterFoldersMuteConfigsDeleteArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterFoldersMuteConfigsPatchArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterFoldersNotificationConfigsCreateArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterFoldersNotificationConfigsDeleteArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterFoldersNotificationConfigsPatchArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterFoldersSecurityHealthAnalyticsSettingsCustomModulesCreateArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterFoldersSecurityHealthAnalyticsSettingsCustomModulesDeleteArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterFoldersSecurityHealthAnalyticsSettingsCustomModulesPatchArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterFoldersSecurityHealthAnalyticsSettingsCustomModulesSimulateArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterFoldersSourcesFindingsExternalSystemsPatchArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterFoldersSourcesFindingsGroupArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterFoldersSourcesFindingsPatchArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterFoldersSourcesFindingsSetMuteArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterFoldersSourcesFindingsSetStateArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterFoldersSourcesFindingsUpdateSecurityMarksArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterOrganizationsAssetsGroupArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterOrganizationsAssetsRunDiscoveryArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterOrganizationsAssetsUpdateSecurityMarksArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterOrganizationsBigQueryExportsCreateArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterOrganizationsBigQueryExportsDeleteArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterOrganizationsBigQueryExportsPatchArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterOrganizationsEventThreatDetectionSettingsCustomModulesCreateArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterOrganizationsEventThreatDetectionSettingsCustomModulesDeleteArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterOrganizationsEventThreatDetectionSettingsCustomModulesPatchArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterOrganizationsEventThreatDetectionSettingsValidateCustomModuleArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterOrganizationsFindingsBulkMuteArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterOrganizationsLocationsMuteConfigsDeleteArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterOrganizationsLocationsMuteConfigsPatchArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterOrganizationsMuteConfigsCreateArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterOrganizationsMuteConfigsDeleteArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterOrganizationsMuteConfigsPatchArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterOrganizationsNotificationConfigsCreateArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterOrganizationsNotificationConfigsDeleteArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterOrganizationsNotificationConfigsPatchArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterOrganizationsOperationsCancelArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterOrganizationsOperationsDeleteArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterOrganizationsResourceValueConfigsBatchCreateArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterOrganizationsResourceValueConfigsDeleteArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterOrganizationsResourceValueConfigsPatchArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterOrganizationsSecurityHealthAnalyticsSettingsCustomModulesCreateArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterOrganizationsSecurityHealthAnalyticsSettingsCustomModulesDeleteArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterOrganizationsSecurityHealthAnalyticsSettingsCustomModulesPatchArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterOrganizationsSecurityHealthAnalyticsSettingsCustomModulesSimulateArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterOrganizationsSourcesCreateArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterOrganizationsSourcesFindingsCreateArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterOrganizationsSourcesFindingsExternalSystemsPatchArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterOrganizationsSourcesFindingsGroupArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterOrganizationsSourcesFindingsPatchArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterOrganizationsSourcesFindingsSetMuteArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterOrganizationsSourcesFindingsSetStateArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterOrganizationsSourcesFindingsUpdateSecurityMarksArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterOrganizationsSourcesGetIamPolicyArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterOrganizationsSourcesPatchArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterOrganizationsSourcesSetIamPolicyArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterOrganizationsSourcesTestIamPermissionsArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterOrganizationsUpdateOrganizationSettingsArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterProjectsAssetsGroupArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterProjectsAssetsUpdateSecurityMarksArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterProjectsBigQueryExportsCreateArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterProjectsBigQueryExportsDeleteArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterProjectsBigQueryExportsPatchArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterProjectsEventThreatDetectionSettingsCustomModulesCreateArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterProjectsEventThreatDetectionSettingsCustomModulesDeleteArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterProjectsEventThreatDetectionSettingsCustomModulesPatchArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterProjectsEventThreatDetectionSettingsValidateCustomModuleArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterProjectsFindingsBulkMuteArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterProjectsLocationsMuteConfigsDeleteArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterProjectsLocationsMuteConfigsPatchArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterProjectsMuteConfigsCreateArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterProjectsMuteConfigsDeleteArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterProjectsMuteConfigsPatchArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterProjectsNotificationConfigsCreateArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterProjectsNotificationConfigsDeleteArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterProjectsNotificationConfigsPatchArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterProjectsSecurityHealthAnalyticsSettingsCustomModulesCreateArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterProjectsSecurityHealthAnalyticsSettingsCustomModulesDeleteArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterProjectsSecurityHealthAnalyticsSettingsCustomModulesPatchArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterProjectsSecurityHealthAnalyticsSettingsCustomModulesSimulateArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterProjectsSourcesFindingsExternalSystemsPatchArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterProjectsSourcesFindingsGroupArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterProjectsSourcesFindingsPatchArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterProjectsSourcesFindingsSetMuteArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterProjectsSourcesFindingsSetStateArgs;
use crate::providers::gcp::clients::securitycenter::SecuritycenterProjectsSourcesFindingsUpdateSecurityMarksArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// SecuritycenterProvider with automatic state tracking.
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
/// let provider = SecuritycenterProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct SecuritycenterProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> SecuritycenterProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new SecuritycenterProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Securitycenter folders assets group.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GroupAssetsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_folders_assets_group(
        &self,
        args: &SecuritycenterFoldersAssetsGroupArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GroupAssetsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_folders_assets_group_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_folders_assets_group_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter folders assets update security marks.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SecurityMarks result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_folders_assets_update_security_marks(
        &self,
        args: &SecuritycenterFoldersAssetsUpdateSecurityMarksArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SecurityMarks, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_folders_assets_update_security_marks_builder(
            &self.http_client,
            &args.name,
            &args.startTime,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_folders_assets_update_security_marks_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter folders big query exports create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudSecuritycenterV1BigQueryExport result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_folders_big_query_exports_create(
        &self,
        args: &SecuritycenterFoldersBigQueryExportsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudSecuritycenterV1BigQueryExport, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_folders_big_query_exports_create_builder(
            &self.http_client,
            &args.parent,
            &args.bigQueryExportId,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_folders_big_query_exports_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter folders big query exports delete.
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
    pub fn securitycenter_folders_big_query_exports_delete(
        &self,
        args: &SecuritycenterFoldersBigQueryExportsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_folders_big_query_exports_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_folders_big_query_exports_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter folders big query exports patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudSecuritycenterV1BigQueryExport result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_folders_big_query_exports_patch(
        &self,
        args: &SecuritycenterFoldersBigQueryExportsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudSecuritycenterV1BigQueryExport, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_folders_big_query_exports_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_folders_big_query_exports_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter folders event threat detection settings validate custom module.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ValidateEventThreatDetectionCustomModuleResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_folders_event_threat_detection_settings_validate_custom_module(
        &self,
        args: &SecuritycenterFoldersEventThreatDetectionSettingsValidateCustomModuleArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ValidateEventThreatDetectionCustomModuleResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_folders_event_threat_detection_settings_validate_custom_module_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_folders_event_threat_detection_settings_validate_custom_module_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter folders event threat detection settings custom modules create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EventThreatDetectionCustomModule result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_folders_event_threat_detection_settings_custom_modules_create(
        &self,
        args: &SecuritycenterFoldersEventThreatDetectionSettingsCustomModulesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EventThreatDetectionCustomModule, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_folders_event_threat_detection_settings_custom_modules_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_folders_event_threat_detection_settings_custom_modules_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter folders event threat detection settings custom modules delete.
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
    pub fn securitycenter_folders_event_threat_detection_settings_custom_modules_delete(
        &self,
        args: &SecuritycenterFoldersEventThreatDetectionSettingsCustomModulesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_folders_event_threat_detection_settings_custom_modules_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_folders_event_threat_detection_settings_custom_modules_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter folders event threat detection settings custom modules patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EventThreatDetectionCustomModule result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_folders_event_threat_detection_settings_custom_modules_patch(
        &self,
        args: &SecuritycenterFoldersEventThreatDetectionSettingsCustomModulesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EventThreatDetectionCustomModule, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_folders_event_threat_detection_settings_custom_modules_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_folders_event_threat_detection_settings_custom_modules_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter folders findings bulk mute.
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
    pub fn securitycenter_folders_findings_bulk_mute(
        &self,
        args: &SecuritycenterFoldersFindingsBulkMuteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_folders_findings_bulk_mute_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_folders_findings_bulk_mute_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter folders locations mute configs delete.
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
    pub fn securitycenter_folders_locations_mute_configs_delete(
        &self,
        args: &SecuritycenterFoldersLocationsMuteConfigsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_folders_locations_mute_configs_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_folders_locations_mute_configs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter folders locations mute configs patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudSecuritycenterV1MuteConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_folders_locations_mute_configs_patch(
        &self,
        args: &SecuritycenterFoldersLocationsMuteConfigsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudSecuritycenterV1MuteConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_folders_locations_mute_configs_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_folders_locations_mute_configs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter folders mute configs create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudSecuritycenterV1MuteConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_folders_mute_configs_create(
        &self,
        args: &SecuritycenterFoldersMuteConfigsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudSecuritycenterV1MuteConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_folders_mute_configs_create_builder(
            &self.http_client,
            &args.parent,
            &args.muteConfigId,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_folders_mute_configs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter folders mute configs delete.
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
    pub fn securitycenter_folders_mute_configs_delete(
        &self,
        args: &SecuritycenterFoldersMuteConfigsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_folders_mute_configs_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_folders_mute_configs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter folders mute configs patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudSecuritycenterV1MuteConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_folders_mute_configs_patch(
        &self,
        args: &SecuritycenterFoldersMuteConfigsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudSecuritycenterV1MuteConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_folders_mute_configs_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_folders_mute_configs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter folders notification configs create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NotificationConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_folders_notification_configs_create(
        &self,
        args: &SecuritycenterFoldersNotificationConfigsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NotificationConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_folders_notification_configs_create_builder(
            &self.http_client,
            &args.parent,
            &args.configId,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_folders_notification_configs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter folders notification configs delete.
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
    pub fn securitycenter_folders_notification_configs_delete(
        &self,
        args: &SecuritycenterFoldersNotificationConfigsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_folders_notification_configs_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_folders_notification_configs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter folders notification configs patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NotificationConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_folders_notification_configs_patch(
        &self,
        args: &SecuritycenterFoldersNotificationConfigsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NotificationConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_folders_notification_configs_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_folders_notification_configs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter folders security health analytics settings custom modules create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudSecuritycenterV1SecurityHealthAnalyticsCustomModule result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_folders_security_health_analytics_settings_custom_modules_create(
        &self,
        args: &SecuritycenterFoldersSecurityHealthAnalyticsSettingsCustomModulesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudSecuritycenterV1SecurityHealthAnalyticsCustomModule, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_folders_security_health_analytics_settings_custom_modules_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_folders_security_health_analytics_settings_custom_modules_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter folders security health analytics settings custom modules delete.
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
    pub fn securitycenter_folders_security_health_analytics_settings_custom_modules_delete(
        &self,
        args: &SecuritycenterFoldersSecurityHealthAnalyticsSettingsCustomModulesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_folders_security_health_analytics_settings_custom_modules_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_folders_security_health_analytics_settings_custom_modules_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter folders security health analytics settings custom modules patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudSecuritycenterV1SecurityHealthAnalyticsCustomModule result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_folders_security_health_analytics_settings_custom_modules_patch(
        &self,
        args: &SecuritycenterFoldersSecurityHealthAnalyticsSettingsCustomModulesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudSecuritycenterV1SecurityHealthAnalyticsCustomModule, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_folders_security_health_analytics_settings_custom_modules_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_folders_security_health_analytics_settings_custom_modules_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter folders security health analytics settings custom modules simulate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SimulateSecurityHealthAnalyticsCustomModuleResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_folders_security_health_analytics_settings_custom_modules_simulate(
        &self,
        args: &SecuritycenterFoldersSecurityHealthAnalyticsSettingsCustomModulesSimulateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SimulateSecurityHealthAnalyticsCustomModuleResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_folders_security_health_analytics_settings_custom_modules_simulate_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_folders_security_health_analytics_settings_custom_modules_simulate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter folders sources findings group.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GroupFindingsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_folders_sources_findings_group(
        &self,
        args: &SecuritycenterFoldersSourcesFindingsGroupArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GroupFindingsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_folders_sources_findings_group_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_folders_sources_findings_group_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter folders sources findings patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Finding result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_folders_sources_findings_patch(
        &self,
        args: &SecuritycenterFoldersSourcesFindingsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Finding, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_folders_sources_findings_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_folders_sources_findings_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter folders sources findings set mute.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Finding result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_folders_sources_findings_set_mute(
        &self,
        args: &SecuritycenterFoldersSourcesFindingsSetMuteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Finding, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_folders_sources_findings_set_mute_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_folders_sources_findings_set_mute_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter folders sources findings set state.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Finding result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_folders_sources_findings_set_state(
        &self,
        args: &SecuritycenterFoldersSourcesFindingsSetStateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Finding, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_folders_sources_findings_set_state_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_folders_sources_findings_set_state_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter folders sources findings update security marks.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SecurityMarks result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_folders_sources_findings_update_security_marks(
        &self,
        args: &SecuritycenterFoldersSourcesFindingsUpdateSecurityMarksArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SecurityMarks, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_folders_sources_findings_update_security_marks_builder(
            &self.http_client,
            &args.name,
            &args.startTime,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_folders_sources_findings_update_security_marks_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter folders sources findings external systems patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudSecuritycenterV1ExternalSystem result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_folders_sources_findings_external_systems_patch(
        &self,
        args: &SecuritycenterFoldersSourcesFindingsExternalSystemsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudSecuritycenterV1ExternalSystem, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_folders_sources_findings_external_systems_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_folders_sources_findings_external_systems_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter organizations update organization settings.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the OrganizationSettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_organizations_update_organization_settings(
        &self,
        args: &SecuritycenterOrganizationsUpdateOrganizationSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<OrganizationSettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_organizations_update_organization_settings_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_organizations_update_organization_settings_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter organizations assets group.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GroupAssetsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_organizations_assets_group(
        &self,
        args: &SecuritycenterOrganizationsAssetsGroupArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GroupAssetsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_organizations_assets_group_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_organizations_assets_group_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter organizations assets run discovery.
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
    pub fn securitycenter_organizations_assets_run_discovery(
        &self,
        args: &SecuritycenterOrganizationsAssetsRunDiscoveryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_organizations_assets_run_discovery_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_organizations_assets_run_discovery_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter organizations assets update security marks.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SecurityMarks result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_organizations_assets_update_security_marks(
        &self,
        args: &SecuritycenterOrganizationsAssetsUpdateSecurityMarksArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SecurityMarks, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_organizations_assets_update_security_marks_builder(
            &self.http_client,
            &args.name,
            &args.startTime,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_organizations_assets_update_security_marks_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter organizations big query exports create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudSecuritycenterV1BigQueryExport result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_organizations_big_query_exports_create(
        &self,
        args: &SecuritycenterOrganizationsBigQueryExportsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudSecuritycenterV1BigQueryExport, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_organizations_big_query_exports_create_builder(
            &self.http_client,
            &args.parent,
            &args.bigQueryExportId,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_organizations_big_query_exports_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter organizations big query exports delete.
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
    pub fn securitycenter_organizations_big_query_exports_delete(
        &self,
        args: &SecuritycenterOrganizationsBigQueryExportsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_organizations_big_query_exports_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_organizations_big_query_exports_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter organizations big query exports patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudSecuritycenterV1BigQueryExport result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_organizations_big_query_exports_patch(
        &self,
        args: &SecuritycenterOrganizationsBigQueryExportsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudSecuritycenterV1BigQueryExport, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_organizations_big_query_exports_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_organizations_big_query_exports_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter organizations event threat detection settings validate custom module.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ValidateEventThreatDetectionCustomModuleResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_organizations_event_threat_detection_settings_validate_custom_module(
        &self,
        args: &SecuritycenterOrganizationsEventThreatDetectionSettingsValidateCustomModuleArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ValidateEventThreatDetectionCustomModuleResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_organizations_event_threat_detection_settings_validate_custom_module_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_organizations_event_threat_detection_settings_validate_custom_module_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter organizations event threat detection settings custom modules create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EventThreatDetectionCustomModule result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_organizations_event_threat_detection_settings_custom_modules_create(
        &self,
        args: &SecuritycenterOrganizationsEventThreatDetectionSettingsCustomModulesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EventThreatDetectionCustomModule, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_organizations_event_threat_detection_settings_custom_modules_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_organizations_event_threat_detection_settings_custom_modules_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter organizations event threat detection settings custom modules delete.
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
    pub fn securitycenter_organizations_event_threat_detection_settings_custom_modules_delete(
        &self,
        args: &SecuritycenterOrganizationsEventThreatDetectionSettingsCustomModulesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_organizations_event_threat_detection_settings_custom_modules_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_organizations_event_threat_detection_settings_custom_modules_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter organizations event threat detection settings custom modules patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EventThreatDetectionCustomModule result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_organizations_event_threat_detection_settings_custom_modules_patch(
        &self,
        args: &SecuritycenterOrganizationsEventThreatDetectionSettingsCustomModulesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EventThreatDetectionCustomModule, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_organizations_event_threat_detection_settings_custom_modules_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_organizations_event_threat_detection_settings_custom_modules_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter organizations findings bulk mute.
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
    pub fn securitycenter_organizations_findings_bulk_mute(
        &self,
        args: &SecuritycenterOrganizationsFindingsBulkMuteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_organizations_findings_bulk_mute_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_organizations_findings_bulk_mute_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter organizations locations mute configs delete.
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
    pub fn securitycenter_organizations_locations_mute_configs_delete(
        &self,
        args: &SecuritycenterOrganizationsLocationsMuteConfigsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_organizations_locations_mute_configs_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_organizations_locations_mute_configs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter organizations locations mute configs patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudSecuritycenterV1MuteConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_organizations_locations_mute_configs_patch(
        &self,
        args: &SecuritycenterOrganizationsLocationsMuteConfigsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudSecuritycenterV1MuteConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_organizations_locations_mute_configs_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_organizations_locations_mute_configs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter organizations mute configs create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudSecuritycenterV1MuteConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_organizations_mute_configs_create(
        &self,
        args: &SecuritycenterOrganizationsMuteConfigsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudSecuritycenterV1MuteConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_organizations_mute_configs_create_builder(
            &self.http_client,
            &args.parent,
            &args.muteConfigId,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_organizations_mute_configs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter organizations mute configs delete.
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
    pub fn securitycenter_organizations_mute_configs_delete(
        &self,
        args: &SecuritycenterOrganizationsMuteConfigsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_organizations_mute_configs_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_organizations_mute_configs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter organizations mute configs patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudSecuritycenterV1MuteConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_organizations_mute_configs_patch(
        &self,
        args: &SecuritycenterOrganizationsMuteConfigsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudSecuritycenterV1MuteConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_organizations_mute_configs_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_organizations_mute_configs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter organizations notification configs create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NotificationConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_organizations_notification_configs_create(
        &self,
        args: &SecuritycenterOrganizationsNotificationConfigsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NotificationConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_organizations_notification_configs_create_builder(
            &self.http_client,
            &args.parent,
            &args.configId,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_organizations_notification_configs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter organizations notification configs delete.
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
    pub fn securitycenter_organizations_notification_configs_delete(
        &self,
        args: &SecuritycenterOrganizationsNotificationConfigsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_organizations_notification_configs_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_organizations_notification_configs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter organizations notification configs patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NotificationConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_organizations_notification_configs_patch(
        &self,
        args: &SecuritycenterOrganizationsNotificationConfigsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NotificationConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_organizations_notification_configs_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_organizations_notification_configs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter organizations operations cancel.
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
    pub fn securitycenter_organizations_operations_cancel(
        &self,
        args: &SecuritycenterOrganizationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_organizations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_organizations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter organizations operations delete.
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
    pub fn securitycenter_organizations_operations_delete(
        &self,
        args: &SecuritycenterOrganizationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_organizations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_organizations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter organizations resource value configs batch create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchCreateResourceValueConfigsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_organizations_resource_value_configs_batch_create(
        &self,
        args: &SecuritycenterOrganizationsResourceValueConfigsBatchCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchCreateResourceValueConfigsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_organizations_resource_value_configs_batch_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_organizations_resource_value_configs_batch_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter organizations resource value configs delete.
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
    pub fn securitycenter_organizations_resource_value_configs_delete(
        &self,
        args: &SecuritycenterOrganizationsResourceValueConfigsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_organizations_resource_value_configs_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_organizations_resource_value_configs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter organizations resource value configs patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudSecuritycenterV1ResourceValueConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_organizations_resource_value_configs_patch(
        &self,
        args: &SecuritycenterOrganizationsResourceValueConfigsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudSecuritycenterV1ResourceValueConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_organizations_resource_value_configs_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_organizations_resource_value_configs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter organizations security health analytics settings custom modules create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudSecuritycenterV1SecurityHealthAnalyticsCustomModule result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_organizations_security_health_analytics_settings_custom_modules_create(
        &self,
        args: &SecuritycenterOrganizationsSecurityHealthAnalyticsSettingsCustomModulesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudSecuritycenterV1SecurityHealthAnalyticsCustomModule, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_organizations_security_health_analytics_settings_custom_modules_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_organizations_security_health_analytics_settings_custom_modules_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter organizations security health analytics settings custom modules delete.
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
    pub fn securitycenter_organizations_security_health_analytics_settings_custom_modules_delete(
        &self,
        args: &SecuritycenterOrganizationsSecurityHealthAnalyticsSettingsCustomModulesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_organizations_security_health_analytics_settings_custom_modules_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_organizations_security_health_analytics_settings_custom_modules_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter organizations security health analytics settings custom modules patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudSecuritycenterV1SecurityHealthAnalyticsCustomModule result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_organizations_security_health_analytics_settings_custom_modules_patch(
        &self,
        args: &SecuritycenterOrganizationsSecurityHealthAnalyticsSettingsCustomModulesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudSecuritycenterV1SecurityHealthAnalyticsCustomModule, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_organizations_security_health_analytics_settings_custom_modules_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_organizations_security_health_analytics_settings_custom_modules_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter organizations security health analytics settings custom modules simulate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SimulateSecurityHealthAnalyticsCustomModuleResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_organizations_security_health_analytics_settings_custom_modules_simulate(
        &self,
        args: &SecuritycenterOrganizationsSecurityHealthAnalyticsSettingsCustomModulesSimulateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SimulateSecurityHealthAnalyticsCustomModuleResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_organizations_security_health_analytics_settings_custom_modules_simulate_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_organizations_security_health_analytics_settings_custom_modules_simulate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter organizations sources create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Source result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_organizations_sources_create(
        &self,
        args: &SecuritycenterOrganizationsSourcesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Source, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_organizations_sources_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_organizations_sources_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter organizations sources get iam policy.
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
    pub fn securitycenter_organizations_sources_get_iam_policy(
        &self,
        args: &SecuritycenterOrganizationsSourcesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_organizations_sources_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_organizations_sources_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter organizations sources patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Source result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_organizations_sources_patch(
        &self,
        args: &SecuritycenterOrganizationsSourcesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Source, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_organizations_sources_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_organizations_sources_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter organizations sources set iam policy.
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
    pub fn securitycenter_organizations_sources_set_iam_policy(
        &self,
        args: &SecuritycenterOrganizationsSourcesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_organizations_sources_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_organizations_sources_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter organizations sources test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_organizations_sources_test_iam_permissions(
        &self,
        args: &SecuritycenterOrganizationsSourcesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_organizations_sources_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_organizations_sources_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter organizations sources findings create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Finding result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_organizations_sources_findings_create(
        &self,
        args: &SecuritycenterOrganizationsSourcesFindingsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Finding, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_organizations_sources_findings_create_builder(
            &self.http_client,
            &args.parent,
            &args.findingId,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_organizations_sources_findings_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter organizations sources findings group.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GroupFindingsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_organizations_sources_findings_group(
        &self,
        args: &SecuritycenterOrganizationsSourcesFindingsGroupArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GroupFindingsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_organizations_sources_findings_group_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_organizations_sources_findings_group_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter organizations sources findings patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Finding result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_organizations_sources_findings_patch(
        &self,
        args: &SecuritycenterOrganizationsSourcesFindingsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Finding, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_organizations_sources_findings_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_organizations_sources_findings_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter organizations sources findings set mute.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Finding result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_organizations_sources_findings_set_mute(
        &self,
        args: &SecuritycenterOrganizationsSourcesFindingsSetMuteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Finding, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_organizations_sources_findings_set_mute_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_organizations_sources_findings_set_mute_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter organizations sources findings set state.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Finding result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_organizations_sources_findings_set_state(
        &self,
        args: &SecuritycenterOrganizationsSourcesFindingsSetStateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Finding, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_organizations_sources_findings_set_state_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_organizations_sources_findings_set_state_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter organizations sources findings update security marks.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SecurityMarks result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_organizations_sources_findings_update_security_marks(
        &self,
        args: &SecuritycenterOrganizationsSourcesFindingsUpdateSecurityMarksArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SecurityMarks, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_organizations_sources_findings_update_security_marks_builder(
            &self.http_client,
            &args.name,
            &args.startTime,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_organizations_sources_findings_update_security_marks_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter organizations sources findings external systems patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudSecuritycenterV1ExternalSystem result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_organizations_sources_findings_external_systems_patch(
        &self,
        args: &SecuritycenterOrganizationsSourcesFindingsExternalSystemsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudSecuritycenterV1ExternalSystem, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_organizations_sources_findings_external_systems_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_organizations_sources_findings_external_systems_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter projects assets group.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GroupAssetsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_projects_assets_group(
        &self,
        args: &SecuritycenterProjectsAssetsGroupArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GroupAssetsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_projects_assets_group_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_projects_assets_group_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter projects assets update security marks.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SecurityMarks result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_projects_assets_update_security_marks(
        &self,
        args: &SecuritycenterProjectsAssetsUpdateSecurityMarksArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SecurityMarks, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_projects_assets_update_security_marks_builder(
            &self.http_client,
            &args.name,
            &args.startTime,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_projects_assets_update_security_marks_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter projects big query exports create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudSecuritycenterV1BigQueryExport result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_projects_big_query_exports_create(
        &self,
        args: &SecuritycenterProjectsBigQueryExportsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudSecuritycenterV1BigQueryExport, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_projects_big_query_exports_create_builder(
            &self.http_client,
            &args.parent,
            &args.bigQueryExportId,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_projects_big_query_exports_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter projects big query exports delete.
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
    pub fn securitycenter_projects_big_query_exports_delete(
        &self,
        args: &SecuritycenterProjectsBigQueryExportsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_projects_big_query_exports_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_projects_big_query_exports_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter projects big query exports patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudSecuritycenterV1BigQueryExport result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_projects_big_query_exports_patch(
        &self,
        args: &SecuritycenterProjectsBigQueryExportsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudSecuritycenterV1BigQueryExport, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_projects_big_query_exports_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_projects_big_query_exports_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter projects event threat detection settings validate custom module.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ValidateEventThreatDetectionCustomModuleResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_projects_event_threat_detection_settings_validate_custom_module(
        &self,
        args: &SecuritycenterProjectsEventThreatDetectionSettingsValidateCustomModuleArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ValidateEventThreatDetectionCustomModuleResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_projects_event_threat_detection_settings_validate_custom_module_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_projects_event_threat_detection_settings_validate_custom_module_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter projects event threat detection settings custom modules create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EventThreatDetectionCustomModule result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_projects_event_threat_detection_settings_custom_modules_create(
        &self,
        args: &SecuritycenterProjectsEventThreatDetectionSettingsCustomModulesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EventThreatDetectionCustomModule, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_projects_event_threat_detection_settings_custom_modules_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_projects_event_threat_detection_settings_custom_modules_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter projects event threat detection settings custom modules delete.
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
    pub fn securitycenter_projects_event_threat_detection_settings_custom_modules_delete(
        &self,
        args: &SecuritycenterProjectsEventThreatDetectionSettingsCustomModulesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_projects_event_threat_detection_settings_custom_modules_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_projects_event_threat_detection_settings_custom_modules_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter projects event threat detection settings custom modules patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EventThreatDetectionCustomModule result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_projects_event_threat_detection_settings_custom_modules_patch(
        &self,
        args: &SecuritycenterProjectsEventThreatDetectionSettingsCustomModulesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EventThreatDetectionCustomModule, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_projects_event_threat_detection_settings_custom_modules_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_projects_event_threat_detection_settings_custom_modules_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter projects findings bulk mute.
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
    pub fn securitycenter_projects_findings_bulk_mute(
        &self,
        args: &SecuritycenterProjectsFindingsBulkMuteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_projects_findings_bulk_mute_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_projects_findings_bulk_mute_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter projects locations mute configs delete.
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
    pub fn securitycenter_projects_locations_mute_configs_delete(
        &self,
        args: &SecuritycenterProjectsLocationsMuteConfigsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_projects_locations_mute_configs_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_projects_locations_mute_configs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter projects locations mute configs patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudSecuritycenterV1MuteConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_projects_locations_mute_configs_patch(
        &self,
        args: &SecuritycenterProjectsLocationsMuteConfigsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudSecuritycenterV1MuteConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_projects_locations_mute_configs_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_projects_locations_mute_configs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter projects mute configs create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudSecuritycenterV1MuteConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_projects_mute_configs_create(
        &self,
        args: &SecuritycenterProjectsMuteConfigsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudSecuritycenterV1MuteConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_projects_mute_configs_create_builder(
            &self.http_client,
            &args.parent,
            &args.muteConfigId,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_projects_mute_configs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter projects mute configs delete.
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
    pub fn securitycenter_projects_mute_configs_delete(
        &self,
        args: &SecuritycenterProjectsMuteConfigsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_projects_mute_configs_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_projects_mute_configs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter projects mute configs patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudSecuritycenterV1MuteConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_projects_mute_configs_patch(
        &self,
        args: &SecuritycenterProjectsMuteConfigsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudSecuritycenterV1MuteConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_projects_mute_configs_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_projects_mute_configs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter projects notification configs create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NotificationConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_projects_notification_configs_create(
        &self,
        args: &SecuritycenterProjectsNotificationConfigsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NotificationConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_projects_notification_configs_create_builder(
            &self.http_client,
            &args.parent,
            &args.configId,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_projects_notification_configs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter projects notification configs delete.
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
    pub fn securitycenter_projects_notification_configs_delete(
        &self,
        args: &SecuritycenterProjectsNotificationConfigsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_projects_notification_configs_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_projects_notification_configs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter projects notification configs patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NotificationConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_projects_notification_configs_patch(
        &self,
        args: &SecuritycenterProjectsNotificationConfigsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NotificationConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_projects_notification_configs_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_projects_notification_configs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter projects security health analytics settings custom modules create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudSecuritycenterV1SecurityHealthAnalyticsCustomModule result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_projects_security_health_analytics_settings_custom_modules_create(
        &self,
        args: &SecuritycenterProjectsSecurityHealthAnalyticsSettingsCustomModulesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudSecuritycenterV1SecurityHealthAnalyticsCustomModule, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_projects_security_health_analytics_settings_custom_modules_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_projects_security_health_analytics_settings_custom_modules_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter projects security health analytics settings custom modules delete.
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
    pub fn securitycenter_projects_security_health_analytics_settings_custom_modules_delete(
        &self,
        args: &SecuritycenterProjectsSecurityHealthAnalyticsSettingsCustomModulesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_projects_security_health_analytics_settings_custom_modules_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_projects_security_health_analytics_settings_custom_modules_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter projects security health analytics settings custom modules patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudSecuritycenterV1SecurityHealthAnalyticsCustomModule result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_projects_security_health_analytics_settings_custom_modules_patch(
        &self,
        args: &SecuritycenterProjectsSecurityHealthAnalyticsSettingsCustomModulesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudSecuritycenterV1SecurityHealthAnalyticsCustomModule, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_projects_security_health_analytics_settings_custom_modules_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_projects_security_health_analytics_settings_custom_modules_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter projects security health analytics settings custom modules simulate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SimulateSecurityHealthAnalyticsCustomModuleResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_projects_security_health_analytics_settings_custom_modules_simulate(
        &self,
        args: &SecuritycenterProjectsSecurityHealthAnalyticsSettingsCustomModulesSimulateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SimulateSecurityHealthAnalyticsCustomModuleResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_projects_security_health_analytics_settings_custom_modules_simulate_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_projects_security_health_analytics_settings_custom_modules_simulate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter projects sources findings group.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GroupFindingsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_projects_sources_findings_group(
        &self,
        args: &SecuritycenterProjectsSourcesFindingsGroupArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GroupFindingsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_projects_sources_findings_group_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_projects_sources_findings_group_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter projects sources findings patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Finding result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_projects_sources_findings_patch(
        &self,
        args: &SecuritycenterProjectsSourcesFindingsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Finding, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_projects_sources_findings_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_projects_sources_findings_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter projects sources findings set mute.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Finding result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_projects_sources_findings_set_mute(
        &self,
        args: &SecuritycenterProjectsSourcesFindingsSetMuteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Finding, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_projects_sources_findings_set_mute_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_projects_sources_findings_set_mute_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter projects sources findings set state.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Finding result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_projects_sources_findings_set_state(
        &self,
        args: &SecuritycenterProjectsSourcesFindingsSetStateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Finding, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_projects_sources_findings_set_state_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_projects_sources_findings_set_state_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter projects sources findings update security marks.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SecurityMarks result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_projects_sources_findings_update_security_marks(
        &self,
        args: &SecuritycenterProjectsSourcesFindingsUpdateSecurityMarksArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SecurityMarks, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_projects_sources_findings_update_security_marks_builder(
            &self.http_client,
            &args.name,
            &args.startTime,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_projects_sources_findings_update_security_marks_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securitycenter projects sources findings external systems patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudSecuritycenterV1ExternalSystem result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn securitycenter_projects_sources_findings_external_systems_patch(
        &self,
        args: &SecuritycenterProjectsSourcesFindingsExternalSystemsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudSecuritycenterV1ExternalSystem, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securitycenter_projects_sources_findings_external_systems_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = securitycenter_projects_sources_findings_external_systems_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
