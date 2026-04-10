//! IntegrationsProvider - State-aware integrations API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       integrations API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::integrations::{
    integrations_projects_locations_generate_open_api_spec_builder, integrations_projects_locations_generate_open_api_spec_task,
    integrations_projects_locations_apps_script_projects_create_builder, integrations_projects_locations_apps_script_projects_create_task,
    integrations_projects_locations_apps_script_projects_link_builder, integrations_projects_locations_apps_script_projects_link_task,
    integrations_projects_locations_auth_configs_create_builder, integrations_projects_locations_auth_configs_create_task,
    integrations_projects_locations_auth_configs_delete_builder, integrations_projects_locations_auth_configs_delete_task,
    integrations_projects_locations_auth_configs_patch_builder, integrations_projects_locations_auth_configs_patch_task,
    integrations_projects_locations_certificates_create_builder, integrations_projects_locations_certificates_create_task,
    integrations_projects_locations_certificates_delete_builder, integrations_projects_locations_certificates_delete_task,
    integrations_projects_locations_certificates_patch_builder, integrations_projects_locations_certificates_patch_task,
    integrations_projects_locations_clients_change_config_builder, integrations_projects_locations_clients_change_config_task,
    integrations_projects_locations_clients_deprovision_builder, integrations_projects_locations_clients_deprovision_task,
    integrations_projects_locations_clients_provision_builder, integrations_projects_locations_clients_provision_task,
    integrations_projects_locations_clients_provision_client_post_processor_builder, integrations_projects_locations_clients_provision_client_post_processor_task,
    integrations_projects_locations_clients_replace_builder, integrations_projects_locations_clients_replace_task,
    integrations_projects_locations_clients_switch_builder, integrations_projects_locations_clients_switch_task,
    integrations_projects_locations_clients_switch_variable_masking_builder, integrations_projects_locations_clients_switch_variable_masking_task,
    integrations_projects_locations_clients_toggle_http_builder, integrations_projects_locations_clients_toggle_http_task,
    integrations_projects_locations_cloud_functions_create_builder, integrations_projects_locations_cloud_functions_create_task,
    integrations_projects_locations_integrations_delete_builder, integrations_projects_locations_integrations_delete_task,
    integrations_projects_locations_integrations_execute_builder, integrations_projects_locations_integrations_execute_task,
    integrations_projects_locations_integrations_execute_event_builder, integrations_projects_locations_integrations_execute_event_task,
    integrations_projects_locations_integrations_schedule_builder, integrations_projects_locations_integrations_schedule_task,
    integrations_projects_locations_integrations_test_builder, integrations_projects_locations_integrations_test_task,
    integrations_projects_locations_integrations_executions_cancel_builder, integrations_projects_locations_integrations_executions_cancel_task,
    integrations_projects_locations_integrations_executions_replay_builder, integrations_projects_locations_integrations_executions_replay_task,
    integrations_projects_locations_integrations_executions_suspensions_lift_builder, integrations_projects_locations_integrations_executions_suspensions_lift_task,
    integrations_projects_locations_integrations_executions_suspensions_resolve_builder, integrations_projects_locations_integrations_executions_suspensions_resolve_task,
    integrations_projects_locations_integrations_versions_create_builder, integrations_projects_locations_integrations_versions_create_task,
    integrations_projects_locations_integrations_versions_delete_builder, integrations_projects_locations_integrations_versions_delete_task,
    integrations_projects_locations_integrations_versions_patch_builder, integrations_projects_locations_integrations_versions_patch_task,
    integrations_projects_locations_integrations_versions_publish_builder, integrations_projects_locations_integrations_versions_publish_task,
    integrations_projects_locations_integrations_versions_test_builder, integrations_projects_locations_integrations_versions_test_task,
    integrations_projects_locations_integrations_versions_unpublish_builder, integrations_projects_locations_integrations_versions_unpublish_task,
    integrations_projects_locations_integrations_versions_upload_builder, integrations_projects_locations_integrations_versions_upload_task,
    integrations_projects_locations_integrations_versions_test_cases_create_builder, integrations_projects_locations_integrations_versions_test_cases_create_task,
    integrations_projects_locations_integrations_versions_test_cases_delete_builder, integrations_projects_locations_integrations_versions_test_cases_delete_task,
    integrations_projects_locations_integrations_versions_test_cases_execute_builder, integrations_projects_locations_integrations_versions_test_cases_execute_task,
    integrations_projects_locations_integrations_versions_test_cases_execute_test_builder, integrations_projects_locations_integrations_versions_test_cases_execute_test_task,
    integrations_projects_locations_integrations_versions_test_cases_patch_builder, integrations_projects_locations_integrations_versions_test_cases_patch_task,
    integrations_projects_locations_integrations_versions_test_cases_takeover_edit_lock_builder, integrations_projects_locations_integrations_versions_test_cases_takeover_edit_lock_task,
    integrations_projects_locations_integrations_versions_test_cases_upload_builder, integrations_projects_locations_integrations_versions_test_cases_upload_task,
    integrations_projects_locations_products_auth_configs_create_builder, integrations_projects_locations_products_auth_configs_create_task,
    integrations_projects_locations_products_auth_configs_delete_builder, integrations_projects_locations_products_auth_configs_delete_task,
    integrations_projects_locations_products_auth_configs_patch_builder, integrations_projects_locations_products_auth_configs_patch_task,
    integrations_projects_locations_products_certificates_create_builder, integrations_projects_locations_products_certificates_create_task,
    integrations_projects_locations_products_certificates_delete_builder, integrations_projects_locations_products_certificates_delete_task,
    integrations_projects_locations_products_certificates_patch_builder, integrations_projects_locations_products_certificates_patch_task,
    integrations_projects_locations_products_cloud_functions_create_builder, integrations_projects_locations_products_cloud_functions_create_task,
    integrations_projects_locations_products_integrations_execute_builder, integrations_projects_locations_products_integrations_execute_task,
    integrations_projects_locations_products_integrations_schedule_builder, integrations_projects_locations_products_integrations_schedule_task,
    integrations_projects_locations_products_integrations_test_builder, integrations_projects_locations_products_integrations_test_task,
    integrations_projects_locations_products_integrations_executions_suspensions_lift_builder, integrations_projects_locations_products_integrations_executions_suspensions_lift_task,
    integrations_projects_locations_products_integrations_executions_suspensions_resolve_builder, integrations_projects_locations_products_integrations_executions_suspensions_resolve_task,
    integrations_projects_locations_products_integrations_versions_create_builder, integrations_projects_locations_products_integrations_versions_create_task,
    integrations_projects_locations_products_integrations_versions_delete_builder, integrations_projects_locations_products_integrations_versions_delete_task,
    integrations_projects_locations_products_integrations_versions_patch_builder, integrations_projects_locations_products_integrations_versions_patch_task,
    integrations_projects_locations_products_integrations_versions_publish_builder, integrations_projects_locations_products_integrations_versions_publish_task,
    integrations_projects_locations_products_integrations_versions_takeover_edit_lock_builder, integrations_projects_locations_products_integrations_versions_takeover_edit_lock_task,
    integrations_projects_locations_products_integrations_versions_unpublish_builder, integrations_projects_locations_products_integrations_versions_unpublish_task,
    integrations_projects_locations_products_integrations_versions_upload_builder, integrations_projects_locations_products_integrations_versions_upload_task,
    integrations_projects_locations_products_sfdc_instances_create_builder, integrations_projects_locations_products_sfdc_instances_create_task,
    integrations_projects_locations_products_sfdc_instances_delete_builder, integrations_projects_locations_products_sfdc_instances_delete_task,
    integrations_projects_locations_products_sfdc_instances_patch_builder, integrations_projects_locations_products_sfdc_instances_patch_task,
    integrations_projects_locations_products_sfdc_instances_sfdc_channels_create_builder, integrations_projects_locations_products_sfdc_instances_sfdc_channels_create_task,
    integrations_projects_locations_products_sfdc_instances_sfdc_channels_delete_builder, integrations_projects_locations_products_sfdc_instances_sfdc_channels_delete_task,
    integrations_projects_locations_products_sfdc_instances_sfdc_channels_patch_builder, integrations_projects_locations_products_sfdc_instances_sfdc_channels_patch_task,
    integrations_projects_locations_sfdc_instances_create_builder, integrations_projects_locations_sfdc_instances_create_task,
    integrations_projects_locations_sfdc_instances_delete_builder, integrations_projects_locations_sfdc_instances_delete_task,
    integrations_projects_locations_sfdc_instances_patch_builder, integrations_projects_locations_sfdc_instances_patch_task,
    integrations_projects_locations_sfdc_instances_sfdc_channels_create_builder, integrations_projects_locations_sfdc_instances_sfdc_channels_create_task,
    integrations_projects_locations_sfdc_instances_sfdc_channels_delete_builder, integrations_projects_locations_sfdc_instances_sfdc_channels_delete_task,
    integrations_projects_locations_sfdc_instances_sfdc_channels_patch_builder, integrations_projects_locations_sfdc_instances_sfdc_channels_patch_task,
    integrations_projects_locations_templates_create_builder, integrations_projects_locations_templates_create_task,
    integrations_projects_locations_templates_delete_builder, integrations_projects_locations_templates_delete_task,
    integrations_projects_locations_templates_import_builder, integrations_projects_locations_templates_import_task,
    integrations_projects_locations_templates_patch_builder, integrations_projects_locations_templates_patch_task,
    integrations_projects_locations_templates_share_builder, integrations_projects_locations_templates_share_task,
    integrations_projects_locations_templates_unshare_builder, integrations_projects_locations_templates_unshare_task,
    integrations_projects_locations_templates_upload_builder, integrations_projects_locations_templates_upload_task,
    integrations_projects_locations_templates_use_builder, integrations_projects_locations_templates_use_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::integrations::GoogleCloudIntegrationsV1alphaAuthConfig;
use crate::providers::gcp::clients::integrations::GoogleCloudIntegrationsV1alphaCancelExecutionResponse;
use crate::providers::gcp::clients::integrations::GoogleCloudIntegrationsV1alphaCertificate;
use crate::providers::gcp::clients::integrations::GoogleCloudIntegrationsV1alphaChangeCustomerConfigResponse;
use crate::providers::gcp::clients::integrations::GoogleCloudIntegrationsV1alphaCreateAppsScriptProjectResponse;
use crate::providers::gcp::clients::integrations::GoogleCloudIntegrationsV1alphaCreateCloudFunctionResponse;
use crate::providers::gcp::clients::integrations::GoogleCloudIntegrationsV1alphaExecuteEventResponse;
use crate::providers::gcp::clients::integrations::GoogleCloudIntegrationsV1alphaExecuteIntegrationsResponse;
use crate::providers::gcp::clients::integrations::GoogleCloudIntegrationsV1alphaExecuteTestCaseResponse;
use crate::providers::gcp::clients::integrations::GoogleCloudIntegrationsV1alphaExecuteTestCasesResponse;
use crate::providers::gcp::clients::integrations::GoogleCloudIntegrationsV1alphaGenerateOpenApiSpecResponse;
use crate::providers::gcp::clients::integrations::GoogleCloudIntegrationsV1alphaImportTemplateResponse;
use crate::providers::gcp::clients::integrations::GoogleCloudIntegrationsV1alphaIntegrationVersion;
use crate::providers::gcp::clients::integrations::GoogleCloudIntegrationsV1alphaLiftSuspensionResponse;
use crate::providers::gcp::clients::integrations::GoogleCloudIntegrationsV1alphaLinkAppsScriptProjectResponse;
use crate::providers::gcp::clients::integrations::GoogleCloudIntegrationsV1alphaProvisionClientPostProcessorResponse;
use crate::providers::gcp::clients::integrations::GoogleCloudIntegrationsV1alphaPublishIntegrationVersionResponse;
use crate::providers::gcp::clients::integrations::GoogleCloudIntegrationsV1alphaReplayExecutionResponse;
use crate::providers::gcp::clients::integrations::GoogleCloudIntegrationsV1alphaResolveSuspensionResponse;
use crate::providers::gcp::clients::integrations::GoogleCloudIntegrationsV1alphaScheduleIntegrationsResponse;
use crate::providers::gcp::clients::integrations::GoogleCloudIntegrationsV1alphaSfdcChannel;
use crate::providers::gcp::clients::integrations::GoogleCloudIntegrationsV1alphaSfdcInstance;
use crate::providers::gcp::clients::integrations::GoogleCloudIntegrationsV1alphaTakeoverEditLockResponse;
use crate::providers::gcp::clients::integrations::GoogleCloudIntegrationsV1alphaTemplate;
use crate::providers::gcp::clients::integrations::GoogleCloudIntegrationsV1alphaTestCase;
use crate::providers::gcp::clients::integrations::GoogleCloudIntegrationsV1alphaTestIntegrationsResponse;
use crate::providers::gcp::clients::integrations::GoogleCloudIntegrationsV1alphaUploadIntegrationVersionResponse;
use crate::providers::gcp::clients::integrations::GoogleCloudIntegrationsV1alphaUploadTemplateResponse;
use crate::providers::gcp::clients::integrations::GoogleCloudIntegrationsV1alphaUploadTestCaseResponse;
use crate::providers::gcp::clients::integrations::GoogleCloudIntegrationsV1alphaUseTemplateResponse;
use crate::providers::gcp::clients::integrations::GoogleProtobufEmpty;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsAppsScriptProjectsCreateArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsAppsScriptProjectsLinkArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsAuthConfigsCreateArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsAuthConfigsDeleteArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsAuthConfigsPatchArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsCertificatesCreateArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsCertificatesDeleteArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsCertificatesPatchArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsClientsChangeConfigArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsClientsDeprovisionArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsClientsProvisionArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsClientsProvisionClientPostProcessorArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsClientsReplaceArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsClientsSwitchArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsClientsSwitchVariableMaskingArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsClientsToggleHttpArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsCloudFunctionsCreateArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsGenerateOpenApiSpecArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsIntegrationsDeleteArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsIntegrationsExecuteArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsIntegrationsExecuteEventArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsIntegrationsExecutionsCancelArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsIntegrationsExecutionsReplayArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsIntegrationsExecutionsSuspensionsLiftArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsIntegrationsExecutionsSuspensionsResolveArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsIntegrationsScheduleArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsIntegrationsTestArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsIntegrationsVersionsCreateArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsIntegrationsVersionsDeleteArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsIntegrationsVersionsPatchArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsIntegrationsVersionsPublishArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsIntegrationsVersionsTestArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsIntegrationsVersionsTestCasesCreateArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsIntegrationsVersionsTestCasesDeleteArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsIntegrationsVersionsTestCasesExecuteArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsIntegrationsVersionsTestCasesExecuteTestArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsIntegrationsVersionsTestCasesPatchArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsIntegrationsVersionsTestCasesTakeoverEditLockArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsIntegrationsVersionsTestCasesUploadArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsIntegrationsVersionsUnpublishArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsIntegrationsVersionsUploadArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsProductsAuthConfigsCreateArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsProductsAuthConfigsDeleteArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsProductsAuthConfigsPatchArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsProductsCertificatesCreateArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsProductsCertificatesDeleteArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsProductsCertificatesPatchArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsProductsCloudFunctionsCreateArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsProductsIntegrationsExecuteArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsProductsIntegrationsExecutionsSuspensionsLiftArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsProductsIntegrationsExecutionsSuspensionsResolveArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsProductsIntegrationsScheduleArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsProductsIntegrationsTestArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsProductsIntegrationsVersionsCreateArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsProductsIntegrationsVersionsDeleteArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsProductsIntegrationsVersionsPatchArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsProductsIntegrationsVersionsPublishArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsProductsIntegrationsVersionsTakeoverEditLockArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsProductsIntegrationsVersionsUnpublishArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsProductsIntegrationsVersionsUploadArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsProductsSfdcInstancesCreateArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsProductsSfdcInstancesDeleteArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsProductsSfdcInstancesPatchArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsProductsSfdcInstancesSfdcChannelsCreateArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsProductsSfdcInstancesSfdcChannelsDeleteArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsProductsSfdcInstancesSfdcChannelsPatchArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsSfdcInstancesCreateArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsSfdcInstancesDeleteArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsSfdcInstancesPatchArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsSfdcInstancesSfdcChannelsCreateArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsSfdcInstancesSfdcChannelsDeleteArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsSfdcInstancesSfdcChannelsPatchArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsTemplatesCreateArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsTemplatesDeleteArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsTemplatesImportArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsTemplatesPatchArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsTemplatesShareArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsTemplatesUnshareArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsTemplatesUploadArgs;
use crate::providers::gcp::clients::integrations::IntegrationsProjectsLocationsTemplatesUseArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// IntegrationsProvider with automatic state tracking.
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
/// let provider = IntegrationsProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct IntegrationsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> IntegrationsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new IntegrationsProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Integrations projects locations generate open api spec.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaGenerateOpenApiSpecResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_generate_open_api_spec(
        &self,
        args: &IntegrationsProjectsLocationsGenerateOpenApiSpecArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaGenerateOpenApiSpecResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_generate_open_api_spec_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_generate_open_api_spec_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations apps script projects create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaCreateAppsScriptProjectResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_apps_script_projects_create(
        &self,
        args: &IntegrationsProjectsLocationsAppsScriptProjectsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaCreateAppsScriptProjectResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_apps_script_projects_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_apps_script_projects_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations apps script projects link.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaLinkAppsScriptProjectResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_apps_script_projects_link(
        &self,
        args: &IntegrationsProjectsLocationsAppsScriptProjectsLinkArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaLinkAppsScriptProjectResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_apps_script_projects_link_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_apps_script_projects_link_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations auth configs create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaAuthConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_auth_configs_create(
        &self,
        args: &IntegrationsProjectsLocationsAuthConfigsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaAuthConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_auth_configs_create_builder(
            &self.http_client,
            &args.parent,
            &args.clientCertificate.encryptedPrivateKey,
            &args.clientCertificate.passphrase,
            &args.clientCertificate.sslCertificate,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_auth_configs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations auth configs delete.
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
    pub fn integrations_projects_locations_auth_configs_delete(
        &self,
        args: &IntegrationsProjectsLocationsAuthConfigsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_auth_configs_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_auth_configs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations auth configs patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaAuthConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_auth_configs_patch(
        &self,
        args: &IntegrationsProjectsLocationsAuthConfigsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaAuthConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_auth_configs_patch_builder(
            &self.http_client,
            &args.name,
            &args.clientCertificate.encryptedPrivateKey,
            &args.clientCertificate.passphrase,
            &args.clientCertificate.sslCertificate,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_auth_configs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations certificates create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaCertificate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_certificates_create(
        &self,
        args: &IntegrationsProjectsLocationsCertificatesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaCertificate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_certificates_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_certificates_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations certificates delete.
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
    pub fn integrations_projects_locations_certificates_delete(
        &self,
        args: &IntegrationsProjectsLocationsCertificatesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_certificates_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_certificates_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations certificates patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaCertificate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_certificates_patch(
        &self,
        args: &IntegrationsProjectsLocationsCertificatesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaCertificate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_certificates_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_certificates_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations clients change config.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaChangeCustomerConfigResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_clients_change_config(
        &self,
        args: &IntegrationsProjectsLocationsClientsChangeConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaChangeCustomerConfigResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_clients_change_config_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_clients_change_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations clients deprovision.
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
    pub fn integrations_projects_locations_clients_deprovision(
        &self,
        args: &IntegrationsProjectsLocationsClientsDeprovisionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_clients_deprovision_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_clients_deprovision_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations clients provision.
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
    pub fn integrations_projects_locations_clients_provision(
        &self,
        args: &IntegrationsProjectsLocationsClientsProvisionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_clients_provision_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_clients_provision_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations clients provision client post processor.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaProvisionClientPostProcessorResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_clients_provision_client_post_processor(
        &self,
        args: &IntegrationsProjectsLocationsClientsProvisionClientPostProcessorArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaProvisionClientPostProcessorResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_clients_provision_client_post_processor_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_clients_provision_client_post_processor_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations clients replace.
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
    pub fn integrations_projects_locations_clients_replace(
        &self,
        args: &IntegrationsProjectsLocationsClientsReplaceArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_clients_replace_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_clients_replace_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations clients switch.
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
    pub fn integrations_projects_locations_clients_switch(
        &self,
        args: &IntegrationsProjectsLocationsClientsSwitchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_clients_switch_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_clients_switch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations clients switch variable masking.
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
    pub fn integrations_projects_locations_clients_switch_variable_masking(
        &self,
        args: &IntegrationsProjectsLocationsClientsSwitchVariableMaskingArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_clients_switch_variable_masking_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_clients_switch_variable_masking_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations clients toggle http.
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
    pub fn integrations_projects_locations_clients_toggle_http(
        &self,
        args: &IntegrationsProjectsLocationsClientsToggleHttpArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_clients_toggle_http_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_clients_toggle_http_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations cloud functions create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaCreateCloudFunctionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_cloud_functions_create(
        &self,
        args: &IntegrationsProjectsLocationsCloudFunctionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaCreateCloudFunctionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_cloud_functions_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_cloud_functions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations integrations delete.
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
    pub fn integrations_projects_locations_integrations_delete(
        &self,
        args: &IntegrationsProjectsLocationsIntegrationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_integrations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_integrations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations integrations execute.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaExecuteIntegrationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_integrations_execute(
        &self,
        args: &IntegrationsProjectsLocationsIntegrationsExecuteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaExecuteIntegrationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_integrations_execute_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_integrations_execute_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations integrations execute event.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaExecuteEventResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_integrations_execute_event(
        &self,
        args: &IntegrationsProjectsLocationsIntegrationsExecuteEventArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaExecuteEventResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_integrations_execute_event_builder(
            &self.http_client,
            &args.name,
            &args.triggerId,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_integrations_execute_event_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations integrations schedule.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaScheduleIntegrationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_integrations_schedule(
        &self,
        args: &IntegrationsProjectsLocationsIntegrationsScheduleArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaScheduleIntegrationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_integrations_schedule_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_integrations_schedule_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations integrations test.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaTestIntegrationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_integrations_test(
        &self,
        args: &IntegrationsProjectsLocationsIntegrationsTestArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaTestIntegrationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_integrations_test_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_integrations_test_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations integrations executions cancel.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaCancelExecutionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_integrations_executions_cancel(
        &self,
        args: &IntegrationsProjectsLocationsIntegrationsExecutionsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaCancelExecutionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_integrations_executions_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_integrations_executions_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations integrations executions replay.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaReplayExecutionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_integrations_executions_replay(
        &self,
        args: &IntegrationsProjectsLocationsIntegrationsExecutionsReplayArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaReplayExecutionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_integrations_executions_replay_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_integrations_executions_replay_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations integrations executions suspensions lift.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaLiftSuspensionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_integrations_executions_suspensions_lift(
        &self,
        args: &IntegrationsProjectsLocationsIntegrationsExecutionsSuspensionsLiftArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaLiftSuspensionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_integrations_executions_suspensions_lift_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_integrations_executions_suspensions_lift_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations integrations executions suspensions resolve.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaResolveSuspensionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_integrations_executions_suspensions_resolve(
        &self,
        args: &IntegrationsProjectsLocationsIntegrationsExecutionsSuspensionsResolveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaResolveSuspensionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_integrations_executions_suspensions_resolve_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_integrations_executions_suspensions_resolve_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations integrations versions create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaIntegrationVersion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_integrations_versions_create(
        &self,
        args: &IntegrationsProjectsLocationsIntegrationsVersionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaIntegrationVersion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_integrations_versions_create_builder(
            &self.http_client,
            &args.parent,
            &args.createSampleIntegrations,
            &args.newIntegration,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_integrations_versions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations integrations versions delete.
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
    pub fn integrations_projects_locations_integrations_versions_delete(
        &self,
        args: &IntegrationsProjectsLocationsIntegrationsVersionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_integrations_versions_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_integrations_versions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations integrations versions patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaIntegrationVersion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_integrations_versions_patch(
        &self,
        args: &IntegrationsProjectsLocationsIntegrationsVersionsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaIntegrationVersion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_integrations_versions_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_integrations_versions_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations integrations versions publish.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaPublishIntegrationVersionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_integrations_versions_publish(
        &self,
        args: &IntegrationsProjectsLocationsIntegrationsVersionsPublishArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaPublishIntegrationVersionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_integrations_versions_publish_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_integrations_versions_publish_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations integrations versions test.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaTestIntegrationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_integrations_versions_test(
        &self,
        args: &IntegrationsProjectsLocationsIntegrationsVersionsTestArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaTestIntegrationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_integrations_versions_test_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_integrations_versions_test_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations integrations versions unpublish.
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
    pub fn integrations_projects_locations_integrations_versions_unpublish(
        &self,
        args: &IntegrationsProjectsLocationsIntegrationsVersionsUnpublishArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_integrations_versions_unpublish_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_integrations_versions_unpublish_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations integrations versions upload.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaUploadIntegrationVersionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_integrations_versions_upload(
        &self,
        args: &IntegrationsProjectsLocationsIntegrationsVersionsUploadArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaUploadIntegrationVersionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_integrations_versions_upload_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_integrations_versions_upload_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations integrations versions test cases create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaTestCase result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_integrations_versions_test_cases_create(
        &self,
        args: &IntegrationsProjectsLocationsIntegrationsVersionsTestCasesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaTestCase, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_integrations_versions_test_cases_create_builder(
            &self.http_client,
            &args.parent,
            &args.testCaseId,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_integrations_versions_test_cases_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations integrations versions test cases delete.
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
    pub fn integrations_projects_locations_integrations_versions_test_cases_delete(
        &self,
        args: &IntegrationsProjectsLocationsIntegrationsVersionsTestCasesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_integrations_versions_test_cases_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_integrations_versions_test_cases_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations integrations versions test cases execute.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaExecuteTestCasesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_integrations_versions_test_cases_execute(
        &self,
        args: &IntegrationsProjectsLocationsIntegrationsVersionsTestCasesExecuteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaExecuteTestCasesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_integrations_versions_test_cases_execute_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_integrations_versions_test_cases_execute_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations integrations versions test cases execute test.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaExecuteTestCaseResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_integrations_versions_test_cases_execute_test(
        &self,
        args: &IntegrationsProjectsLocationsIntegrationsVersionsTestCasesExecuteTestArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaExecuteTestCaseResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_integrations_versions_test_cases_execute_test_builder(
            &self.http_client,
            &args.testCaseName,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_integrations_versions_test_cases_execute_test_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations integrations versions test cases patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaTestCase result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_integrations_versions_test_cases_patch(
        &self,
        args: &IntegrationsProjectsLocationsIntegrationsVersionsTestCasesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaTestCase, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_integrations_versions_test_cases_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_integrations_versions_test_cases_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations integrations versions test cases takeover edit lock.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaTestCase result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_integrations_versions_test_cases_takeover_edit_lock(
        &self,
        args: &IntegrationsProjectsLocationsIntegrationsVersionsTestCasesTakeoverEditLockArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaTestCase, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_integrations_versions_test_cases_takeover_edit_lock_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_integrations_versions_test_cases_takeover_edit_lock_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations integrations versions test cases upload.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaUploadTestCaseResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_integrations_versions_test_cases_upload(
        &self,
        args: &IntegrationsProjectsLocationsIntegrationsVersionsTestCasesUploadArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaUploadTestCaseResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_integrations_versions_test_cases_upload_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_integrations_versions_test_cases_upload_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations products auth configs create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaAuthConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_products_auth_configs_create(
        &self,
        args: &IntegrationsProjectsLocationsProductsAuthConfigsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaAuthConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_products_auth_configs_create_builder(
            &self.http_client,
            &args.parent,
            &args.clientCertificate.encryptedPrivateKey,
            &args.clientCertificate.passphrase,
            &args.clientCertificate.sslCertificate,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_products_auth_configs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations products auth configs delete.
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
    pub fn integrations_projects_locations_products_auth_configs_delete(
        &self,
        args: &IntegrationsProjectsLocationsProductsAuthConfigsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_products_auth_configs_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_products_auth_configs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations products auth configs patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaAuthConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_products_auth_configs_patch(
        &self,
        args: &IntegrationsProjectsLocationsProductsAuthConfigsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaAuthConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_products_auth_configs_patch_builder(
            &self.http_client,
            &args.name,
            &args.clientCertificate.encryptedPrivateKey,
            &args.clientCertificate.passphrase,
            &args.clientCertificate.sslCertificate,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_products_auth_configs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations products certificates create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaCertificate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_products_certificates_create(
        &self,
        args: &IntegrationsProjectsLocationsProductsCertificatesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaCertificate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_products_certificates_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_products_certificates_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations products certificates delete.
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
    pub fn integrations_projects_locations_products_certificates_delete(
        &self,
        args: &IntegrationsProjectsLocationsProductsCertificatesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_products_certificates_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_products_certificates_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations products certificates patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaCertificate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_products_certificates_patch(
        &self,
        args: &IntegrationsProjectsLocationsProductsCertificatesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaCertificate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_products_certificates_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_products_certificates_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations products cloud functions create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaCreateCloudFunctionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_products_cloud_functions_create(
        &self,
        args: &IntegrationsProjectsLocationsProductsCloudFunctionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaCreateCloudFunctionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_products_cloud_functions_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_products_cloud_functions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations products integrations execute.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaExecuteIntegrationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_products_integrations_execute(
        &self,
        args: &IntegrationsProjectsLocationsProductsIntegrationsExecuteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaExecuteIntegrationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_products_integrations_execute_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_products_integrations_execute_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations products integrations schedule.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaScheduleIntegrationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_products_integrations_schedule(
        &self,
        args: &IntegrationsProjectsLocationsProductsIntegrationsScheduleArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaScheduleIntegrationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_products_integrations_schedule_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_products_integrations_schedule_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations products integrations test.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaTestIntegrationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_products_integrations_test(
        &self,
        args: &IntegrationsProjectsLocationsProductsIntegrationsTestArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaTestIntegrationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_products_integrations_test_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_products_integrations_test_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations products integrations executions suspensions lift.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaLiftSuspensionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_products_integrations_executions_suspensions_lift(
        &self,
        args: &IntegrationsProjectsLocationsProductsIntegrationsExecutionsSuspensionsLiftArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaLiftSuspensionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_products_integrations_executions_suspensions_lift_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_products_integrations_executions_suspensions_lift_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations products integrations executions suspensions resolve.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaResolveSuspensionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_products_integrations_executions_suspensions_resolve(
        &self,
        args: &IntegrationsProjectsLocationsProductsIntegrationsExecutionsSuspensionsResolveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaResolveSuspensionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_products_integrations_executions_suspensions_resolve_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_products_integrations_executions_suspensions_resolve_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations products integrations versions create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaIntegrationVersion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_products_integrations_versions_create(
        &self,
        args: &IntegrationsProjectsLocationsProductsIntegrationsVersionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaIntegrationVersion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_products_integrations_versions_create_builder(
            &self.http_client,
            &args.parent,
            &args.createSampleIntegrations,
            &args.newIntegration,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_products_integrations_versions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations products integrations versions delete.
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
    pub fn integrations_projects_locations_products_integrations_versions_delete(
        &self,
        args: &IntegrationsProjectsLocationsProductsIntegrationsVersionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_products_integrations_versions_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_products_integrations_versions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations products integrations versions patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaIntegrationVersion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_products_integrations_versions_patch(
        &self,
        args: &IntegrationsProjectsLocationsProductsIntegrationsVersionsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaIntegrationVersion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_products_integrations_versions_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_products_integrations_versions_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations products integrations versions publish.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaPublishIntegrationVersionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_products_integrations_versions_publish(
        &self,
        args: &IntegrationsProjectsLocationsProductsIntegrationsVersionsPublishArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaPublishIntegrationVersionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_products_integrations_versions_publish_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_products_integrations_versions_publish_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations products integrations versions takeover edit lock.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaTakeoverEditLockResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_products_integrations_versions_takeover_edit_lock(
        &self,
        args: &IntegrationsProjectsLocationsProductsIntegrationsVersionsTakeoverEditLockArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaTakeoverEditLockResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_products_integrations_versions_takeover_edit_lock_builder(
            &self.http_client,
            &args.integrationVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_products_integrations_versions_takeover_edit_lock_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations products integrations versions unpublish.
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
    pub fn integrations_projects_locations_products_integrations_versions_unpublish(
        &self,
        args: &IntegrationsProjectsLocationsProductsIntegrationsVersionsUnpublishArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_products_integrations_versions_unpublish_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_products_integrations_versions_unpublish_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations products integrations versions upload.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaUploadIntegrationVersionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_products_integrations_versions_upload(
        &self,
        args: &IntegrationsProjectsLocationsProductsIntegrationsVersionsUploadArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaUploadIntegrationVersionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_products_integrations_versions_upload_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_products_integrations_versions_upload_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations products sfdc instances create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaSfdcInstance result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_products_sfdc_instances_create(
        &self,
        args: &IntegrationsProjectsLocationsProductsSfdcInstancesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaSfdcInstance, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_products_sfdc_instances_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_products_sfdc_instances_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations products sfdc instances delete.
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
    pub fn integrations_projects_locations_products_sfdc_instances_delete(
        &self,
        args: &IntegrationsProjectsLocationsProductsSfdcInstancesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_products_sfdc_instances_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_products_sfdc_instances_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations products sfdc instances patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaSfdcInstance result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_products_sfdc_instances_patch(
        &self,
        args: &IntegrationsProjectsLocationsProductsSfdcInstancesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaSfdcInstance, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_products_sfdc_instances_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_products_sfdc_instances_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations products sfdc instances sfdc channels create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaSfdcChannel result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_products_sfdc_instances_sfdc_channels_create(
        &self,
        args: &IntegrationsProjectsLocationsProductsSfdcInstancesSfdcChannelsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaSfdcChannel, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_products_sfdc_instances_sfdc_channels_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_products_sfdc_instances_sfdc_channels_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations products sfdc instances sfdc channels delete.
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
    pub fn integrations_projects_locations_products_sfdc_instances_sfdc_channels_delete(
        &self,
        args: &IntegrationsProjectsLocationsProductsSfdcInstancesSfdcChannelsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_products_sfdc_instances_sfdc_channels_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_products_sfdc_instances_sfdc_channels_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations products sfdc instances sfdc channels patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaSfdcChannel result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_products_sfdc_instances_sfdc_channels_patch(
        &self,
        args: &IntegrationsProjectsLocationsProductsSfdcInstancesSfdcChannelsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaSfdcChannel, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_products_sfdc_instances_sfdc_channels_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_products_sfdc_instances_sfdc_channels_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations sfdc instances create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaSfdcInstance result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_sfdc_instances_create(
        &self,
        args: &IntegrationsProjectsLocationsSfdcInstancesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaSfdcInstance, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_sfdc_instances_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_sfdc_instances_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations sfdc instances delete.
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
    pub fn integrations_projects_locations_sfdc_instances_delete(
        &self,
        args: &IntegrationsProjectsLocationsSfdcInstancesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_sfdc_instances_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_sfdc_instances_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations sfdc instances patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaSfdcInstance result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_sfdc_instances_patch(
        &self,
        args: &IntegrationsProjectsLocationsSfdcInstancesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaSfdcInstance, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_sfdc_instances_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_sfdc_instances_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations sfdc instances sfdc channels create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaSfdcChannel result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_sfdc_instances_sfdc_channels_create(
        &self,
        args: &IntegrationsProjectsLocationsSfdcInstancesSfdcChannelsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaSfdcChannel, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_sfdc_instances_sfdc_channels_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_sfdc_instances_sfdc_channels_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations sfdc instances sfdc channels delete.
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
    pub fn integrations_projects_locations_sfdc_instances_sfdc_channels_delete(
        &self,
        args: &IntegrationsProjectsLocationsSfdcInstancesSfdcChannelsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_sfdc_instances_sfdc_channels_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_sfdc_instances_sfdc_channels_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations sfdc instances sfdc channels patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaSfdcChannel result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_sfdc_instances_sfdc_channels_patch(
        &self,
        args: &IntegrationsProjectsLocationsSfdcInstancesSfdcChannelsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaSfdcChannel, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_sfdc_instances_sfdc_channels_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_sfdc_instances_sfdc_channels_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations templates create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_templates_create(
        &self,
        args: &IntegrationsProjectsLocationsTemplatesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_templates_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_templates_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations templates delete.
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
    pub fn integrations_projects_locations_templates_delete(
        &self,
        args: &IntegrationsProjectsLocationsTemplatesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_templates_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_templates_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations templates import.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaImportTemplateResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_templates_import(
        &self,
        args: &IntegrationsProjectsLocationsTemplatesImportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaImportTemplateResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_templates_import_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_templates_import_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations templates patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_templates_patch(
        &self,
        args: &IntegrationsProjectsLocationsTemplatesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_templates_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_templates_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations templates share.
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
    pub fn integrations_projects_locations_templates_share(
        &self,
        args: &IntegrationsProjectsLocationsTemplatesShareArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_templates_share_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_templates_share_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations templates unshare.
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
    pub fn integrations_projects_locations_templates_unshare(
        &self,
        args: &IntegrationsProjectsLocationsTemplatesUnshareArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_templates_unshare_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_templates_unshare_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations templates upload.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaUploadTemplateResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_templates_upload(
        &self,
        args: &IntegrationsProjectsLocationsTemplatesUploadArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaUploadTemplateResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_templates_upload_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_templates_upload_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Integrations projects locations templates use.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudIntegrationsV1alphaUseTemplateResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn integrations_projects_locations_templates_use(
        &self,
        args: &IntegrationsProjectsLocationsTemplatesUseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudIntegrationsV1alphaUseTemplateResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = integrations_projects_locations_templates_use_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = integrations_projects_locations_templates_use_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
