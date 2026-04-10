//! HealthcareProvider - State-aware healthcare API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       healthcare API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::healthcare::{
    healthcare_projects_locations_datasets_create_builder, healthcare_projects_locations_datasets_create_task,
    healthcare_projects_locations_datasets_deidentify_builder, healthcare_projects_locations_datasets_deidentify_task,
    healthcare_projects_locations_datasets_delete_builder, healthcare_projects_locations_datasets_delete_task,
    healthcare_projects_locations_datasets_patch_builder, healthcare_projects_locations_datasets_patch_task,
    healthcare_projects_locations_datasets_set_iam_policy_builder, healthcare_projects_locations_datasets_set_iam_policy_task,
    healthcare_projects_locations_datasets_test_iam_permissions_builder, healthcare_projects_locations_datasets_test_iam_permissions_task,
    healthcare_projects_locations_datasets_consent_stores_check_data_access_builder, healthcare_projects_locations_datasets_consent_stores_check_data_access_task,
    healthcare_projects_locations_datasets_consent_stores_create_builder, healthcare_projects_locations_datasets_consent_stores_create_task,
    healthcare_projects_locations_datasets_consent_stores_delete_builder, healthcare_projects_locations_datasets_consent_stores_delete_task,
    healthcare_projects_locations_datasets_consent_stores_evaluate_user_consents_builder, healthcare_projects_locations_datasets_consent_stores_evaluate_user_consents_task,
    healthcare_projects_locations_datasets_consent_stores_patch_builder, healthcare_projects_locations_datasets_consent_stores_patch_task,
    healthcare_projects_locations_datasets_consent_stores_query_accessible_data_builder, healthcare_projects_locations_datasets_consent_stores_query_accessible_data_task,
    healthcare_projects_locations_datasets_consent_stores_set_iam_policy_builder, healthcare_projects_locations_datasets_consent_stores_set_iam_policy_task,
    healthcare_projects_locations_datasets_consent_stores_test_iam_permissions_builder, healthcare_projects_locations_datasets_consent_stores_test_iam_permissions_task,
    healthcare_projects_locations_datasets_consent_stores_attribute_definitions_create_builder, healthcare_projects_locations_datasets_consent_stores_attribute_definitions_create_task,
    healthcare_projects_locations_datasets_consent_stores_attribute_definitions_delete_builder, healthcare_projects_locations_datasets_consent_stores_attribute_definitions_delete_task,
    healthcare_projects_locations_datasets_consent_stores_attribute_definitions_patch_builder, healthcare_projects_locations_datasets_consent_stores_attribute_definitions_patch_task,
    healthcare_projects_locations_datasets_consent_stores_consent_artifacts_create_builder, healthcare_projects_locations_datasets_consent_stores_consent_artifacts_create_task,
    healthcare_projects_locations_datasets_consent_stores_consent_artifacts_delete_builder, healthcare_projects_locations_datasets_consent_stores_consent_artifacts_delete_task,
    healthcare_projects_locations_datasets_consent_stores_consents_activate_builder, healthcare_projects_locations_datasets_consent_stores_consents_activate_task,
    healthcare_projects_locations_datasets_consent_stores_consents_create_builder, healthcare_projects_locations_datasets_consent_stores_consents_create_task,
    healthcare_projects_locations_datasets_consent_stores_consents_delete_builder, healthcare_projects_locations_datasets_consent_stores_consents_delete_task,
    healthcare_projects_locations_datasets_consent_stores_consents_delete_revision_builder, healthcare_projects_locations_datasets_consent_stores_consents_delete_revision_task,
    healthcare_projects_locations_datasets_consent_stores_consents_patch_builder, healthcare_projects_locations_datasets_consent_stores_consents_patch_task,
    healthcare_projects_locations_datasets_consent_stores_consents_reject_builder, healthcare_projects_locations_datasets_consent_stores_consents_reject_task,
    healthcare_projects_locations_datasets_consent_stores_consents_revoke_builder, healthcare_projects_locations_datasets_consent_stores_consents_revoke_task,
    healthcare_projects_locations_datasets_consent_stores_user_data_mappings_archive_builder, healthcare_projects_locations_datasets_consent_stores_user_data_mappings_archive_task,
    healthcare_projects_locations_datasets_consent_stores_user_data_mappings_create_builder, healthcare_projects_locations_datasets_consent_stores_user_data_mappings_create_task,
    healthcare_projects_locations_datasets_consent_stores_user_data_mappings_delete_builder, healthcare_projects_locations_datasets_consent_stores_user_data_mappings_delete_task,
    healthcare_projects_locations_datasets_consent_stores_user_data_mappings_patch_builder, healthcare_projects_locations_datasets_consent_stores_user_data_mappings_patch_task,
    healthcare_projects_locations_datasets_data_mapper_workspaces_set_iam_policy_builder, healthcare_projects_locations_datasets_data_mapper_workspaces_set_iam_policy_task,
    healthcare_projects_locations_datasets_data_mapper_workspaces_test_iam_permissions_builder, healthcare_projects_locations_datasets_data_mapper_workspaces_test_iam_permissions_task,
    healthcare_projects_locations_datasets_dicom_stores_create_builder, healthcare_projects_locations_datasets_dicom_stores_create_task,
    healthcare_projects_locations_datasets_dicom_stores_deidentify_builder, healthcare_projects_locations_datasets_dicom_stores_deidentify_task,
    healthcare_projects_locations_datasets_dicom_stores_delete_builder, healthcare_projects_locations_datasets_dicom_stores_delete_task,
    healthcare_projects_locations_datasets_dicom_stores_export_builder, healthcare_projects_locations_datasets_dicom_stores_export_task,
    healthcare_projects_locations_datasets_dicom_stores_import_builder, healthcare_projects_locations_datasets_dicom_stores_import_task,
    healthcare_projects_locations_datasets_dicom_stores_patch_builder, healthcare_projects_locations_datasets_dicom_stores_patch_task,
    healthcare_projects_locations_datasets_dicom_stores_set_blob_storage_settings_builder, healthcare_projects_locations_datasets_dicom_stores_set_blob_storage_settings_task,
    healthcare_projects_locations_datasets_dicom_stores_set_iam_policy_builder, healthcare_projects_locations_datasets_dicom_stores_set_iam_policy_task,
    healthcare_projects_locations_datasets_dicom_stores_store_instances_builder, healthcare_projects_locations_datasets_dicom_stores_store_instances_task,
    healthcare_projects_locations_datasets_dicom_stores_test_iam_permissions_builder, healthcare_projects_locations_datasets_dicom_stores_test_iam_permissions_task,
    healthcare_projects_locations_datasets_dicom_stores_dicom_web_studies_set_blob_storage_settings_builder, healthcare_projects_locations_datasets_dicom_stores_dicom_web_studies_set_blob_storage_settings_task,
    healthcare_projects_locations_datasets_dicom_stores_studies_delete_builder, healthcare_projects_locations_datasets_dicom_stores_studies_delete_task,
    healthcare_projects_locations_datasets_dicom_stores_studies_store_instances_builder, healthcare_projects_locations_datasets_dicom_stores_studies_store_instances_task,
    healthcare_projects_locations_datasets_dicom_stores_studies_series_delete_builder, healthcare_projects_locations_datasets_dicom_stores_studies_series_delete_task,
    healthcare_projects_locations_datasets_dicom_stores_studies_series_instances_delete_builder, healthcare_projects_locations_datasets_dicom_stores_studies_series_instances_delete_task,
    healthcare_projects_locations_datasets_fhir_stores_apply_admin_consents_builder, healthcare_projects_locations_datasets_fhir_stores_apply_admin_consents_task,
    healthcare_projects_locations_datasets_fhir_stores_apply_consents_builder, healthcare_projects_locations_datasets_fhir_stores_apply_consents_task,
    healthcare_projects_locations_datasets_fhir_stores_bulk_delete_builder, healthcare_projects_locations_datasets_fhir_stores_bulk_delete_task,
    healthcare_projects_locations_datasets_fhir_stores_create_builder, healthcare_projects_locations_datasets_fhir_stores_create_task,
    healthcare_projects_locations_datasets_fhir_stores_deidentify_builder, healthcare_projects_locations_datasets_fhir_stores_deidentify_task,
    healthcare_projects_locations_datasets_fhir_stores_delete_builder, healthcare_projects_locations_datasets_fhir_stores_delete_task,
    healthcare_projects_locations_datasets_fhir_stores_export_builder, healthcare_projects_locations_datasets_fhir_stores_export_task,
    healthcare_projects_locations_datasets_fhir_stores_import_builder, healthcare_projects_locations_datasets_fhir_stores_import_task,
    healthcare_projects_locations_datasets_fhir_stores_patch_builder, healthcare_projects_locations_datasets_fhir_stores_patch_task,
    healthcare_projects_locations_datasets_fhir_stores_rollback_builder, healthcare_projects_locations_datasets_fhir_stores_rollback_task,
    healthcare_projects_locations_datasets_fhir_stores_set_iam_policy_builder, healthcare_projects_locations_datasets_fhir_stores_set_iam_policy_task,
    healthcare_projects_locations_datasets_fhir_stores_test_iam_permissions_builder, healthcare_projects_locations_datasets_fhir_stores_test_iam_permissions_task,
    healthcare_projects_locations_datasets_fhir_stores_fhir_binary_create_builder, healthcare_projects_locations_datasets_fhir_stores_fhir_binary_create_task,
    healthcare_projects_locations_datasets_fhir_stores_fhir_binary_update_builder, healthcare_projects_locations_datasets_fhir_stores_fhir_binary_update_task,
    healthcare_projects_locations_datasets_fhir_stores_fhir_resource_purge_builder, healthcare_projects_locations_datasets_fhir_stores_fhir_resource_purge_task,
    healthcare_projects_locations_datasets_fhir_stores_fhir_resource_validate_builder, healthcare_projects_locations_datasets_fhir_stores_fhir_resource_validate_task,
    healthcare_projects_locations_datasets_fhir_stores_fhir_conditional_delete_builder, healthcare_projects_locations_datasets_fhir_stores_fhir_conditional_delete_task,
    healthcare_projects_locations_datasets_fhir_stores_fhir_conditional_patch_builder, healthcare_projects_locations_datasets_fhir_stores_fhir_conditional_patch_task,
    healthcare_projects_locations_datasets_fhir_stores_fhir_conditional_update_builder, healthcare_projects_locations_datasets_fhir_stores_fhir_conditional_update_task,
    healthcare_projects_locations_datasets_fhir_stores_fhir_create_builder, healthcare_projects_locations_datasets_fhir_stores_fhir_create_task,
    healthcare_projects_locations_datasets_fhir_stores_fhir_delete_builder, healthcare_projects_locations_datasets_fhir_stores_fhir_delete_task,
    healthcare_projects_locations_datasets_fhir_stores_fhir_execute_bundle_builder, healthcare_projects_locations_datasets_fhir_stores_fhir_execute_bundle_task,
    healthcare_projects_locations_datasets_fhir_stores_fhir_patch_builder, healthcare_projects_locations_datasets_fhir_stores_fhir_patch_task,
    healthcare_projects_locations_datasets_fhir_stores_fhir_search_builder, healthcare_projects_locations_datasets_fhir_stores_fhir_search_task,
    healthcare_projects_locations_datasets_fhir_stores_fhir_search_type_builder, healthcare_projects_locations_datasets_fhir_stores_fhir_search_type_task,
    healthcare_projects_locations_datasets_fhir_stores_fhir_update_builder, healthcare_projects_locations_datasets_fhir_stores_fhir_update_task,
    healthcare_projects_locations_datasets_fhir_stores_operations_delete_fhir_operation_builder, healthcare_projects_locations_datasets_fhir_stores_operations_delete_fhir_operation_task,
    healthcare_projects_locations_datasets_hl7_v2_stores_create_builder, healthcare_projects_locations_datasets_hl7_v2_stores_create_task,
    healthcare_projects_locations_datasets_hl7_v2_stores_delete_builder, healthcare_projects_locations_datasets_hl7_v2_stores_delete_task,
    healthcare_projects_locations_datasets_hl7_v2_stores_export_builder, healthcare_projects_locations_datasets_hl7_v2_stores_export_task,
    healthcare_projects_locations_datasets_hl7_v2_stores_import_builder, healthcare_projects_locations_datasets_hl7_v2_stores_import_task,
    healthcare_projects_locations_datasets_hl7_v2_stores_patch_builder, healthcare_projects_locations_datasets_hl7_v2_stores_patch_task,
    healthcare_projects_locations_datasets_hl7_v2_stores_rollback_builder, healthcare_projects_locations_datasets_hl7_v2_stores_rollback_task,
    healthcare_projects_locations_datasets_hl7_v2_stores_set_iam_policy_builder, healthcare_projects_locations_datasets_hl7_v2_stores_set_iam_policy_task,
    healthcare_projects_locations_datasets_hl7_v2_stores_test_iam_permissions_builder, healthcare_projects_locations_datasets_hl7_v2_stores_test_iam_permissions_task,
    healthcare_projects_locations_datasets_hl7_v2_stores_messages_create_builder, healthcare_projects_locations_datasets_hl7_v2_stores_messages_create_task,
    healthcare_projects_locations_datasets_hl7_v2_stores_messages_delete_builder, healthcare_projects_locations_datasets_hl7_v2_stores_messages_delete_task,
    healthcare_projects_locations_datasets_hl7_v2_stores_messages_ingest_builder, healthcare_projects_locations_datasets_hl7_v2_stores_messages_ingest_task,
    healthcare_projects_locations_datasets_hl7_v2_stores_messages_patch_builder, healthcare_projects_locations_datasets_hl7_v2_stores_messages_patch_task,
    healthcare_projects_locations_datasets_operations_cancel_builder, healthcare_projects_locations_datasets_operations_cancel_task,
    healthcare_projects_locations_services_nlp_analyze_entities_builder, healthcare_projects_locations_services_nlp_analyze_entities_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::healthcare::AnalyzeEntitiesResponse;
use crate::providers::gcp::clients::healthcare::ArchiveUserDataMappingResponse;
use crate::providers::gcp::clients::healthcare::AttributeDefinition;
use crate::providers::gcp::clients::healthcare::CheckDataAccessResponse;
use crate::providers::gcp::clients::healthcare::Consent;
use crate::providers::gcp::clients::healthcare::ConsentArtifact;
use crate::providers::gcp::clients::healthcare::ConsentStore;
use crate::providers::gcp::clients::healthcare::Dataset;
use crate::providers::gcp::clients::healthcare::DicomStore;
use crate::providers::gcp::clients::healthcare::Empty;
use crate::providers::gcp::clients::healthcare::EvaluateUserConsentsResponse;
use crate::providers::gcp::clients::healthcare::FhirStore;
use crate::providers::gcp::clients::healthcare::Hl7V2Store;
use crate::providers::gcp::clients::healthcare::HttpBody;
use crate::providers::gcp::clients::healthcare::IngestMessageResponse;
use crate::providers::gcp::clients::healthcare::Message;
use crate::providers::gcp::clients::healthcare::Operation;
use crate::providers::gcp::clients::healthcare::Policy;
use crate::providers::gcp::clients::healthcare::TestIamPermissionsResponse;
use crate::providers::gcp::clients::healthcare::UserDataMapping;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsConsentStoresAttributeDefinitionsCreateArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsConsentStoresAttributeDefinitionsDeleteArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsConsentStoresAttributeDefinitionsPatchArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsConsentStoresCheckDataAccessArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsConsentStoresConsentArtifactsCreateArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsConsentStoresConsentArtifactsDeleteArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsConsentStoresConsentsActivateArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsConsentStoresConsentsCreateArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsConsentStoresConsentsDeleteArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsConsentStoresConsentsDeleteRevisionArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsConsentStoresConsentsPatchArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsConsentStoresConsentsRejectArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsConsentStoresConsentsRevokeArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsConsentStoresCreateArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsConsentStoresDeleteArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsConsentStoresEvaluateUserConsentsArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsConsentStoresPatchArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsConsentStoresQueryAccessibleDataArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsConsentStoresSetIamPolicyArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsConsentStoresTestIamPermissionsArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsConsentStoresUserDataMappingsArchiveArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsConsentStoresUserDataMappingsCreateArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsConsentStoresUserDataMappingsDeleteArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsConsentStoresUserDataMappingsPatchArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsCreateArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsDataMapperWorkspacesSetIamPolicyArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsDataMapperWorkspacesTestIamPermissionsArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsDeidentifyArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsDeleteArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsDicomStoresCreateArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsDicomStoresDeidentifyArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsDicomStoresDeleteArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsDicomStoresDicomWebStudiesSetBlobStorageSettingsArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsDicomStoresExportArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsDicomStoresImportArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsDicomStoresPatchArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsDicomStoresSetBlobStorageSettingsArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsDicomStoresSetIamPolicyArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsDicomStoresStoreInstancesArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsDicomStoresStudiesDeleteArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsDicomStoresStudiesSeriesDeleteArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsDicomStoresStudiesSeriesInstancesDeleteArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsDicomStoresStudiesStoreInstancesArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsDicomStoresTestIamPermissionsArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsFhirStoresApplyAdminConsentsArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsFhirStoresApplyConsentsArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsFhirStoresBulkDeleteArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsFhirStoresCreateArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsFhirStoresDeidentifyArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsFhirStoresDeleteArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsFhirStoresExportArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsFhirStoresFhirBinaryCreateArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsFhirStoresFhirBinaryUpdateArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsFhirStoresFhirConditionalDeleteArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsFhirStoresFhirConditionalPatchArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsFhirStoresFhirConditionalUpdateArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsFhirStoresFhirCreateArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsFhirStoresFhirDeleteArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsFhirStoresFhirExecuteBundleArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsFhirStoresFhirPatchArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsFhirStoresFhirResourcePurgeArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsFhirStoresFhirResourceValidateArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsFhirStoresFhirSearchArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsFhirStoresFhirSearchTypeArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsFhirStoresFhirUpdateArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsFhirStoresImportArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsFhirStoresOperationsDeleteFhirOperationArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsFhirStoresPatchArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsFhirStoresRollbackArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsFhirStoresSetIamPolicyArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsFhirStoresTestIamPermissionsArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsHl7V2StoresCreateArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsHl7V2StoresDeleteArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsHl7V2StoresExportArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsHl7V2StoresImportArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsHl7V2StoresMessagesCreateArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsHl7V2StoresMessagesDeleteArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsHl7V2StoresMessagesIngestArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsHl7V2StoresMessagesPatchArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsHl7V2StoresPatchArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsHl7V2StoresRollbackArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsHl7V2StoresSetIamPolicyArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsHl7V2StoresTestIamPermissionsArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsOperationsCancelArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsPatchArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsSetIamPolicyArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsDatasetsTestIamPermissionsArgs;
use crate::providers::gcp::clients::healthcare::HealthcareProjectsLocationsServicesNlpAnalyzeEntitiesArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// HealthcareProvider with automatic state tracking.
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
/// let provider = HealthcareProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct HealthcareProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> HealthcareProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new HealthcareProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Healthcare projects locations datasets create.
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
    pub fn healthcare_projects_locations_datasets_create(
        &self,
        args: &HealthcareProjectsLocationsDatasetsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_create_builder(
            &self.http_client,
            &args.parent,
            &args.datasetId,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets deidentify.
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
    pub fn healthcare_projects_locations_datasets_deidentify(
        &self,
        args: &HealthcareProjectsLocationsDatasetsDeidentifyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_deidentify_builder(
            &self.http_client,
            &args.sourceDataset,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_deidentify_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets delete.
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
    pub fn healthcare_projects_locations_datasets_delete(
        &self,
        args: &HealthcareProjectsLocationsDatasetsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Dataset result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn healthcare_projects_locations_datasets_patch(
        &self,
        args: &HealthcareProjectsLocationsDatasetsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Dataset, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets set iam policy.
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
    pub fn healthcare_projects_locations_datasets_set_iam_policy(
        &self,
        args: &HealthcareProjectsLocationsDatasetsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets test iam permissions.
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
    pub fn healthcare_projects_locations_datasets_test_iam_permissions(
        &self,
        args: &HealthcareProjectsLocationsDatasetsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets consent stores check data access.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CheckDataAccessResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn healthcare_projects_locations_datasets_consent_stores_check_data_access(
        &self,
        args: &HealthcareProjectsLocationsDatasetsConsentStoresCheckDataAccessArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CheckDataAccessResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_consent_stores_check_data_access_builder(
            &self.http_client,
            &args.consentStore,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_consent_stores_check_data_access_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets consent stores create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ConsentStore result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn healthcare_projects_locations_datasets_consent_stores_create(
        &self,
        args: &HealthcareProjectsLocationsDatasetsConsentStoresCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ConsentStore, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_consent_stores_create_builder(
            &self.http_client,
            &args.parent,
            &args.consentStoreId,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_consent_stores_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets consent stores delete.
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
    pub fn healthcare_projects_locations_datasets_consent_stores_delete(
        &self,
        args: &HealthcareProjectsLocationsDatasetsConsentStoresDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_consent_stores_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_consent_stores_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets consent stores evaluate user consents.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EvaluateUserConsentsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn healthcare_projects_locations_datasets_consent_stores_evaluate_user_consents(
        &self,
        args: &HealthcareProjectsLocationsDatasetsConsentStoresEvaluateUserConsentsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EvaluateUserConsentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_consent_stores_evaluate_user_consents_builder(
            &self.http_client,
            &args.consentStore,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_consent_stores_evaluate_user_consents_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets consent stores patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ConsentStore result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn healthcare_projects_locations_datasets_consent_stores_patch(
        &self,
        args: &HealthcareProjectsLocationsDatasetsConsentStoresPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ConsentStore, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_consent_stores_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_consent_stores_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets consent stores query accessible data.
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
    pub fn healthcare_projects_locations_datasets_consent_stores_query_accessible_data(
        &self,
        args: &HealthcareProjectsLocationsDatasetsConsentStoresQueryAccessibleDataArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_consent_stores_query_accessible_data_builder(
            &self.http_client,
            &args.consentStore,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_consent_stores_query_accessible_data_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets consent stores set iam policy.
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
    pub fn healthcare_projects_locations_datasets_consent_stores_set_iam_policy(
        &self,
        args: &HealthcareProjectsLocationsDatasetsConsentStoresSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_consent_stores_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_consent_stores_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets consent stores test iam permissions.
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
    pub fn healthcare_projects_locations_datasets_consent_stores_test_iam_permissions(
        &self,
        args: &HealthcareProjectsLocationsDatasetsConsentStoresTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_consent_stores_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_consent_stores_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets consent stores attribute definitions create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AttributeDefinition result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn healthcare_projects_locations_datasets_consent_stores_attribute_definitions_create(
        &self,
        args: &HealthcareProjectsLocationsDatasetsConsentStoresAttributeDefinitionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AttributeDefinition, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_consent_stores_attribute_definitions_create_builder(
            &self.http_client,
            &args.parent,
            &args.attributeDefinitionId,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_consent_stores_attribute_definitions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets consent stores attribute definitions delete.
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
    pub fn healthcare_projects_locations_datasets_consent_stores_attribute_definitions_delete(
        &self,
        args: &HealthcareProjectsLocationsDatasetsConsentStoresAttributeDefinitionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_consent_stores_attribute_definitions_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_consent_stores_attribute_definitions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets consent stores attribute definitions patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AttributeDefinition result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn healthcare_projects_locations_datasets_consent_stores_attribute_definitions_patch(
        &self,
        args: &HealthcareProjectsLocationsDatasetsConsentStoresAttributeDefinitionsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AttributeDefinition, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_consent_stores_attribute_definitions_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_consent_stores_attribute_definitions_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets consent stores consent artifacts create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ConsentArtifact result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn healthcare_projects_locations_datasets_consent_stores_consent_artifacts_create(
        &self,
        args: &HealthcareProjectsLocationsDatasetsConsentStoresConsentArtifactsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ConsentArtifact, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_consent_stores_consent_artifacts_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_consent_stores_consent_artifacts_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets consent stores consent artifacts delete.
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
    pub fn healthcare_projects_locations_datasets_consent_stores_consent_artifacts_delete(
        &self,
        args: &HealthcareProjectsLocationsDatasetsConsentStoresConsentArtifactsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_consent_stores_consent_artifacts_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_consent_stores_consent_artifacts_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets consent stores consents activate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Consent result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn healthcare_projects_locations_datasets_consent_stores_consents_activate(
        &self,
        args: &HealthcareProjectsLocationsDatasetsConsentStoresConsentsActivateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Consent, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_consent_stores_consents_activate_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_consent_stores_consents_activate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets consent stores consents create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Consent result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn healthcare_projects_locations_datasets_consent_stores_consents_create(
        &self,
        args: &HealthcareProjectsLocationsDatasetsConsentStoresConsentsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Consent, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_consent_stores_consents_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_consent_stores_consents_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets consent stores consents delete.
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
    pub fn healthcare_projects_locations_datasets_consent_stores_consents_delete(
        &self,
        args: &HealthcareProjectsLocationsDatasetsConsentStoresConsentsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_consent_stores_consents_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_consent_stores_consents_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets consent stores consents delete revision.
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
    pub fn healthcare_projects_locations_datasets_consent_stores_consents_delete_revision(
        &self,
        args: &HealthcareProjectsLocationsDatasetsConsentStoresConsentsDeleteRevisionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_consent_stores_consents_delete_revision_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_consent_stores_consents_delete_revision_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets consent stores consents patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Consent result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn healthcare_projects_locations_datasets_consent_stores_consents_patch(
        &self,
        args: &HealthcareProjectsLocationsDatasetsConsentStoresConsentsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Consent, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_consent_stores_consents_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_consent_stores_consents_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets consent stores consents reject.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Consent result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn healthcare_projects_locations_datasets_consent_stores_consents_reject(
        &self,
        args: &HealthcareProjectsLocationsDatasetsConsentStoresConsentsRejectArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Consent, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_consent_stores_consents_reject_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_consent_stores_consents_reject_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets consent stores consents revoke.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Consent result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn healthcare_projects_locations_datasets_consent_stores_consents_revoke(
        &self,
        args: &HealthcareProjectsLocationsDatasetsConsentStoresConsentsRevokeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Consent, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_consent_stores_consents_revoke_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_consent_stores_consents_revoke_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets consent stores user data mappings archive.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ArchiveUserDataMappingResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn healthcare_projects_locations_datasets_consent_stores_user_data_mappings_archive(
        &self,
        args: &HealthcareProjectsLocationsDatasetsConsentStoresUserDataMappingsArchiveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ArchiveUserDataMappingResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_consent_stores_user_data_mappings_archive_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_consent_stores_user_data_mappings_archive_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets consent stores user data mappings create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UserDataMapping result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn healthcare_projects_locations_datasets_consent_stores_user_data_mappings_create(
        &self,
        args: &HealthcareProjectsLocationsDatasetsConsentStoresUserDataMappingsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UserDataMapping, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_consent_stores_user_data_mappings_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_consent_stores_user_data_mappings_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets consent stores user data mappings delete.
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
    pub fn healthcare_projects_locations_datasets_consent_stores_user_data_mappings_delete(
        &self,
        args: &HealthcareProjectsLocationsDatasetsConsentStoresUserDataMappingsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_consent_stores_user_data_mappings_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_consent_stores_user_data_mappings_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets consent stores user data mappings patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UserDataMapping result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn healthcare_projects_locations_datasets_consent_stores_user_data_mappings_patch(
        &self,
        args: &HealthcareProjectsLocationsDatasetsConsentStoresUserDataMappingsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UserDataMapping, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_consent_stores_user_data_mappings_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_consent_stores_user_data_mappings_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets data mapper workspaces set iam policy.
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
    pub fn healthcare_projects_locations_datasets_data_mapper_workspaces_set_iam_policy(
        &self,
        args: &HealthcareProjectsLocationsDatasetsDataMapperWorkspacesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_data_mapper_workspaces_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_data_mapper_workspaces_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets data mapper workspaces test iam permissions.
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
    pub fn healthcare_projects_locations_datasets_data_mapper_workspaces_test_iam_permissions(
        &self,
        args: &HealthcareProjectsLocationsDatasetsDataMapperWorkspacesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_data_mapper_workspaces_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_data_mapper_workspaces_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets dicom stores create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DicomStore result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn healthcare_projects_locations_datasets_dicom_stores_create(
        &self,
        args: &HealthcareProjectsLocationsDatasetsDicomStoresCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DicomStore, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_dicom_stores_create_builder(
            &self.http_client,
            &args.parent,
            &args.dicomStoreId,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_dicom_stores_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets dicom stores deidentify.
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
    pub fn healthcare_projects_locations_datasets_dicom_stores_deidentify(
        &self,
        args: &HealthcareProjectsLocationsDatasetsDicomStoresDeidentifyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_dicom_stores_deidentify_builder(
            &self.http_client,
            &args.sourceStore,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_dicom_stores_deidentify_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets dicom stores delete.
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
    pub fn healthcare_projects_locations_datasets_dicom_stores_delete(
        &self,
        args: &HealthcareProjectsLocationsDatasetsDicomStoresDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_dicom_stores_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_dicom_stores_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets dicom stores export.
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
    pub fn healthcare_projects_locations_datasets_dicom_stores_export(
        &self,
        args: &HealthcareProjectsLocationsDatasetsDicomStoresExportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_dicom_stores_export_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_dicom_stores_export_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets dicom stores import.
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
    pub fn healthcare_projects_locations_datasets_dicom_stores_import(
        &self,
        args: &HealthcareProjectsLocationsDatasetsDicomStoresImportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_dicom_stores_import_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_dicom_stores_import_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets dicom stores patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DicomStore result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn healthcare_projects_locations_datasets_dicom_stores_patch(
        &self,
        args: &HealthcareProjectsLocationsDatasetsDicomStoresPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DicomStore, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_dicom_stores_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_dicom_stores_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets dicom stores set blob storage settings.
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
    pub fn healthcare_projects_locations_datasets_dicom_stores_set_blob_storage_settings(
        &self,
        args: &HealthcareProjectsLocationsDatasetsDicomStoresSetBlobStorageSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_dicom_stores_set_blob_storage_settings_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_dicom_stores_set_blob_storage_settings_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets dicom stores set iam policy.
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
    pub fn healthcare_projects_locations_datasets_dicom_stores_set_iam_policy(
        &self,
        args: &HealthcareProjectsLocationsDatasetsDicomStoresSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_dicom_stores_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_dicom_stores_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets dicom stores store instances.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HttpBody result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn healthcare_projects_locations_datasets_dicom_stores_store_instances(
        &self,
        args: &HealthcareProjectsLocationsDatasetsDicomStoresStoreInstancesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_dicom_stores_store_instances_builder(
            &self.http_client,
            &args.parent,
            &args.dicomWebPath,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_dicom_stores_store_instances_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets dicom stores test iam permissions.
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
    pub fn healthcare_projects_locations_datasets_dicom_stores_test_iam_permissions(
        &self,
        args: &HealthcareProjectsLocationsDatasetsDicomStoresTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_dicom_stores_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_dicom_stores_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets dicom stores dicom web studies set blob storage settings.
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
    pub fn healthcare_projects_locations_datasets_dicom_stores_dicom_web_studies_set_blob_storage_settings(
        &self,
        args: &HealthcareProjectsLocationsDatasetsDicomStoresDicomWebStudiesSetBlobStorageSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_dicom_stores_dicom_web_studies_set_blob_storage_settings_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_dicom_stores_dicom_web_studies_set_blob_storage_settings_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets dicom stores studies delete.
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
    pub fn healthcare_projects_locations_datasets_dicom_stores_studies_delete(
        &self,
        args: &HealthcareProjectsLocationsDatasetsDicomStoresStudiesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_dicom_stores_studies_delete_builder(
            &self.http_client,
            &args.parent,
            &args.dicomWebPath,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_dicom_stores_studies_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets dicom stores studies store instances.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HttpBody result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn healthcare_projects_locations_datasets_dicom_stores_studies_store_instances(
        &self,
        args: &HealthcareProjectsLocationsDatasetsDicomStoresStudiesStoreInstancesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_dicom_stores_studies_store_instances_builder(
            &self.http_client,
            &args.parent,
            &args.dicomWebPath,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_dicom_stores_studies_store_instances_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets dicom stores studies series delete.
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
    pub fn healthcare_projects_locations_datasets_dicom_stores_studies_series_delete(
        &self,
        args: &HealthcareProjectsLocationsDatasetsDicomStoresStudiesSeriesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_dicom_stores_studies_series_delete_builder(
            &self.http_client,
            &args.parent,
            &args.dicomWebPath,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_dicom_stores_studies_series_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets dicom stores studies series instances delete.
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
    pub fn healthcare_projects_locations_datasets_dicom_stores_studies_series_instances_delete(
        &self,
        args: &HealthcareProjectsLocationsDatasetsDicomStoresStudiesSeriesInstancesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_dicom_stores_studies_series_instances_delete_builder(
            &self.http_client,
            &args.parent,
            &args.dicomWebPath,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_dicom_stores_studies_series_instances_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets fhir stores apply admin consents.
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
    pub fn healthcare_projects_locations_datasets_fhir_stores_apply_admin_consents(
        &self,
        args: &HealthcareProjectsLocationsDatasetsFhirStoresApplyAdminConsentsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_fhir_stores_apply_admin_consents_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_fhir_stores_apply_admin_consents_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets fhir stores apply consents.
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
    pub fn healthcare_projects_locations_datasets_fhir_stores_apply_consents(
        &self,
        args: &HealthcareProjectsLocationsDatasetsFhirStoresApplyConsentsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_fhir_stores_apply_consents_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_fhir_stores_apply_consents_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets fhir stores bulk delete.
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
    pub fn healthcare_projects_locations_datasets_fhir_stores_bulk_delete(
        &self,
        args: &HealthcareProjectsLocationsDatasetsFhirStoresBulkDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_fhir_stores_bulk_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_fhir_stores_bulk_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets fhir stores create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FhirStore result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn healthcare_projects_locations_datasets_fhir_stores_create(
        &self,
        args: &HealthcareProjectsLocationsDatasetsFhirStoresCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FhirStore, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_fhir_stores_create_builder(
            &self.http_client,
            &args.parent,
            &args.fhirStoreId,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_fhir_stores_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets fhir stores deidentify.
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
    pub fn healthcare_projects_locations_datasets_fhir_stores_deidentify(
        &self,
        args: &HealthcareProjectsLocationsDatasetsFhirStoresDeidentifyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_fhir_stores_deidentify_builder(
            &self.http_client,
            &args.sourceStore,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_fhir_stores_deidentify_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets fhir stores delete.
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
    pub fn healthcare_projects_locations_datasets_fhir_stores_delete(
        &self,
        args: &HealthcareProjectsLocationsDatasetsFhirStoresDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_fhir_stores_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_fhir_stores_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets fhir stores export.
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
    pub fn healthcare_projects_locations_datasets_fhir_stores_export(
        &self,
        args: &HealthcareProjectsLocationsDatasetsFhirStoresExportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_fhir_stores_export_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_fhir_stores_export_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets fhir stores import.
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
    pub fn healthcare_projects_locations_datasets_fhir_stores_import(
        &self,
        args: &HealthcareProjectsLocationsDatasetsFhirStoresImportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_fhir_stores_import_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_fhir_stores_import_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets fhir stores patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FhirStore result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn healthcare_projects_locations_datasets_fhir_stores_patch(
        &self,
        args: &HealthcareProjectsLocationsDatasetsFhirStoresPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FhirStore, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_fhir_stores_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_fhir_stores_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets fhir stores rollback.
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
    pub fn healthcare_projects_locations_datasets_fhir_stores_rollback(
        &self,
        args: &HealthcareProjectsLocationsDatasetsFhirStoresRollbackArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_fhir_stores_rollback_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_fhir_stores_rollback_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets fhir stores set iam policy.
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
    pub fn healthcare_projects_locations_datasets_fhir_stores_set_iam_policy(
        &self,
        args: &HealthcareProjectsLocationsDatasetsFhirStoresSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_fhir_stores_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_fhir_stores_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets fhir stores test iam permissions.
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
    pub fn healthcare_projects_locations_datasets_fhir_stores_test_iam_permissions(
        &self,
        args: &HealthcareProjectsLocationsDatasetsFhirStoresTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_fhir_stores_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_fhir_stores_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets fhir stores fhir binary create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HttpBody result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn healthcare_projects_locations_datasets_fhir_stores_fhir_binary_create(
        &self,
        args: &HealthcareProjectsLocationsDatasetsFhirStoresFhirBinaryCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_fhir_stores_fhir_binary_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_fhir_stores_fhir_binary_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets fhir stores fhir binary update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HttpBody result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn healthcare_projects_locations_datasets_fhir_stores_fhir_binary_update(
        &self,
        args: &HealthcareProjectsLocationsDatasetsFhirStoresFhirBinaryUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_fhir_stores_fhir_binary_update_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_fhir_stores_fhir_binary_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets fhir stores fhir resource purge.
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
    pub fn healthcare_projects_locations_datasets_fhir_stores_fhir_resource_purge(
        &self,
        args: &HealthcareProjectsLocationsDatasetsFhirStoresFhirResourcePurgeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_fhir_stores_fhir_resource_purge_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_fhir_stores_fhir_resource_purge_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets fhir stores fhir resource validate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HttpBody result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn healthcare_projects_locations_datasets_fhir_stores_fhir_resource_validate(
        &self,
        args: &HealthcareProjectsLocationsDatasetsFhirStoresFhirResourceValidateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_fhir_stores_fhir_resource_validate_builder(
            &self.http_client,
            &args.parent,
            &args.type,
            &args.profile,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_fhir_stores_fhir_resource_validate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets fhir stores fhir conditional delete.
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
    pub fn healthcare_projects_locations_datasets_fhir_stores_fhir_conditional_delete(
        &self,
        args: &HealthcareProjectsLocationsDatasetsFhirStoresFhirConditionalDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_fhir_stores_fhir_conditional_delete_builder(
            &self.http_client,
            &args.parent,
            &args.type,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_fhir_stores_fhir_conditional_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets fhir stores fhir conditional patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HttpBody result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn healthcare_projects_locations_datasets_fhir_stores_fhir_conditional_patch(
        &self,
        args: &HealthcareProjectsLocationsDatasetsFhirStoresFhirConditionalPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_fhir_stores_fhir_conditional_patch_builder(
            &self.http_client,
            &args.parent,
            &args.type,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_fhir_stores_fhir_conditional_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets fhir stores fhir conditional update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HttpBody result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn healthcare_projects_locations_datasets_fhir_stores_fhir_conditional_update(
        &self,
        args: &HealthcareProjectsLocationsDatasetsFhirStoresFhirConditionalUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_fhir_stores_fhir_conditional_update_builder(
            &self.http_client,
            &args.parent,
            &args.type,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_fhir_stores_fhir_conditional_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets fhir stores fhir create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HttpBody result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn healthcare_projects_locations_datasets_fhir_stores_fhir_create(
        &self,
        args: &HealthcareProjectsLocationsDatasetsFhirStoresFhirCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_fhir_stores_fhir_create_builder(
            &self.http_client,
            &args.parent,
            &args.type,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_fhir_stores_fhir_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets fhir stores fhir delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HttpBody result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn healthcare_projects_locations_datasets_fhir_stores_fhir_delete(
        &self,
        args: &HealthcareProjectsLocationsDatasetsFhirStoresFhirDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_fhir_stores_fhir_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_fhir_stores_fhir_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets fhir stores fhir execute bundle.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HttpBody result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn healthcare_projects_locations_datasets_fhir_stores_fhir_execute_bundle(
        &self,
        args: &HealthcareProjectsLocationsDatasetsFhirStoresFhirExecuteBundleArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_fhir_stores_fhir_execute_bundle_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_fhir_stores_fhir_execute_bundle_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets fhir stores fhir patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HttpBody result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn healthcare_projects_locations_datasets_fhir_stores_fhir_patch(
        &self,
        args: &HealthcareProjectsLocationsDatasetsFhirStoresFhirPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_fhir_stores_fhir_patch_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_fhir_stores_fhir_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets fhir stores fhir search.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HttpBody result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn healthcare_projects_locations_datasets_fhir_stores_fhir_search(
        &self,
        args: &HealthcareProjectsLocationsDatasetsFhirStoresFhirSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_fhir_stores_fhir_search_builder(
            &self.http_client,
            &args.parent,
            &args.resourceType,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_fhir_stores_fhir_search_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets fhir stores fhir search type.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HttpBody result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn healthcare_projects_locations_datasets_fhir_stores_fhir_search_type(
        &self,
        args: &HealthcareProjectsLocationsDatasetsFhirStoresFhirSearchTypeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_fhir_stores_fhir_search_type_builder(
            &self.http_client,
            &args.parent,
            &args.resourceType,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_fhir_stores_fhir_search_type_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets fhir stores fhir update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HttpBody result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn healthcare_projects_locations_datasets_fhir_stores_fhir_update(
        &self,
        args: &HealthcareProjectsLocationsDatasetsFhirStoresFhirUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_fhir_stores_fhir_update_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_fhir_stores_fhir_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets fhir stores operations delete fhir operation.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HttpBody result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn healthcare_projects_locations_datasets_fhir_stores_operations_delete_fhir_operation(
        &self,
        args: &HealthcareProjectsLocationsDatasetsFhirStoresOperationsDeleteFhirOperationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_fhir_stores_operations_delete_fhir_operation_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_fhir_stores_operations_delete_fhir_operation_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets hl7 v2 stores create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Hl7V2Store result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn healthcare_projects_locations_datasets_hl7_v2_stores_create(
        &self,
        args: &HealthcareProjectsLocationsDatasetsHl7V2StoresCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Hl7V2Store, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_hl7_v2_stores_create_builder(
            &self.http_client,
            &args.parent,
            &args.hl7V2StoreId,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_hl7_v2_stores_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets hl7 v2 stores delete.
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
    pub fn healthcare_projects_locations_datasets_hl7_v2_stores_delete(
        &self,
        args: &HealthcareProjectsLocationsDatasetsHl7V2StoresDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_hl7_v2_stores_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_hl7_v2_stores_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets hl7 v2 stores export.
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
    pub fn healthcare_projects_locations_datasets_hl7_v2_stores_export(
        &self,
        args: &HealthcareProjectsLocationsDatasetsHl7V2StoresExportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_hl7_v2_stores_export_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_hl7_v2_stores_export_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets hl7 v2 stores import.
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
    pub fn healthcare_projects_locations_datasets_hl7_v2_stores_import(
        &self,
        args: &HealthcareProjectsLocationsDatasetsHl7V2StoresImportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_hl7_v2_stores_import_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_hl7_v2_stores_import_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets hl7 v2 stores patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Hl7V2Store result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn healthcare_projects_locations_datasets_hl7_v2_stores_patch(
        &self,
        args: &HealthcareProjectsLocationsDatasetsHl7V2StoresPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Hl7V2Store, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_hl7_v2_stores_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_hl7_v2_stores_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets hl7 v2 stores rollback.
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
    pub fn healthcare_projects_locations_datasets_hl7_v2_stores_rollback(
        &self,
        args: &HealthcareProjectsLocationsDatasetsHl7V2StoresRollbackArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_hl7_v2_stores_rollback_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_hl7_v2_stores_rollback_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets hl7 v2 stores set iam policy.
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
    pub fn healthcare_projects_locations_datasets_hl7_v2_stores_set_iam_policy(
        &self,
        args: &HealthcareProjectsLocationsDatasetsHl7V2StoresSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_hl7_v2_stores_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_hl7_v2_stores_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets hl7 v2 stores test iam permissions.
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
    pub fn healthcare_projects_locations_datasets_hl7_v2_stores_test_iam_permissions(
        &self,
        args: &HealthcareProjectsLocationsDatasetsHl7V2StoresTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_hl7_v2_stores_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_hl7_v2_stores_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets hl7 v2 stores messages create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Message result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn healthcare_projects_locations_datasets_hl7_v2_stores_messages_create(
        &self,
        args: &HealthcareProjectsLocationsDatasetsHl7V2StoresMessagesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Message, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_hl7_v2_stores_messages_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_hl7_v2_stores_messages_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets hl7 v2 stores messages delete.
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
    pub fn healthcare_projects_locations_datasets_hl7_v2_stores_messages_delete(
        &self,
        args: &HealthcareProjectsLocationsDatasetsHl7V2StoresMessagesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_hl7_v2_stores_messages_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_hl7_v2_stores_messages_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets hl7 v2 stores messages ingest.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IngestMessageResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn healthcare_projects_locations_datasets_hl7_v2_stores_messages_ingest(
        &self,
        args: &HealthcareProjectsLocationsDatasetsHl7V2StoresMessagesIngestArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IngestMessageResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_hl7_v2_stores_messages_ingest_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_hl7_v2_stores_messages_ingest_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets hl7 v2 stores messages patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Message result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn healthcare_projects_locations_datasets_hl7_v2_stores_messages_patch(
        &self,
        args: &HealthcareProjectsLocationsDatasetsHl7V2StoresMessagesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Message, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_hl7_v2_stores_messages_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_hl7_v2_stores_messages_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations datasets operations cancel.
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
    pub fn healthcare_projects_locations_datasets_operations_cancel(
        &self,
        args: &HealthcareProjectsLocationsDatasetsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_datasets_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_datasets_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Healthcare projects locations services nlp analyze entities.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AnalyzeEntitiesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn healthcare_projects_locations_services_nlp_analyze_entities(
        &self,
        args: &HealthcareProjectsLocationsServicesNlpAnalyzeEntitiesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AnalyzeEntitiesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = healthcare_projects_locations_services_nlp_analyze_entities_builder(
            &self.http_client,
            &args.nlpService,
        )
        .map_err(ProviderError::Api)?;

        let task = healthcare_projects_locations_services_nlp_analyze_entities_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
