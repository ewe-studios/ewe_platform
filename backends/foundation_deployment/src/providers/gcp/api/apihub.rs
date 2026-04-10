//! ApihubProvider - State-aware apihub API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       apihub API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::apihub::{
    apihub_projects_locations_collect_api_data_builder, apihub_projects_locations_collect_api_data_task,
    apihub_projects_locations_get_builder, apihub_projects_locations_get_task,
    apihub_projects_locations_list_builder, apihub_projects_locations_list_task,
    apihub_projects_locations_lookup_runtime_project_attachment_builder, apihub_projects_locations_lookup_runtime_project_attachment_task,
    apihub_projects_locations_retrieve_api_views_builder, apihub_projects_locations_retrieve_api_views_task,
    apihub_projects_locations_search_resources_builder, apihub_projects_locations_search_resources_task,
    apihub_projects_locations_addons_get_builder, apihub_projects_locations_addons_get_task,
    apihub_projects_locations_addons_list_builder, apihub_projects_locations_addons_list_task,
    apihub_projects_locations_addons_manage_config_builder, apihub_projects_locations_addons_manage_config_task,
    apihub_projects_locations_api_hub_instances_create_builder, apihub_projects_locations_api_hub_instances_create_task,
    apihub_projects_locations_api_hub_instances_delete_builder, apihub_projects_locations_api_hub_instances_delete_task,
    apihub_projects_locations_api_hub_instances_get_builder, apihub_projects_locations_api_hub_instances_get_task,
    apihub_projects_locations_api_hub_instances_lookup_builder, apihub_projects_locations_api_hub_instances_lookup_task,
    apihub_projects_locations_api_hub_instances_patch_builder, apihub_projects_locations_api_hub_instances_patch_task,
    apihub_projects_locations_apis_create_builder, apihub_projects_locations_apis_create_task,
    apihub_projects_locations_apis_delete_builder, apihub_projects_locations_apis_delete_task,
    apihub_projects_locations_apis_get_builder, apihub_projects_locations_apis_get_task,
    apihub_projects_locations_apis_list_builder, apihub_projects_locations_apis_list_task,
    apihub_projects_locations_apis_patch_builder, apihub_projects_locations_apis_patch_task,
    apihub_projects_locations_apis_versions_create_builder, apihub_projects_locations_apis_versions_create_task,
    apihub_projects_locations_apis_versions_delete_builder, apihub_projects_locations_apis_versions_delete_task,
    apihub_projects_locations_apis_versions_get_builder, apihub_projects_locations_apis_versions_get_task,
    apihub_projects_locations_apis_versions_list_builder, apihub_projects_locations_apis_versions_list_task,
    apihub_projects_locations_apis_versions_patch_builder, apihub_projects_locations_apis_versions_patch_task,
    apihub_projects_locations_apis_versions_definitions_get_builder, apihub_projects_locations_apis_versions_definitions_get_task,
    apihub_projects_locations_apis_versions_operations_create_builder, apihub_projects_locations_apis_versions_operations_create_task,
    apihub_projects_locations_apis_versions_operations_delete_builder, apihub_projects_locations_apis_versions_operations_delete_task,
    apihub_projects_locations_apis_versions_operations_get_builder, apihub_projects_locations_apis_versions_operations_get_task,
    apihub_projects_locations_apis_versions_operations_list_builder, apihub_projects_locations_apis_versions_operations_list_task,
    apihub_projects_locations_apis_versions_operations_patch_builder, apihub_projects_locations_apis_versions_operations_patch_task,
    apihub_projects_locations_apis_versions_specs_create_builder, apihub_projects_locations_apis_versions_specs_create_task,
    apihub_projects_locations_apis_versions_specs_delete_builder, apihub_projects_locations_apis_versions_specs_delete_task,
    apihub_projects_locations_apis_versions_specs_fetch_additional_spec_content_builder, apihub_projects_locations_apis_versions_specs_fetch_additional_spec_content_task,
    apihub_projects_locations_apis_versions_specs_get_builder, apihub_projects_locations_apis_versions_specs_get_task,
    apihub_projects_locations_apis_versions_specs_get_contents_builder, apihub_projects_locations_apis_versions_specs_get_contents_task,
    apihub_projects_locations_apis_versions_specs_lint_builder, apihub_projects_locations_apis_versions_specs_lint_task,
    apihub_projects_locations_apis_versions_specs_list_builder, apihub_projects_locations_apis_versions_specs_list_task,
    apihub_projects_locations_apis_versions_specs_patch_builder, apihub_projects_locations_apis_versions_specs_patch_task,
    apihub_projects_locations_attributes_create_builder, apihub_projects_locations_attributes_create_task,
    apihub_projects_locations_attributes_delete_builder, apihub_projects_locations_attributes_delete_task,
    apihub_projects_locations_attributes_get_builder, apihub_projects_locations_attributes_get_task,
    apihub_projects_locations_attributes_list_builder, apihub_projects_locations_attributes_list_task,
    apihub_projects_locations_attributes_patch_builder, apihub_projects_locations_attributes_patch_task,
    apihub_projects_locations_curations_create_builder, apihub_projects_locations_curations_create_task,
    apihub_projects_locations_curations_delete_builder, apihub_projects_locations_curations_delete_task,
    apihub_projects_locations_curations_get_builder, apihub_projects_locations_curations_get_task,
    apihub_projects_locations_curations_list_builder, apihub_projects_locations_curations_list_task,
    apihub_projects_locations_curations_patch_builder, apihub_projects_locations_curations_patch_task,
    apihub_projects_locations_dependencies_create_builder, apihub_projects_locations_dependencies_create_task,
    apihub_projects_locations_dependencies_delete_builder, apihub_projects_locations_dependencies_delete_task,
    apihub_projects_locations_dependencies_get_builder, apihub_projects_locations_dependencies_get_task,
    apihub_projects_locations_dependencies_list_builder, apihub_projects_locations_dependencies_list_task,
    apihub_projects_locations_dependencies_patch_builder, apihub_projects_locations_dependencies_patch_task,
    apihub_projects_locations_deployments_create_builder, apihub_projects_locations_deployments_create_task,
    apihub_projects_locations_deployments_delete_builder, apihub_projects_locations_deployments_delete_task,
    apihub_projects_locations_deployments_get_builder, apihub_projects_locations_deployments_get_task,
    apihub_projects_locations_deployments_list_builder, apihub_projects_locations_deployments_list_task,
    apihub_projects_locations_deployments_patch_builder, apihub_projects_locations_deployments_patch_task,
    apihub_projects_locations_discovered_api_observations_get_builder, apihub_projects_locations_discovered_api_observations_get_task,
    apihub_projects_locations_discovered_api_observations_list_builder, apihub_projects_locations_discovered_api_observations_list_task,
    apihub_projects_locations_discovered_api_observations_discovered_api_operations_get_builder, apihub_projects_locations_discovered_api_observations_discovered_api_operations_get_task,
    apihub_projects_locations_discovered_api_observations_discovered_api_operations_list_builder, apihub_projects_locations_discovered_api_observations_discovered_api_operations_list_task,
    apihub_projects_locations_external_apis_create_builder, apihub_projects_locations_external_apis_create_task,
    apihub_projects_locations_external_apis_delete_builder, apihub_projects_locations_external_apis_delete_task,
    apihub_projects_locations_external_apis_get_builder, apihub_projects_locations_external_apis_get_task,
    apihub_projects_locations_external_apis_list_builder, apihub_projects_locations_external_apis_list_task,
    apihub_projects_locations_external_apis_patch_builder, apihub_projects_locations_external_apis_patch_task,
    apihub_projects_locations_host_project_registrations_create_builder, apihub_projects_locations_host_project_registrations_create_task,
    apihub_projects_locations_host_project_registrations_get_builder, apihub_projects_locations_host_project_registrations_get_task,
    apihub_projects_locations_host_project_registrations_list_builder, apihub_projects_locations_host_project_registrations_list_task,
    apihub_projects_locations_operations_cancel_builder, apihub_projects_locations_operations_cancel_task,
    apihub_projects_locations_operations_delete_builder, apihub_projects_locations_operations_delete_task,
    apihub_projects_locations_operations_get_builder, apihub_projects_locations_operations_get_task,
    apihub_projects_locations_operations_list_builder, apihub_projects_locations_operations_list_task,
    apihub_projects_locations_plugins_create_builder, apihub_projects_locations_plugins_create_task,
    apihub_projects_locations_plugins_delete_builder, apihub_projects_locations_plugins_delete_task,
    apihub_projects_locations_plugins_disable_builder, apihub_projects_locations_plugins_disable_task,
    apihub_projects_locations_plugins_enable_builder, apihub_projects_locations_plugins_enable_task,
    apihub_projects_locations_plugins_get_builder, apihub_projects_locations_plugins_get_task,
    apihub_projects_locations_plugins_get_style_guide_builder, apihub_projects_locations_plugins_get_style_guide_task,
    apihub_projects_locations_plugins_list_builder, apihub_projects_locations_plugins_list_task,
    apihub_projects_locations_plugins_update_style_guide_builder, apihub_projects_locations_plugins_update_style_guide_task,
    apihub_projects_locations_plugins_instances_create_builder, apihub_projects_locations_plugins_instances_create_task,
    apihub_projects_locations_plugins_instances_delete_builder, apihub_projects_locations_plugins_instances_delete_task,
    apihub_projects_locations_plugins_instances_disable_action_builder, apihub_projects_locations_plugins_instances_disable_action_task,
    apihub_projects_locations_plugins_instances_enable_action_builder, apihub_projects_locations_plugins_instances_enable_action_task,
    apihub_projects_locations_plugins_instances_execute_action_builder, apihub_projects_locations_plugins_instances_execute_action_task,
    apihub_projects_locations_plugins_instances_get_builder, apihub_projects_locations_plugins_instances_get_task,
    apihub_projects_locations_plugins_instances_list_builder, apihub_projects_locations_plugins_instances_list_task,
    apihub_projects_locations_plugins_instances_manage_source_data_builder, apihub_projects_locations_plugins_instances_manage_source_data_task,
    apihub_projects_locations_plugins_instances_patch_builder, apihub_projects_locations_plugins_instances_patch_task,
    apihub_projects_locations_plugins_style_guide_get_contents_builder, apihub_projects_locations_plugins_style_guide_get_contents_task,
    apihub_projects_locations_runtime_project_attachments_create_builder, apihub_projects_locations_runtime_project_attachments_create_task,
    apihub_projects_locations_runtime_project_attachments_delete_builder, apihub_projects_locations_runtime_project_attachments_delete_task,
    apihub_projects_locations_runtime_project_attachments_get_builder, apihub_projects_locations_runtime_project_attachments_get_task,
    apihub_projects_locations_runtime_project_attachments_list_builder, apihub_projects_locations_runtime_project_attachments_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::apihub::Empty;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1Addon;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1Api;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1ApiHubInstance;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1ApiOperation;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1Attribute;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1Curation;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1Definition;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1Dependency;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1Deployment;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1DiscoveredApiObservation;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1DiscoveredApiOperation;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1ExternalApi;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1FetchAdditionalSpecContentResponse;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1HostProjectRegistration;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1ListAddonsResponse;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1ListApiOperationsResponse;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1ListApisResponse;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1ListAttributesResponse;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1ListCurationsResponse;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1ListDependenciesResponse;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1ListDeploymentsResponse;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1ListDiscoveredApiObservationsResponse;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1ListDiscoveredApiOperationsResponse;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1ListExternalApisResponse;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1ListHostProjectRegistrationsResponse;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1ListPluginInstancesResponse;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1ListPluginsResponse;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1ListRuntimeProjectAttachmentsResponse;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1ListSpecsResponse;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1ListVersionsResponse;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1LookupApiHubInstanceResponse;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1LookupRuntimeProjectAttachmentResponse;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1ManagePluginInstanceSourceDataResponse;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1Plugin;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1PluginInstance;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1RetrieveApiViewsResponse;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1RuntimeProjectAttachment;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1SearchResourcesResponse;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1Spec;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1SpecContents;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1StyleGuide;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1StyleGuideContents;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1Version;
use crate::providers::gcp::clients::apihub::GoogleCloudLocationListLocationsResponse;
use crate::providers::gcp::clients::apihub::GoogleCloudLocationLocation;
use crate::providers::gcp::clients::apihub::GoogleLongrunningListOperationsResponse;
use crate::providers::gcp::clients::apihub::GoogleLongrunningOperation;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsAddonsGetArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsAddonsListArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsAddonsManageConfigArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsApiHubInstancesCreateArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsApiHubInstancesDeleteArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsApiHubInstancesGetArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsApiHubInstancesLookupArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsApiHubInstancesPatchArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsApisCreateArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsApisDeleteArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsApisGetArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsApisListArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsApisPatchArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsApisVersionsCreateArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsApisVersionsDefinitionsGetArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsApisVersionsDeleteArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsApisVersionsGetArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsApisVersionsListArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsApisVersionsOperationsCreateArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsApisVersionsOperationsDeleteArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsApisVersionsOperationsGetArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsApisVersionsOperationsListArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsApisVersionsOperationsPatchArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsApisVersionsPatchArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsApisVersionsSpecsCreateArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsApisVersionsSpecsDeleteArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsApisVersionsSpecsFetchAdditionalSpecContentArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsApisVersionsSpecsGetArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsApisVersionsSpecsGetContentsArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsApisVersionsSpecsLintArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsApisVersionsSpecsListArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsApisVersionsSpecsPatchArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsAttributesCreateArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsAttributesDeleteArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsAttributesGetArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsAttributesListArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsAttributesPatchArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsCollectApiDataArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsCurationsCreateArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsCurationsDeleteArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsCurationsGetArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsCurationsListArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsCurationsPatchArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsDependenciesCreateArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsDependenciesDeleteArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsDependenciesGetArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsDependenciesListArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsDependenciesPatchArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsDeploymentsCreateArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsDeploymentsDeleteArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsDeploymentsGetArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsDeploymentsListArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsDeploymentsPatchArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsDiscoveredApiObservationsDiscoveredApiOperationsGetArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsDiscoveredApiObservationsDiscoveredApiOperationsListArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsDiscoveredApiObservationsGetArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsDiscoveredApiObservationsListArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsExternalApisCreateArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsExternalApisDeleteArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsExternalApisGetArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsExternalApisListArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsExternalApisPatchArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsGetArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsHostProjectRegistrationsCreateArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsHostProjectRegistrationsGetArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsHostProjectRegistrationsListArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsListArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsLookupRuntimeProjectAttachmentArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsOperationsListArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsPluginsCreateArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsPluginsDeleteArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsPluginsDisableArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsPluginsEnableArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsPluginsGetArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsPluginsGetStyleGuideArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsPluginsInstancesCreateArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsPluginsInstancesDeleteArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsPluginsInstancesDisableActionArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsPluginsInstancesEnableActionArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsPluginsInstancesExecuteActionArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsPluginsInstancesGetArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsPluginsInstancesListArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsPluginsInstancesManageSourceDataArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsPluginsInstancesPatchArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsPluginsListArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsPluginsStyleGuideGetContentsArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsPluginsUpdateStyleGuideArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsRetrieveApiViewsArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsRuntimeProjectAttachmentsCreateArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsRuntimeProjectAttachmentsDeleteArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsRuntimeProjectAttachmentsGetArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsRuntimeProjectAttachmentsListArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsSearchResourcesArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// ApihubProvider with automatic state tracking.
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
/// let provider = ApihubProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct ApihubProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> ApihubProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new ApihubProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Apihub projects locations collect api data.
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
    pub fn apihub_projects_locations_collect_api_data(
        &self,
        args: &ApihubProjectsLocationsCollectApiDataArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_collect_api_data_builder(
            &self.http_client,
            &args.location,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_collect_api_data_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudLocationLocation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apihub_projects_locations_get(
        &self,
        args: &ApihubProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudLocationLocation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudLocationListLocationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apihub_projects_locations_list(
        &self,
        args: &ApihubProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudLocationListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations lookup runtime project attachment.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1LookupRuntimeProjectAttachmentResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apihub_projects_locations_lookup_runtime_project_attachment(
        &self,
        args: &ApihubProjectsLocationsLookupRuntimeProjectAttachmentArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1LookupRuntimeProjectAttachmentResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_lookup_runtime_project_attachment_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_lookup_runtime_project_attachment_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations retrieve api views.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1RetrieveApiViewsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apihub_projects_locations_retrieve_api_views(
        &self,
        args: &ApihubProjectsLocationsRetrieveApiViewsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1RetrieveApiViewsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_retrieve_api_views_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_retrieve_api_views_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations search resources.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1SearchResourcesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apihub_projects_locations_search_resources(
        &self,
        args: &ApihubProjectsLocationsSearchResourcesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1SearchResourcesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_search_resources_builder(
            &self.http_client,
            &args.location,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_search_resources_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations addons get.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1Addon result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apihub_projects_locations_addons_get(
        &self,
        args: &ApihubProjectsLocationsAddonsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1Addon, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_addons_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_addons_get_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations addons list.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1ListAddonsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apihub_projects_locations_addons_list(
        &self,
        args: &ApihubProjectsLocationsAddonsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1ListAddonsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_addons_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_addons_list_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations addons manage config.
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
    pub fn apihub_projects_locations_addons_manage_config(
        &self,
        args: &ApihubProjectsLocationsAddonsManageConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_addons_manage_config_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_addons_manage_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations api hub instances create.
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
    pub fn apihub_projects_locations_api_hub_instances_create(
        &self,
        args: &ApihubProjectsLocationsApiHubInstancesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_api_hub_instances_create_builder(
            &self.http_client,
            &args.parent,
            &args.apiHubInstanceId,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_api_hub_instances_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations api hub instances delete.
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
    pub fn apihub_projects_locations_api_hub_instances_delete(
        &self,
        args: &ApihubProjectsLocationsApiHubInstancesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_api_hub_instances_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_api_hub_instances_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations api hub instances get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1ApiHubInstance result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apihub_projects_locations_api_hub_instances_get(
        &self,
        args: &ApihubProjectsLocationsApiHubInstancesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1ApiHubInstance, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_api_hub_instances_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_api_hub_instances_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations api hub instances lookup.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1LookupApiHubInstanceResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apihub_projects_locations_api_hub_instances_lookup(
        &self,
        args: &ApihubProjectsLocationsApiHubInstancesLookupArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1LookupApiHubInstanceResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_api_hub_instances_lookup_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_api_hub_instances_lookup_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations api hub instances patch.
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
    pub fn apihub_projects_locations_api_hub_instances_patch(
        &self,
        args: &ApihubProjectsLocationsApiHubInstancesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_api_hub_instances_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_api_hub_instances_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations apis create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1Api result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apihub_projects_locations_apis_create(
        &self,
        args: &ApihubProjectsLocationsApisCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1Api, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_apis_create_builder(
            &self.http_client,
            &args.parent,
            &args.apiId,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_apis_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations apis delete.
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
    pub fn apihub_projects_locations_apis_delete(
        &self,
        args: &ApihubProjectsLocationsApisDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_apis_delete_builder(
            &self.http_client,
            &args.name,
            &args.force,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_apis_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations apis get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1Api result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apihub_projects_locations_apis_get(
        &self,
        args: &ApihubProjectsLocationsApisGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1Api, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_apis_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_apis_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations apis list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1ListApisResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apihub_projects_locations_apis_list(
        &self,
        args: &ApihubProjectsLocationsApisListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1ListApisResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_apis_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_apis_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations apis patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1Api result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apihub_projects_locations_apis_patch(
        &self,
        args: &ApihubProjectsLocationsApisPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1Api, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_apis_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_apis_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations apis versions create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1Version result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apihub_projects_locations_apis_versions_create(
        &self,
        args: &ApihubProjectsLocationsApisVersionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1Version, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_apis_versions_create_builder(
            &self.http_client,
            &args.parent,
            &args.versionId,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_apis_versions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations apis versions delete.
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
    pub fn apihub_projects_locations_apis_versions_delete(
        &self,
        args: &ApihubProjectsLocationsApisVersionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_apis_versions_delete_builder(
            &self.http_client,
            &args.name,
            &args.force,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_apis_versions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations apis versions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1Version result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apihub_projects_locations_apis_versions_get(
        &self,
        args: &ApihubProjectsLocationsApisVersionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1Version, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_apis_versions_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_apis_versions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations apis versions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1ListVersionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apihub_projects_locations_apis_versions_list(
        &self,
        args: &ApihubProjectsLocationsApisVersionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1ListVersionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_apis_versions_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_apis_versions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations apis versions patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1Version result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apihub_projects_locations_apis_versions_patch(
        &self,
        args: &ApihubProjectsLocationsApisVersionsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1Version, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_apis_versions_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_apis_versions_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations apis versions definitions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1Definition result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apihub_projects_locations_apis_versions_definitions_get(
        &self,
        args: &ApihubProjectsLocationsApisVersionsDefinitionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1Definition, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_apis_versions_definitions_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_apis_versions_definitions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations apis versions operations create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1ApiOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apihub_projects_locations_apis_versions_operations_create(
        &self,
        args: &ApihubProjectsLocationsApisVersionsOperationsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1ApiOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_apis_versions_operations_create_builder(
            &self.http_client,
            &args.parent,
            &args.apiOperationId,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_apis_versions_operations_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations apis versions operations delete.
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
    pub fn apihub_projects_locations_apis_versions_operations_delete(
        &self,
        args: &ApihubProjectsLocationsApisVersionsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_apis_versions_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_apis_versions_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations apis versions operations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1ApiOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apihub_projects_locations_apis_versions_operations_get(
        &self,
        args: &ApihubProjectsLocationsApisVersionsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1ApiOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_apis_versions_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_apis_versions_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations apis versions operations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1ListApiOperationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apihub_projects_locations_apis_versions_operations_list(
        &self,
        args: &ApihubProjectsLocationsApisVersionsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1ListApiOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_apis_versions_operations_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_apis_versions_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations apis versions operations patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1ApiOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apihub_projects_locations_apis_versions_operations_patch(
        &self,
        args: &ApihubProjectsLocationsApisVersionsOperationsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1ApiOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_apis_versions_operations_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_apis_versions_operations_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations apis versions specs create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1Spec result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apihub_projects_locations_apis_versions_specs_create(
        &self,
        args: &ApihubProjectsLocationsApisVersionsSpecsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1Spec, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_apis_versions_specs_create_builder(
            &self.http_client,
            &args.parent,
            &args.specId,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_apis_versions_specs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations apis versions specs delete.
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
    pub fn apihub_projects_locations_apis_versions_specs_delete(
        &self,
        args: &ApihubProjectsLocationsApisVersionsSpecsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_apis_versions_specs_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_apis_versions_specs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations apis versions specs fetch additional spec content.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1FetchAdditionalSpecContentResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apihub_projects_locations_apis_versions_specs_fetch_additional_spec_content(
        &self,
        args: &ApihubProjectsLocationsApisVersionsSpecsFetchAdditionalSpecContentArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1FetchAdditionalSpecContentResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_apis_versions_specs_fetch_additional_spec_content_builder(
            &self.http_client,
            &args.name,
            &args.specContentType,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_apis_versions_specs_fetch_additional_spec_content_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations apis versions specs get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1Spec result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apihub_projects_locations_apis_versions_specs_get(
        &self,
        args: &ApihubProjectsLocationsApisVersionsSpecsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1Spec, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_apis_versions_specs_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_apis_versions_specs_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations apis versions specs get contents.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1SpecContents result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apihub_projects_locations_apis_versions_specs_get_contents(
        &self,
        args: &ApihubProjectsLocationsApisVersionsSpecsGetContentsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1SpecContents, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_apis_versions_specs_get_contents_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_apis_versions_specs_get_contents_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations apis versions specs lint.
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
    pub fn apihub_projects_locations_apis_versions_specs_lint(
        &self,
        args: &ApihubProjectsLocationsApisVersionsSpecsLintArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_apis_versions_specs_lint_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_apis_versions_specs_lint_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations apis versions specs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1ListSpecsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apihub_projects_locations_apis_versions_specs_list(
        &self,
        args: &ApihubProjectsLocationsApisVersionsSpecsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1ListSpecsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_apis_versions_specs_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_apis_versions_specs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations apis versions specs patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1Spec result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apihub_projects_locations_apis_versions_specs_patch(
        &self,
        args: &ApihubProjectsLocationsApisVersionsSpecsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1Spec, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_apis_versions_specs_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_apis_versions_specs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations attributes create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1Attribute result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apihub_projects_locations_attributes_create(
        &self,
        args: &ApihubProjectsLocationsAttributesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1Attribute, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_attributes_create_builder(
            &self.http_client,
            &args.parent,
            &args.attributeId,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_attributes_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations attributes delete.
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
    pub fn apihub_projects_locations_attributes_delete(
        &self,
        args: &ApihubProjectsLocationsAttributesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_attributes_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_attributes_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations attributes get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1Attribute result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apihub_projects_locations_attributes_get(
        &self,
        args: &ApihubProjectsLocationsAttributesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1Attribute, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_attributes_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_attributes_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations attributes list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1ListAttributesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apihub_projects_locations_attributes_list(
        &self,
        args: &ApihubProjectsLocationsAttributesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1ListAttributesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_attributes_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_attributes_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations attributes patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1Attribute result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apihub_projects_locations_attributes_patch(
        &self,
        args: &ApihubProjectsLocationsAttributesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1Attribute, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_attributes_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_attributes_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations curations create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1Curation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apihub_projects_locations_curations_create(
        &self,
        args: &ApihubProjectsLocationsCurationsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1Curation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_curations_create_builder(
            &self.http_client,
            &args.parent,
            &args.curationId,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_curations_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations curations delete.
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
    pub fn apihub_projects_locations_curations_delete(
        &self,
        args: &ApihubProjectsLocationsCurationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_curations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_curations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations curations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1Curation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apihub_projects_locations_curations_get(
        &self,
        args: &ApihubProjectsLocationsCurationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1Curation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_curations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_curations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations curations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1ListCurationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apihub_projects_locations_curations_list(
        &self,
        args: &ApihubProjectsLocationsCurationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1ListCurationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_curations_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_curations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations curations patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1Curation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apihub_projects_locations_curations_patch(
        &self,
        args: &ApihubProjectsLocationsCurationsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1Curation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_curations_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_curations_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations dependencies create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1Dependency result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apihub_projects_locations_dependencies_create(
        &self,
        args: &ApihubProjectsLocationsDependenciesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1Dependency, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_dependencies_create_builder(
            &self.http_client,
            &args.parent,
            &args.dependencyId,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_dependencies_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations dependencies delete.
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
    pub fn apihub_projects_locations_dependencies_delete(
        &self,
        args: &ApihubProjectsLocationsDependenciesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_dependencies_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_dependencies_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations dependencies get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1Dependency result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apihub_projects_locations_dependencies_get(
        &self,
        args: &ApihubProjectsLocationsDependenciesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1Dependency, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_dependencies_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_dependencies_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations dependencies list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1ListDependenciesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apihub_projects_locations_dependencies_list(
        &self,
        args: &ApihubProjectsLocationsDependenciesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1ListDependenciesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_dependencies_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_dependencies_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations dependencies patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1Dependency result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apihub_projects_locations_dependencies_patch(
        &self,
        args: &ApihubProjectsLocationsDependenciesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1Dependency, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_dependencies_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_dependencies_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations deployments create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1Deployment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apihub_projects_locations_deployments_create(
        &self,
        args: &ApihubProjectsLocationsDeploymentsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1Deployment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_deployments_create_builder(
            &self.http_client,
            &args.parent,
            &args.deploymentId,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_deployments_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations deployments delete.
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
    pub fn apihub_projects_locations_deployments_delete(
        &self,
        args: &ApihubProjectsLocationsDeploymentsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_deployments_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_deployments_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations deployments get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1Deployment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apihub_projects_locations_deployments_get(
        &self,
        args: &ApihubProjectsLocationsDeploymentsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1Deployment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_deployments_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_deployments_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations deployments list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1ListDeploymentsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apihub_projects_locations_deployments_list(
        &self,
        args: &ApihubProjectsLocationsDeploymentsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1ListDeploymentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_deployments_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_deployments_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations deployments patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1Deployment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apihub_projects_locations_deployments_patch(
        &self,
        args: &ApihubProjectsLocationsDeploymentsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1Deployment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_deployments_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_deployments_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations discovered api observations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1DiscoveredApiObservation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apihub_projects_locations_discovered_api_observations_get(
        &self,
        args: &ApihubProjectsLocationsDiscoveredApiObservationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1DiscoveredApiObservation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_discovered_api_observations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_discovered_api_observations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations discovered api observations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1ListDiscoveredApiObservationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apihub_projects_locations_discovered_api_observations_list(
        &self,
        args: &ApihubProjectsLocationsDiscoveredApiObservationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1ListDiscoveredApiObservationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_discovered_api_observations_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_discovered_api_observations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations discovered api observations discovered api operations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1DiscoveredApiOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apihub_projects_locations_discovered_api_observations_discovered_api_operations_get(
        &self,
        args: &ApihubProjectsLocationsDiscoveredApiObservationsDiscoveredApiOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1DiscoveredApiOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_discovered_api_observations_discovered_api_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_discovered_api_observations_discovered_api_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations discovered api observations discovered api operations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1ListDiscoveredApiOperationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apihub_projects_locations_discovered_api_observations_discovered_api_operations_list(
        &self,
        args: &ApihubProjectsLocationsDiscoveredApiObservationsDiscoveredApiOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1ListDiscoveredApiOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_discovered_api_observations_discovered_api_operations_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_discovered_api_observations_discovered_api_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations external apis create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1ExternalApi result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apihub_projects_locations_external_apis_create(
        &self,
        args: &ApihubProjectsLocationsExternalApisCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1ExternalApi, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_external_apis_create_builder(
            &self.http_client,
            &args.parent,
            &args.externalApiId,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_external_apis_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations external apis delete.
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
    pub fn apihub_projects_locations_external_apis_delete(
        &self,
        args: &ApihubProjectsLocationsExternalApisDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_external_apis_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_external_apis_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations external apis get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1ExternalApi result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apihub_projects_locations_external_apis_get(
        &self,
        args: &ApihubProjectsLocationsExternalApisGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1ExternalApi, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_external_apis_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_external_apis_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations external apis list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1ListExternalApisResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apihub_projects_locations_external_apis_list(
        &self,
        args: &ApihubProjectsLocationsExternalApisListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1ListExternalApisResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_external_apis_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_external_apis_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations external apis patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1ExternalApi result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apihub_projects_locations_external_apis_patch(
        &self,
        args: &ApihubProjectsLocationsExternalApisPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1ExternalApi, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_external_apis_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_external_apis_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations host project registrations create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1HostProjectRegistration result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apihub_projects_locations_host_project_registrations_create(
        &self,
        args: &ApihubProjectsLocationsHostProjectRegistrationsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1HostProjectRegistration, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_host_project_registrations_create_builder(
            &self.http_client,
            &args.parent,
            &args.hostProjectRegistrationId,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_host_project_registrations_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations host project registrations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1HostProjectRegistration result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apihub_projects_locations_host_project_registrations_get(
        &self,
        args: &ApihubProjectsLocationsHostProjectRegistrationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1HostProjectRegistration, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_host_project_registrations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_host_project_registrations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations host project registrations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1ListHostProjectRegistrationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apihub_projects_locations_host_project_registrations_list(
        &self,
        args: &ApihubProjectsLocationsHostProjectRegistrationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1ListHostProjectRegistrationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_host_project_registrations_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_host_project_registrations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations operations cancel.
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
    pub fn apihub_projects_locations_operations_cancel(
        &self,
        args: &ApihubProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations operations delete.
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
    pub fn apihub_projects_locations_operations_delete(
        &self,
        args: &ApihubProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations operations get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn apihub_projects_locations_operations_get(
        &self,
        args: &ApihubProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations operations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningListOperationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apihub_projects_locations_operations_list(
        &self,
        args: &ApihubProjectsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations plugins create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1Plugin result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apihub_projects_locations_plugins_create(
        &self,
        args: &ApihubProjectsLocationsPluginsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1Plugin, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_plugins_create_builder(
            &self.http_client,
            &args.parent,
            &args.pluginId,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_plugins_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations plugins delete.
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
    pub fn apihub_projects_locations_plugins_delete(
        &self,
        args: &ApihubProjectsLocationsPluginsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_plugins_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_plugins_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations plugins disable.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1Plugin result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apihub_projects_locations_plugins_disable(
        &self,
        args: &ApihubProjectsLocationsPluginsDisableArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1Plugin, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_plugins_disable_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_plugins_disable_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations plugins enable.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1Plugin result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apihub_projects_locations_plugins_enable(
        &self,
        args: &ApihubProjectsLocationsPluginsEnableArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1Plugin, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_plugins_enable_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_plugins_enable_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations plugins get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1Plugin result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apihub_projects_locations_plugins_get(
        &self,
        args: &ApihubProjectsLocationsPluginsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1Plugin, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_plugins_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_plugins_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations plugins get style guide.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1StyleGuide result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apihub_projects_locations_plugins_get_style_guide(
        &self,
        args: &ApihubProjectsLocationsPluginsGetStyleGuideArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1StyleGuide, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_plugins_get_style_guide_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_plugins_get_style_guide_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations plugins list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1ListPluginsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apihub_projects_locations_plugins_list(
        &self,
        args: &ApihubProjectsLocationsPluginsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1ListPluginsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_plugins_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_plugins_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations plugins update style guide.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1StyleGuide result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apihub_projects_locations_plugins_update_style_guide(
        &self,
        args: &ApihubProjectsLocationsPluginsUpdateStyleGuideArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1StyleGuide, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_plugins_update_style_guide_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_plugins_update_style_guide_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations plugins instances create.
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
    pub fn apihub_projects_locations_plugins_instances_create(
        &self,
        args: &ApihubProjectsLocationsPluginsInstancesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_plugins_instances_create_builder(
            &self.http_client,
            &args.parent,
            &args.pluginInstanceId,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_plugins_instances_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations plugins instances delete.
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
    pub fn apihub_projects_locations_plugins_instances_delete(
        &self,
        args: &ApihubProjectsLocationsPluginsInstancesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_plugins_instances_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_plugins_instances_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations plugins instances disable action.
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
    pub fn apihub_projects_locations_plugins_instances_disable_action(
        &self,
        args: &ApihubProjectsLocationsPluginsInstancesDisableActionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_plugins_instances_disable_action_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_plugins_instances_disable_action_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations plugins instances enable action.
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
    pub fn apihub_projects_locations_plugins_instances_enable_action(
        &self,
        args: &ApihubProjectsLocationsPluginsInstancesEnableActionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_plugins_instances_enable_action_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_plugins_instances_enable_action_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations plugins instances execute action.
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
    pub fn apihub_projects_locations_plugins_instances_execute_action(
        &self,
        args: &ApihubProjectsLocationsPluginsInstancesExecuteActionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_plugins_instances_execute_action_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_plugins_instances_execute_action_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations plugins instances get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1PluginInstance result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apihub_projects_locations_plugins_instances_get(
        &self,
        args: &ApihubProjectsLocationsPluginsInstancesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1PluginInstance, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_plugins_instances_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_plugins_instances_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations plugins instances list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1ListPluginInstancesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apihub_projects_locations_plugins_instances_list(
        &self,
        args: &ApihubProjectsLocationsPluginsInstancesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1ListPluginInstancesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_plugins_instances_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_plugins_instances_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations plugins instances manage source data.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1ManagePluginInstanceSourceDataResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apihub_projects_locations_plugins_instances_manage_source_data(
        &self,
        args: &ApihubProjectsLocationsPluginsInstancesManageSourceDataArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1ManagePluginInstanceSourceDataResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_plugins_instances_manage_source_data_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_plugins_instances_manage_source_data_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations plugins instances patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1PluginInstance result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apihub_projects_locations_plugins_instances_patch(
        &self,
        args: &ApihubProjectsLocationsPluginsInstancesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1PluginInstance, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_plugins_instances_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_plugins_instances_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations plugins style guide get contents.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1StyleGuideContents result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apihub_projects_locations_plugins_style_guide_get_contents(
        &self,
        args: &ApihubProjectsLocationsPluginsStyleGuideGetContentsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1StyleGuideContents, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_plugins_style_guide_get_contents_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_plugins_style_guide_get_contents_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations runtime project attachments create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1RuntimeProjectAttachment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apihub_projects_locations_runtime_project_attachments_create(
        &self,
        args: &ApihubProjectsLocationsRuntimeProjectAttachmentsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1RuntimeProjectAttachment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_runtime_project_attachments_create_builder(
            &self.http_client,
            &args.parent,
            &args.runtimeProjectAttachmentId,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_runtime_project_attachments_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations runtime project attachments delete.
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
    pub fn apihub_projects_locations_runtime_project_attachments_delete(
        &self,
        args: &ApihubProjectsLocationsRuntimeProjectAttachmentsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_runtime_project_attachments_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_runtime_project_attachments_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations runtime project attachments get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1RuntimeProjectAttachment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apihub_projects_locations_runtime_project_attachments_get(
        &self,
        args: &ApihubProjectsLocationsRuntimeProjectAttachmentsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1RuntimeProjectAttachment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_runtime_project_attachments_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_runtime_project_attachments_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apihub projects locations runtime project attachments list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudApihubV1ListRuntimeProjectAttachmentsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apihub_projects_locations_runtime_project_attachments_list(
        &self,
        args: &ApihubProjectsLocationsRuntimeProjectAttachmentsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudApihubV1ListRuntimeProjectAttachmentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apihub_projects_locations_runtime_project_attachments_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = apihub_projects_locations_runtime_project_attachments_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
