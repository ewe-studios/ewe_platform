//! DlpProvider - State-aware dlp API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       dlp API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::dlp::{
    dlp_organizations_deidentify_templates_create_builder, dlp_organizations_deidentify_templates_create_task,
    dlp_organizations_deidentify_templates_delete_builder, dlp_organizations_deidentify_templates_delete_task,
    dlp_organizations_deidentify_templates_patch_builder, dlp_organizations_deidentify_templates_patch_task,
    dlp_organizations_inspect_templates_create_builder, dlp_organizations_inspect_templates_create_task,
    dlp_organizations_inspect_templates_delete_builder, dlp_organizations_inspect_templates_delete_task,
    dlp_organizations_inspect_templates_patch_builder, dlp_organizations_inspect_templates_patch_task,
    dlp_organizations_locations_connections_create_builder, dlp_organizations_locations_connections_create_task,
    dlp_organizations_locations_connections_delete_builder, dlp_organizations_locations_connections_delete_task,
    dlp_organizations_locations_connections_patch_builder, dlp_organizations_locations_connections_patch_task,
    dlp_organizations_locations_deidentify_templates_create_builder, dlp_organizations_locations_deidentify_templates_create_task,
    dlp_organizations_locations_deidentify_templates_delete_builder, dlp_organizations_locations_deidentify_templates_delete_task,
    dlp_organizations_locations_deidentify_templates_patch_builder, dlp_organizations_locations_deidentify_templates_patch_task,
    dlp_organizations_locations_discovery_configs_create_builder, dlp_organizations_locations_discovery_configs_create_task,
    dlp_organizations_locations_discovery_configs_delete_builder, dlp_organizations_locations_discovery_configs_delete_task,
    dlp_organizations_locations_discovery_configs_patch_builder, dlp_organizations_locations_discovery_configs_patch_task,
    dlp_organizations_locations_file_store_data_profiles_delete_builder, dlp_organizations_locations_file_store_data_profiles_delete_task,
    dlp_organizations_locations_inspect_templates_create_builder, dlp_organizations_locations_inspect_templates_create_task,
    dlp_organizations_locations_inspect_templates_delete_builder, dlp_organizations_locations_inspect_templates_delete_task,
    dlp_organizations_locations_inspect_templates_patch_builder, dlp_organizations_locations_inspect_templates_patch_task,
    dlp_organizations_locations_job_triggers_create_builder, dlp_organizations_locations_job_triggers_create_task,
    dlp_organizations_locations_job_triggers_delete_builder, dlp_organizations_locations_job_triggers_delete_task,
    dlp_organizations_locations_job_triggers_patch_builder, dlp_organizations_locations_job_triggers_patch_task,
    dlp_organizations_locations_stored_info_types_create_builder, dlp_organizations_locations_stored_info_types_create_task,
    dlp_organizations_locations_stored_info_types_delete_builder, dlp_organizations_locations_stored_info_types_delete_task,
    dlp_organizations_locations_stored_info_types_patch_builder, dlp_organizations_locations_stored_info_types_patch_task,
    dlp_organizations_locations_table_data_profiles_delete_builder, dlp_organizations_locations_table_data_profiles_delete_task,
    dlp_organizations_stored_info_types_create_builder, dlp_organizations_stored_info_types_create_task,
    dlp_organizations_stored_info_types_delete_builder, dlp_organizations_stored_info_types_delete_task,
    dlp_organizations_stored_info_types_patch_builder, dlp_organizations_stored_info_types_patch_task,
    dlp_projects_content_deidentify_builder, dlp_projects_content_deidentify_task,
    dlp_projects_content_inspect_builder, dlp_projects_content_inspect_task,
    dlp_projects_content_reidentify_builder, dlp_projects_content_reidentify_task,
    dlp_projects_deidentify_templates_create_builder, dlp_projects_deidentify_templates_create_task,
    dlp_projects_deidentify_templates_delete_builder, dlp_projects_deidentify_templates_delete_task,
    dlp_projects_deidentify_templates_patch_builder, dlp_projects_deidentify_templates_patch_task,
    dlp_projects_dlp_jobs_cancel_builder, dlp_projects_dlp_jobs_cancel_task,
    dlp_projects_dlp_jobs_create_builder, dlp_projects_dlp_jobs_create_task,
    dlp_projects_dlp_jobs_delete_builder, dlp_projects_dlp_jobs_delete_task,
    dlp_projects_image_redact_builder, dlp_projects_image_redact_task,
    dlp_projects_inspect_templates_create_builder, dlp_projects_inspect_templates_create_task,
    dlp_projects_inspect_templates_delete_builder, dlp_projects_inspect_templates_delete_task,
    dlp_projects_inspect_templates_patch_builder, dlp_projects_inspect_templates_patch_task,
    dlp_projects_job_triggers_activate_builder, dlp_projects_job_triggers_activate_task,
    dlp_projects_job_triggers_create_builder, dlp_projects_job_triggers_create_task,
    dlp_projects_job_triggers_delete_builder, dlp_projects_job_triggers_delete_task,
    dlp_projects_job_triggers_patch_builder, dlp_projects_job_triggers_patch_task,
    dlp_projects_locations_connections_create_builder, dlp_projects_locations_connections_create_task,
    dlp_projects_locations_connections_delete_builder, dlp_projects_locations_connections_delete_task,
    dlp_projects_locations_connections_patch_builder, dlp_projects_locations_connections_patch_task,
    dlp_projects_locations_content_deidentify_builder, dlp_projects_locations_content_deidentify_task,
    dlp_projects_locations_content_inspect_builder, dlp_projects_locations_content_inspect_task,
    dlp_projects_locations_content_reidentify_builder, dlp_projects_locations_content_reidentify_task,
    dlp_projects_locations_deidentify_templates_create_builder, dlp_projects_locations_deidentify_templates_create_task,
    dlp_projects_locations_deidentify_templates_delete_builder, dlp_projects_locations_deidentify_templates_delete_task,
    dlp_projects_locations_deidentify_templates_patch_builder, dlp_projects_locations_deidentify_templates_patch_task,
    dlp_projects_locations_discovery_configs_create_builder, dlp_projects_locations_discovery_configs_create_task,
    dlp_projects_locations_discovery_configs_delete_builder, dlp_projects_locations_discovery_configs_delete_task,
    dlp_projects_locations_discovery_configs_patch_builder, dlp_projects_locations_discovery_configs_patch_task,
    dlp_projects_locations_dlp_jobs_cancel_builder, dlp_projects_locations_dlp_jobs_cancel_task,
    dlp_projects_locations_dlp_jobs_create_builder, dlp_projects_locations_dlp_jobs_create_task,
    dlp_projects_locations_dlp_jobs_delete_builder, dlp_projects_locations_dlp_jobs_delete_task,
    dlp_projects_locations_dlp_jobs_finish_builder, dlp_projects_locations_dlp_jobs_finish_task,
    dlp_projects_locations_dlp_jobs_hybrid_inspect_builder, dlp_projects_locations_dlp_jobs_hybrid_inspect_task,
    dlp_projects_locations_file_store_data_profiles_delete_builder, dlp_projects_locations_file_store_data_profiles_delete_task,
    dlp_projects_locations_image_redact_builder, dlp_projects_locations_image_redact_task,
    dlp_projects_locations_inspect_templates_create_builder, dlp_projects_locations_inspect_templates_create_task,
    dlp_projects_locations_inspect_templates_delete_builder, dlp_projects_locations_inspect_templates_delete_task,
    dlp_projects_locations_inspect_templates_patch_builder, dlp_projects_locations_inspect_templates_patch_task,
    dlp_projects_locations_job_triggers_activate_builder, dlp_projects_locations_job_triggers_activate_task,
    dlp_projects_locations_job_triggers_create_builder, dlp_projects_locations_job_triggers_create_task,
    dlp_projects_locations_job_triggers_delete_builder, dlp_projects_locations_job_triggers_delete_task,
    dlp_projects_locations_job_triggers_hybrid_inspect_builder, dlp_projects_locations_job_triggers_hybrid_inspect_task,
    dlp_projects_locations_job_triggers_patch_builder, dlp_projects_locations_job_triggers_patch_task,
    dlp_projects_locations_stored_info_types_create_builder, dlp_projects_locations_stored_info_types_create_task,
    dlp_projects_locations_stored_info_types_delete_builder, dlp_projects_locations_stored_info_types_delete_task,
    dlp_projects_locations_stored_info_types_patch_builder, dlp_projects_locations_stored_info_types_patch_task,
    dlp_projects_locations_table_data_profiles_delete_builder, dlp_projects_locations_table_data_profiles_delete_task,
    dlp_projects_stored_info_types_create_builder, dlp_projects_stored_info_types_create_task,
    dlp_projects_stored_info_types_delete_builder, dlp_projects_stored_info_types_delete_task,
    dlp_projects_stored_info_types_patch_builder, dlp_projects_stored_info_types_patch_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::dlp::GooglePrivacyDlpV2Connection;
use crate::providers::gcp::clients::dlp::GooglePrivacyDlpV2DeidentifyContentResponse;
use crate::providers::gcp::clients::dlp::GooglePrivacyDlpV2DeidentifyTemplate;
use crate::providers::gcp::clients::dlp::GooglePrivacyDlpV2DiscoveryConfig;
use crate::providers::gcp::clients::dlp::GooglePrivacyDlpV2DlpJob;
use crate::providers::gcp::clients::dlp::GooglePrivacyDlpV2HybridInspectResponse;
use crate::providers::gcp::clients::dlp::GooglePrivacyDlpV2InspectContentResponse;
use crate::providers::gcp::clients::dlp::GooglePrivacyDlpV2InspectTemplate;
use crate::providers::gcp::clients::dlp::GooglePrivacyDlpV2JobTrigger;
use crate::providers::gcp::clients::dlp::GooglePrivacyDlpV2RedactImageResponse;
use crate::providers::gcp::clients::dlp::GooglePrivacyDlpV2ReidentifyContentResponse;
use crate::providers::gcp::clients::dlp::GooglePrivacyDlpV2StoredInfoType;
use crate::providers::gcp::clients::dlp::GoogleProtobufEmpty;
use crate::providers::gcp::clients::dlp::DlpOrganizationsDeidentifyTemplatesCreateArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsDeidentifyTemplatesDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsDeidentifyTemplatesPatchArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsInspectTemplatesCreateArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsInspectTemplatesDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsInspectTemplatesPatchArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsConnectionsCreateArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsConnectionsDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsConnectionsPatchArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsDeidentifyTemplatesCreateArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsDeidentifyTemplatesDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsDeidentifyTemplatesPatchArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsDiscoveryConfigsCreateArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsDiscoveryConfigsDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsDiscoveryConfigsPatchArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsFileStoreDataProfilesDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsInspectTemplatesCreateArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsInspectTemplatesDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsInspectTemplatesPatchArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsJobTriggersCreateArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsJobTriggersDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsJobTriggersPatchArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsStoredInfoTypesCreateArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsStoredInfoTypesDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsStoredInfoTypesPatchArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsTableDataProfilesDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsStoredInfoTypesCreateArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsStoredInfoTypesDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsStoredInfoTypesPatchArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsContentDeidentifyArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsContentInspectArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsContentReidentifyArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsDeidentifyTemplatesCreateArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsDeidentifyTemplatesDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsDeidentifyTemplatesPatchArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsDlpJobsCancelArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsDlpJobsCreateArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsDlpJobsDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsImageRedactArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsInspectTemplatesCreateArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsInspectTemplatesDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsInspectTemplatesPatchArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsJobTriggersActivateArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsJobTriggersCreateArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsJobTriggersDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsJobTriggersPatchArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsConnectionsCreateArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsConnectionsDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsConnectionsPatchArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsContentDeidentifyArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsContentInspectArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsContentReidentifyArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsDeidentifyTemplatesCreateArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsDeidentifyTemplatesDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsDeidentifyTemplatesPatchArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsDiscoveryConfigsCreateArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsDiscoveryConfigsDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsDiscoveryConfigsPatchArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsDlpJobsCancelArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsDlpJobsCreateArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsDlpJobsDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsDlpJobsFinishArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsDlpJobsHybridInspectArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsFileStoreDataProfilesDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsImageRedactArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsInspectTemplatesCreateArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsInspectTemplatesDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsInspectTemplatesPatchArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsJobTriggersActivateArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsJobTriggersCreateArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsJobTriggersDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsJobTriggersHybridInspectArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsJobTriggersPatchArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsStoredInfoTypesCreateArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsStoredInfoTypesDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsStoredInfoTypesPatchArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsTableDataProfilesDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsStoredInfoTypesCreateArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsStoredInfoTypesDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsStoredInfoTypesPatchArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// DlpProvider with automatic state tracking.
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
/// let provider = DlpProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct DlpProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> DlpProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new DlpProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Dlp organizations deidentify templates create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2DeidentifyTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_organizations_deidentify_templates_create(
        &self,
        args: &DlpOrganizationsDeidentifyTemplatesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2DeidentifyTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_deidentify_templates_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_deidentify_templates_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp organizations deidentify templates delete.
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
    pub fn dlp_organizations_deidentify_templates_delete(
        &self,
        args: &DlpOrganizationsDeidentifyTemplatesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_deidentify_templates_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_deidentify_templates_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp organizations deidentify templates patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2DeidentifyTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_organizations_deidentify_templates_patch(
        &self,
        args: &DlpOrganizationsDeidentifyTemplatesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2DeidentifyTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_deidentify_templates_patch_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_deidentify_templates_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp organizations inspect templates create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2InspectTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_organizations_inspect_templates_create(
        &self,
        args: &DlpOrganizationsInspectTemplatesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2InspectTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_inspect_templates_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_inspect_templates_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp organizations inspect templates delete.
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
    pub fn dlp_organizations_inspect_templates_delete(
        &self,
        args: &DlpOrganizationsInspectTemplatesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_inspect_templates_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_inspect_templates_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp organizations inspect templates patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2InspectTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_organizations_inspect_templates_patch(
        &self,
        args: &DlpOrganizationsInspectTemplatesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2InspectTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_inspect_templates_patch_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_inspect_templates_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp organizations locations connections create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2Connection result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_organizations_locations_connections_create(
        &self,
        args: &DlpOrganizationsLocationsConnectionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2Connection, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_locations_connections_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_locations_connections_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp organizations locations connections delete.
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
    pub fn dlp_organizations_locations_connections_delete(
        &self,
        args: &DlpOrganizationsLocationsConnectionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_locations_connections_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_locations_connections_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp organizations locations connections patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2Connection result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_organizations_locations_connections_patch(
        &self,
        args: &DlpOrganizationsLocationsConnectionsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2Connection, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_locations_connections_patch_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_locations_connections_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp organizations locations deidentify templates create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2DeidentifyTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_organizations_locations_deidentify_templates_create(
        &self,
        args: &DlpOrganizationsLocationsDeidentifyTemplatesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2DeidentifyTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_locations_deidentify_templates_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_locations_deidentify_templates_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp organizations locations deidentify templates delete.
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
    pub fn dlp_organizations_locations_deidentify_templates_delete(
        &self,
        args: &DlpOrganizationsLocationsDeidentifyTemplatesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_locations_deidentify_templates_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_locations_deidentify_templates_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp organizations locations deidentify templates patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2DeidentifyTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_organizations_locations_deidentify_templates_patch(
        &self,
        args: &DlpOrganizationsLocationsDeidentifyTemplatesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2DeidentifyTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_locations_deidentify_templates_patch_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_locations_deidentify_templates_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp organizations locations discovery configs create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2DiscoveryConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_organizations_locations_discovery_configs_create(
        &self,
        args: &DlpOrganizationsLocationsDiscoveryConfigsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2DiscoveryConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_locations_discovery_configs_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_locations_discovery_configs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp organizations locations discovery configs delete.
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
    pub fn dlp_organizations_locations_discovery_configs_delete(
        &self,
        args: &DlpOrganizationsLocationsDiscoveryConfigsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_locations_discovery_configs_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_locations_discovery_configs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp organizations locations discovery configs patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2DiscoveryConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_organizations_locations_discovery_configs_patch(
        &self,
        args: &DlpOrganizationsLocationsDiscoveryConfigsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2DiscoveryConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_locations_discovery_configs_patch_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_locations_discovery_configs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp organizations locations file store data profiles delete.
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
    pub fn dlp_organizations_locations_file_store_data_profiles_delete(
        &self,
        args: &DlpOrganizationsLocationsFileStoreDataProfilesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_locations_file_store_data_profiles_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_locations_file_store_data_profiles_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp organizations locations inspect templates create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2InspectTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_organizations_locations_inspect_templates_create(
        &self,
        args: &DlpOrganizationsLocationsInspectTemplatesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2InspectTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_locations_inspect_templates_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_locations_inspect_templates_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp organizations locations inspect templates delete.
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
    pub fn dlp_organizations_locations_inspect_templates_delete(
        &self,
        args: &DlpOrganizationsLocationsInspectTemplatesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_locations_inspect_templates_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_locations_inspect_templates_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp organizations locations inspect templates patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2InspectTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_organizations_locations_inspect_templates_patch(
        &self,
        args: &DlpOrganizationsLocationsInspectTemplatesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2InspectTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_locations_inspect_templates_patch_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_locations_inspect_templates_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp organizations locations job triggers create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2JobTrigger result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_organizations_locations_job_triggers_create(
        &self,
        args: &DlpOrganizationsLocationsJobTriggersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2JobTrigger, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_locations_job_triggers_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_locations_job_triggers_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp organizations locations job triggers delete.
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
    pub fn dlp_organizations_locations_job_triggers_delete(
        &self,
        args: &DlpOrganizationsLocationsJobTriggersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_locations_job_triggers_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_locations_job_triggers_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp organizations locations job triggers patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2JobTrigger result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_organizations_locations_job_triggers_patch(
        &self,
        args: &DlpOrganizationsLocationsJobTriggersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2JobTrigger, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_locations_job_triggers_patch_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_locations_job_triggers_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp organizations locations stored info types create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2StoredInfoType result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_organizations_locations_stored_info_types_create(
        &self,
        args: &DlpOrganizationsLocationsStoredInfoTypesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2StoredInfoType, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_locations_stored_info_types_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_locations_stored_info_types_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp organizations locations stored info types delete.
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
    pub fn dlp_organizations_locations_stored_info_types_delete(
        &self,
        args: &DlpOrganizationsLocationsStoredInfoTypesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_locations_stored_info_types_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_locations_stored_info_types_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp organizations locations stored info types patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2StoredInfoType result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_organizations_locations_stored_info_types_patch(
        &self,
        args: &DlpOrganizationsLocationsStoredInfoTypesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2StoredInfoType, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_locations_stored_info_types_patch_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_locations_stored_info_types_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp organizations locations table data profiles delete.
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
    pub fn dlp_organizations_locations_table_data_profiles_delete(
        &self,
        args: &DlpOrganizationsLocationsTableDataProfilesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_locations_table_data_profiles_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_locations_table_data_profiles_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp organizations stored info types create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2StoredInfoType result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_organizations_stored_info_types_create(
        &self,
        args: &DlpOrganizationsStoredInfoTypesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2StoredInfoType, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_stored_info_types_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_stored_info_types_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp organizations stored info types delete.
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
    pub fn dlp_organizations_stored_info_types_delete(
        &self,
        args: &DlpOrganizationsStoredInfoTypesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_stored_info_types_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_stored_info_types_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp organizations stored info types patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2StoredInfoType result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_organizations_stored_info_types_patch(
        &self,
        args: &DlpOrganizationsStoredInfoTypesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2StoredInfoType, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_stored_info_types_patch_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_stored_info_types_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects content deidentify.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2DeidentifyContentResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_projects_content_deidentify(
        &self,
        args: &DlpProjectsContentDeidentifyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2DeidentifyContentResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_content_deidentify_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_content_deidentify_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects content inspect.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2InspectContentResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_projects_content_inspect(
        &self,
        args: &DlpProjectsContentInspectArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2InspectContentResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_content_inspect_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_content_inspect_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects content reidentify.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2ReidentifyContentResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_projects_content_reidentify(
        &self,
        args: &DlpProjectsContentReidentifyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2ReidentifyContentResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_content_reidentify_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_content_reidentify_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects deidentify templates create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2DeidentifyTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_projects_deidentify_templates_create(
        &self,
        args: &DlpProjectsDeidentifyTemplatesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2DeidentifyTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_deidentify_templates_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_deidentify_templates_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects deidentify templates delete.
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
    pub fn dlp_projects_deidentify_templates_delete(
        &self,
        args: &DlpProjectsDeidentifyTemplatesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_deidentify_templates_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_deidentify_templates_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects deidentify templates patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2DeidentifyTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_projects_deidentify_templates_patch(
        &self,
        args: &DlpProjectsDeidentifyTemplatesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2DeidentifyTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_deidentify_templates_patch_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_deidentify_templates_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects dlp jobs cancel.
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
    pub fn dlp_projects_dlp_jobs_cancel(
        &self,
        args: &DlpProjectsDlpJobsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_dlp_jobs_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_dlp_jobs_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects dlp jobs create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2DlpJob result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_projects_dlp_jobs_create(
        &self,
        args: &DlpProjectsDlpJobsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2DlpJob, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_dlp_jobs_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_dlp_jobs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects dlp jobs delete.
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
    pub fn dlp_projects_dlp_jobs_delete(
        &self,
        args: &DlpProjectsDlpJobsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_dlp_jobs_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_dlp_jobs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects image redact.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2RedactImageResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_projects_image_redact(
        &self,
        args: &DlpProjectsImageRedactArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2RedactImageResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_image_redact_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_image_redact_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects inspect templates create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2InspectTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_projects_inspect_templates_create(
        &self,
        args: &DlpProjectsInspectTemplatesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2InspectTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_inspect_templates_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_inspect_templates_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects inspect templates delete.
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
    pub fn dlp_projects_inspect_templates_delete(
        &self,
        args: &DlpProjectsInspectTemplatesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_inspect_templates_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_inspect_templates_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects inspect templates patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2InspectTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_projects_inspect_templates_patch(
        &self,
        args: &DlpProjectsInspectTemplatesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2InspectTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_inspect_templates_patch_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_inspect_templates_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects job triggers activate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2DlpJob result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_projects_job_triggers_activate(
        &self,
        args: &DlpProjectsJobTriggersActivateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2DlpJob, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_job_triggers_activate_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_job_triggers_activate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects job triggers create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2JobTrigger result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_projects_job_triggers_create(
        &self,
        args: &DlpProjectsJobTriggersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2JobTrigger, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_job_triggers_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_job_triggers_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects job triggers delete.
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
    pub fn dlp_projects_job_triggers_delete(
        &self,
        args: &DlpProjectsJobTriggersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_job_triggers_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_job_triggers_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects job triggers patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2JobTrigger result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_projects_job_triggers_patch(
        &self,
        args: &DlpProjectsJobTriggersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2JobTrigger, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_job_triggers_patch_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_job_triggers_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects locations connections create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2Connection result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_projects_locations_connections_create(
        &self,
        args: &DlpProjectsLocationsConnectionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2Connection, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_connections_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_connections_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects locations connections delete.
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
    pub fn dlp_projects_locations_connections_delete(
        &self,
        args: &DlpProjectsLocationsConnectionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_connections_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_connections_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects locations connections patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2Connection result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_projects_locations_connections_patch(
        &self,
        args: &DlpProjectsLocationsConnectionsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2Connection, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_connections_patch_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_connections_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects locations content deidentify.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2DeidentifyContentResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_projects_locations_content_deidentify(
        &self,
        args: &DlpProjectsLocationsContentDeidentifyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2DeidentifyContentResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_content_deidentify_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_content_deidentify_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects locations content inspect.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2InspectContentResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_projects_locations_content_inspect(
        &self,
        args: &DlpProjectsLocationsContentInspectArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2InspectContentResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_content_inspect_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_content_inspect_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects locations content reidentify.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2ReidentifyContentResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_projects_locations_content_reidentify(
        &self,
        args: &DlpProjectsLocationsContentReidentifyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2ReidentifyContentResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_content_reidentify_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_content_reidentify_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects locations deidentify templates create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2DeidentifyTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_projects_locations_deidentify_templates_create(
        &self,
        args: &DlpProjectsLocationsDeidentifyTemplatesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2DeidentifyTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_deidentify_templates_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_deidentify_templates_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects locations deidentify templates delete.
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
    pub fn dlp_projects_locations_deidentify_templates_delete(
        &self,
        args: &DlpProjectsLocationsDeidentifyTemplatesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_deidentify_templates_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_deidentify_templates_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects locations deidentify templates patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2DeidentifyTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_projects_locations_deidentify_templates_patch(
        &self,
        args: &DlpProjectsLocationsDeidentifyTemplatesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2DeidentifyTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_deidentify_templates_patch_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_deidentify_templates_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects locations discovery configs create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2DiscoveryConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_projects_locations_discovery_configs_create(
        &self,
        args: &DlpProjectsLocationsDiscoveryConfigsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2DiscoveryConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_discovery_configs_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_discovery_configs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects locations discovery configs delete.
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
    pub fn dlp_projects_locations_discovery_configs_delete(
        &self,
        args: &DlpProjectsLocationsDiscoveryConfigsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_discovery_configs_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_discovery_configs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects locations discovery configs patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2DiscoveryConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_projects_locations_discovery_configs_patch(
        &self,
        args: &DlpProjectsLocationsDiscoveryConfigsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2DiscoveryConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_discovery_configs_patch_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_discovery_configs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects locations dlp jobs cancel.
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
    pub fn dlp_projects_locations_dlp_jobs_cancel(
        &self,
        args: &DlpProjectsLocationsDlpJobsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_dlp_jobs_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_dlp_jobs_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects locations dlp jobs create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2DlpJob result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_projects_locations_dlp_jobs_create(
        &self,
        args: &DlpProjectsLocationsDlpJobsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2DlpJob, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_dlp_jobs_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_dlp_jobs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects locations dlp jobs delete.
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
    pub fn dlp_projects_locations_dlp_jobs_delete(
        &self,
        args: &DlpProjectsLocationsDlpJobsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_dlp_jobs_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_dlp_jobs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects locations dlp jobs finish.
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
    pub fn dlp_projects_locations_dlp_jobs_finish(
        &self,
        args: &DlpProjectsLocationsDlpJobsFinishArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_dlp_jobs_finish_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_dlp_jobs_finish_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects locations dlp jobs hybrid inspect.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2HybridInspectResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_projects_locations_dlp_jobs_hybrid_inspect(
        &self,
        args: &DlpProjectsLocationsDlpJobsHybridInspectArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2HybridInspectResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_dlp_jobs_hybrid_inspect_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_dlp_jobs_hybrid_inspect_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects locations file store data profiles delete.
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
    pub fn dlp_projects_locations_file_store_data_profiles_delete(
        &self,
        args: &DlpProjectsLocationsFileStoreDataProfilesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_file_store_data_profiles_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_file_store_data_profiles_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects locations image redact.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2RedactImageResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_projects_locations_image_redact(
        &self,
        args: &DlpProjectsLocationsImageRedactArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2RedactImageResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_image_redact_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_image_redact_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects locations inspect templates create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2InspectTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_projects_locations_inspect_templates_create(
        &self,
        args: &DlpProjectsLocationsInspectTemplatesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2InspectTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_inspect_templates_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_inspect_templates_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects locations inspect templates delete.
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
    pub fn dlp_projects_locations_inspect_templates_delete(
        &self,
        args: &DlpProjectsLocationsInspectTemplatesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_inspect_templates_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_inspect_templates_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects locations inspect templates patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2InspectTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_projects_locations_inspect_templates_patch(
        &self,
        args: &DlpProjectsLocationsInspectTemplatesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2InspectTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_inspect_templates_patch_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_inspect_templates_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects locations job triggers activate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2DlpJob result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_projects_locations_job_triggers_activate(
        &self,
        args: &DlpProjectsLocationsJobTriggersActivateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2DlpJob, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_job_triggers_activate_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_job_triggers_activate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects locations job triggers create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2JobTrigger result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_projects_locations_job_triggers_create(
        &self,
        args: &DlpProjectsLocationsJobTriggersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2JobTrigger, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_job_triggers_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_job_triggers_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects locations job triggers delete.
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
    pub fn dlp_projects_locations_job_triggers_delete(
        &self,
        args: &DlpProjectsLocationsJobTriggersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_job_triggers_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_job_triggers_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects locations job triggers hybrid inspect.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2HybridInspectResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_projects_locations_job_triggers_hybrid_inspect(
        &self,
        args: &DlpProjectsLocationsJobTriggersHybridInspectArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2HybridInspectResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_job_triggers_hybrid_inspect_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_job_triggers_hybrid_inspect_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects locations job triggers patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2JobTrigger result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_projects_locations_job_triggers_patch(
        &self,
        args: &DlpProjectsLocationsJobTriggersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2JobTrigger, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_job_triggers_patch_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_job_triggers_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects locations stored info types create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2StoredInfoType result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_projects_locations_stored_info_types_create(
        &self,
        args: &DlpProjectsLocationsStoredInfoTypesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2StoredInfoType, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_stored_info_types_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_stored_info_types_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects locations stored info types delete.
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
    pub fn dlp_projects_locations_stored_info_types_delete(
        &self,
        args: &DlpProjectsLocationsStoredInfoTypesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_stored_info_types_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_stored_info_types_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects locations stored info types patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2StoredInfoType result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_projects_locations_stored_info_types_patch(
        &self,
        args: &DlpProjectsLocationsStoredInfoTypesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2StoredInfoType, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_stored_info_types_patch_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_stored_info_types_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects locations table data profiles delete.
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
    pub fn dlp_projects_locations_table_data_profiles_delete(
        &self,
        args: &DlpProjectsLocationsTableDataProfilesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_table_data_profiles_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_table_data_profiles_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects stored info types create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2StoredInfoType result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_projects_stored_info_types_create(
        &self,
        args: &DlpProjectsStoredInfoTypesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2StoredInfoType, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_stored_info_types_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_stored_info_types_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects stored info types delete.
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
    pub fn dlp_projects_stored_info_types_delete(
        &self,
        args: &DlpProjectsStoredInfoTypesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_stored_info_types_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_stored_info_types_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects stored info types patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2StoredInfoType result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dlp_projects_stored_info_types_patch(
        &self,
        args: &DlpProjectsStoredInfoTypesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2StoredInfoType, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_stored_info_types_patch_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_stored_info_types_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
