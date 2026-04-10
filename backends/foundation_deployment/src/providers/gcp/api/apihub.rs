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
    apihub_projects_locations_search_resources_builder, apihub_projects_locations_search_resources_task,
    apihub_projects_locations_addons_manage_config_builder, apihub_projects_locations_addons_manage_config_task,
    apihub_projects_locations_api_hub_instances_create_builder, apihub_projects_locations_api_hub_instances_create_task,
    apihub_projects_locations_api_hub_instances_delete_builder, apihub_projects_locations_api_hub_instances_delete_task,
    apihub_projects_locations_api_hub_instances_patch_builder, apihub_projects_locations_api_hub_instances_patch_task,
    apihub_projects_locations_apis_create_builder, apihub_projects_locations_apis_create_task,
    apihub_projects_locations_apis_delete_builder, apihub_projects_locations_apis_delete_task,
    apihub_projects_locations_apis_patch_builder, apihub_projects_locations_apis_patch_task,
    apihub_projects_locations_apis_versions_create_builder, apihub_projects_locations_apis_versions_create_task,
    apihub_projects_locations_apis_versions_delete_builder, apihub_projects_locations_apis_versions_delete_task,
    apihub_projects_locations_apis_versions_patch_builder, apihub_projects_locations_apis_versions_patch_task,
    apihub_projects_locations_apis_versions_operations_create_builder, apihub_projects_locations_apis_versions_operations_create_task,
    apihub_projects_locations_apis_versions_operations_delete_builder, apihub_projects_locations_apis_versions_operations_delete_task,
    apihub_projects_locations_apis_versions_operations_patch_builder, apihub_projects_locations_apis_versions_operations_patch_task,
    apihub_projects_locations_apis_versions_specs_create_builder, apihub_projects_locations_apis_versions_specs_create_task,
    apihub_projects_locations_apis_versions_specs_delete_builder, apihub_projects_locations_apis_versions_specs_delete_task,
    apihub_projects_locations_apis_versions_specs_lint_builder, apihub_projects_locations_apis_versions_specs_lint_task,
    apihub_projects_locations_apis_versions_specs_patch_builder, apihub_projects_locations_apis_versions_specs_patch_task,
    apihub_projects_locations_attributes_create_builder, apihub_projects_locations_attributes_create_task,
    apihub_projects_locations_attributes_delete_builder, apihub_projects_locations_attributes_delete_task,
    apihub_projects_locations_attributes_patch_builder, apihub_projects_locations_attributes_patch_task,
    apihub_projects_locations_curations_create_builder, apihub_projects_locations_curations_create_task,
    apihub_projects_locations_curations_delete_builder, apihub_projects_locations_curations_delete_task,
    apihub_projects_locations_curations_patch_builder, apihub_projects_locations_curations_patch_task,
    apihub_projects_locations_dependencies_create_builder, apihub_projects_locations_dependencies_create_task,
    apihub_projects_locations_dependencies_delete_builder, apihub_projects_locations_dependencies_delete_task,
    apihub_projects_locations_dependencies_patch_builder, apihub_projects_locations_dependencies_patch_task,
    apihub_projects_locations_deployments_create_builder, apihub_projects_locations_deployments_create_task,
    apihub_projects_locations_deployments_delete_builder, apihub_projects_locations_deployments_delete_task,
    apihub_projects_locations_deployments_patch_builder, apihub_projects_locations_deployments_patch_task,
    apihub_projects_locations_external_apis_create_builder, apihub_projects_locations_external_apis_create_task,
    apihub_projects_locations_external_apis_delete_builder, apihub_projects_locations_external_apis_delete_task,
    apihub_projects_locations_external_apis_patch_builder, apihub_projects_locations_external_apis_patch_task,
    apihub_projects_locations_host_project_registrations_create_builder, apihub_projects_locations_host_project_registrations_create_task,
    apihub_projects_locations_operations_cancel_builder, apihub_projects_locations_operations_cancel_task,
    apihub_projects_locations_operations_delete_builder, apihub_projects_locations_operations_delete_task,
    apihub_projects_locations_plugins_create_builder, apihub_projects_locations_plugins_create_task,
    apihub_projects_locations_plugins_delete_builder, apihub_projects_locations_plugins_delete_task,
    apihub_projects_locations_plugins_disable_builder, apihub_projects_locations_plugins_disable_task,
    apihub_projects_locations_plugins_enable_builder, apihub_projects_locations_plugins_enable_task,
    apihub_projects_locations_plugins_update_style_guide_builder, apihub_projects_locations_plugins_update_style_guide_task,
    apihub_projects_locations_plugins_instances_create_builder, apihub_projects_locations_plugins_instances_create_task,
    apihub_projects_locations_plugins_instances_delete_builder, apihub_projects_locations_plugins_instances_delete_task,
    apihub_projects_locations_plugins_instances_disable_action_builder, apihub_projects_locations_plugins_instances_disable_action_task,
    apihub_projects_locations_plugins_instances_enable_action_builder, apihub_projects_locations_plugins_instances_enable_action_task,
    apihub_projects_locations_plugins_instances_execute_action_builder, apihub_projects_locations_plugins_instances_execute_action_task,
    apihub_projects_locations_plugins_instances_manage_source_data_builder, apihub_projects_locations_plugins_instances_manage_source_data_task,
    apihub_projects_locations_plugins_instances_patch_builder, apihub_projects_locations_plugins_instances_patch_task,
    apihub_projects_locations_runtime_project_attachments_create_builder, apihub_projects_locations_runtime_project_attachments_create_task,
    apihub_projects_locations_runtime_project_attachments_delete_builder, apihub_projects_locations_runtime_project_attachments_delete_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::apihub::Empty;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1Api;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1ApiOperation;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1Attribute;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1Curation;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1Dependency;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1Deployment;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1ExternalApi;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1HostProjectRegistration;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1ManagePluginInstanceSourceDataResponse;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1Plugin;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1PluginInstance;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1RuntimeProjectAttachment;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1SearchResourcesResponse;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1Spec;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1StyleGuide;
use crate::providers::gcp::clients::apihub::GoogleCloudApihubV1Version;
use crate::providers::gcp::clients::apihub::GoogleLongrunningOperation;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsAddonsManageConfigArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsApiHubInstancesCreateArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsApiHubInstancesDeleteArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsApiHubInstancesPatchArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsApisCreateArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsApisDeleteArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsApisPatchArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsApisVersionsCreateArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsApisVersionsDeleteArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsApisVersionsOperationsCreateArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsApisVersionsOperationsDeleteArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsApisVersionsOperationsPatchArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsApisVersionsPatchArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsApisVersionsSpecsCreateArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsApisVersionsSpecsDeleteArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsApisVersionsSpecsLintArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsApisVersionsSpecsPatchArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsAttributesCreateArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsAttributesDeleteArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsAttributesPatchArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsCollectApiDataArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsCurationsCreateArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsCurationsDeleteArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsCurationsPatchArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsDependenciesCreateArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsDependenciesDeleteArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsDependenciesPatchArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsDeploymentsCreateArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsDeploymentsDeleteArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsDeploymentsPatchArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsExternalApisCreateArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsExternalApisDeleteArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsExternalApisPatchArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsHostProjectRegistrationsCreateArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsPluginsCreateArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsPluginsDeleteArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsPluginsDisableArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsPluginsEnableArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsPluginsInstancesCreateArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsPluginsInstancesDeleteArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsPluginsInstancesDisableActionArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsPluginsInstancesEnableActionArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsPluginsInstancesExecuteActionArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsPluginsInstancesManageSourceDataArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsPluginsInstancesPatchArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsPluginsUpdateStyleGuideArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsRuntimeProjectAttachmentsCreateArgs;
use crate::providers::gcp::clients::apihub::ApihubProjectsLocationsRuntimeProjectAttachmentsDeleteArgs;
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

    /// Apihub projects locations search resources.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
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

}
