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
    dlp_info_types_list_builder, dlp_info_types_list_task,
    dlp_locations_info_types_list_builder, dlp_locations_info_types_list_task,
    dlp_organizations_deidentify_templates_create_builder, dlp_organizations_deidentify_templates_create_task,
    dlp_organizations_deidentify_templates_delete_builder, dlp_organizations_deidentify_templates_delete_task,
    dlp_organizations_deidentify_templates_get_builder, dlp_organizations_deidentify_templates_get_task,
    dlp_organizations_deidentify_templates_list_builder, dlp_organizations_deidentify_templates_list_task,
    dlp_organizations_deidentify_templates_patch_builder, dlp_organizations_deidentify_templates_patch_task,
    dlp_organizations_inspect_templates_create_builder, dlp_organizations_inspect_templates_create_task,
    dlp_organizations_inspect_templates_delete_builder, dlp_organizations_inspect_templates_delete_task,
    dlp_organizations_inspect_templates_get_builder, dlp_organizations_inspect_templates_get_task,
    dlp_organizations_inspect_templates_list_builder, dlp_organizations_inspect_templates_list_task,
    dlp_organizations_inspect_templates_patch_builder, dlp_organizations_inspect_templates_patch_task,
    dlp_organizations_locations_column_data_profiles_get_builder, dlp_organizations_locations_column_data_profiles_get_task,
    dlp_organizations_locations_column_data_profiles_list_builder, dlp_organizations_locations_column_data_profiles_list_task,
    dlp_organizations_locations_connections_create_builder, dlp_organizations_locations_connections_create_task,
    dlp_organizations_locations_connections_delete_builder, dlp_organizations_locations_connections_delete_task,
    dlp_organizations_locations_connections_get_builder, dlp_organizations_locations_connections_get_task,
    dlp_organizations_locations_connections_list_builder, dlp_organizations_locations_connections_list_task,
    dlp_organizations_locations_connections_patch_builder, dlp_organizations_locations_connections_patch_task,
    dlp_organizations_locations_connections_search_builder, dlp_organizations_locations_connections_search_task,
    dlp_organizations_locations_deidentify_templates_create_builder, dlp_organizations_locations_deidentify_templates_create_task,
    dlp_organizations_locations_deidentify_templates_delete_builder, dlp_organizations_locations_deidentify_templates_delete_task,
    dlp_organizations_locations_deidentify_templates_get_builder, dlp_organizations_locations_deidentify_templates_get_task,
    dlp_organizations_locations_deidentify_templates_list_builder, dlp_organizations_locations_deidentify_templates_list_task,
    dlp_organizations_locations_deidentify_templates_patch_builder, dlp_organizations_locations_deidentify_templates_patch_task,
    dlp_organizations_locations_discovery_configs_create_builder, dlp_organizations_locations_discovery_configs_create_task,
    dlp_organizations_locations_discovery_configs_delete_builder, dlp_organizations_locations_discovery_configs_delete_task,
    dlp_organizations_locations_discovery_configs_get_builder, dlp_organizations_locations_discovery_configs_get_task,
    dlp_organizations_locations_discovery_configs_list_builder, dlp_organizations_locations_discovery_configs_list_task,
    dlp_organizations_locations_discovery_configs_patch_builder, dlp_organizations_locations_discovery_configs_patch_task,
    dlp_organizations_locations_dlp_jobs_list_builder, dlp_organizations_locations_dlp_jobs_list_task,
    dlp_organizations_locations_file_store_data_profiles_delete_builder, dlp_organizations_locations_file_store_data_profiles_delete_task,
    dlp_organizations_locations_file_store_data_profiles_get_builder, dlp_organizations_locations_file_store_data_profiles_get_task,
    dlp_organizations_locations_file_store_data_profiles_list_builder, dlp_organizations_locations_file_store_data_profiles_list_task,
    dlp_organizations_locations_info_types_list_builder, dlp_organizations_locations_info_types_list_task,
    dlp_organizations_locations_inspect_templates_create_builder, dlp_organizations_locations_inspect_templates_create_task,
    dlp_organizations_locations_inspect_templates_delete_builder, dlp_organizations_locations_inspect_templates_delete_task,
    dlp_organizations_locations_inspect_templates_get_builder, dlp_organizations_locations_inspect_templates_get_task,
    dlp_organizations_locations_inspect_templates_list_builder, dlp_organizations_locations_inspect_templates_list_task,
    dlp_organizations_locations_inspect_templates_patch_builder, dlp_organizations_locations_inspect_templates_patch_task,
    dlp_organizations_locations_job_triggers_create_builder, dlp_organizations_locations_job_triggers_create_task,
    dlp_organizations_locations_job_triggers_delete_builder, dlp_organizations_locations_job_triggers_delete_task,
    dlp_organizations_locations_job_triggers_get_builder, dlp_organizations_locations_job_triggers_get_task,
    dlp_organizations_locations_job_triggers_list_builder, dlp_organizations_locations_job_triggers_list_task,
    dlp_organizations_locations_job_triggers_patch_builder, dlp_organizations_locations_job_triggers_patch_task,
    dlp_organizations_locations_project_data_profiles_get_builder, dlp_organizations_locations_project_data_profiles_get_task,
    dlp_organizations_locations_project_data_profiles_list_builder, dlp_organizations_locations_project_data_profiles_list_task,
    dlp_organizations_locations_stored_info_types_create_builder, dlp_organizations_locations_stored_info_types_create_task,
    dlp_organizations_locations_stored_info_types_delete_builder, dlp_organizations_locations_stored_info_types_delete_task,
    dlp_organizations_locations_stored_info_types_get_builder, dlp_organizations_locations_stored_info_types_get_task,
    dlp_organizations_locations_stored_info_types_list_builder, dlp_organizations_locations_stored_info_types_list_task,
    dlp_organizations_locations_stored_info_types_patch_builder, dlp_organizations_locations_stored_info_types_patch_task,
    dlp_organizations_locations_table_data_profiles_delete_builder, dlp_organizations_locations_table_data_profiles_delete_task,
    dlp_organizations_locations_table_data_profiles_get_builder, dlp_organizations_locations_table_data_profiles_get_task,
    dlp_organizations_locations_table_data_profiles_list_builder, dlp_organizations_locations_table_data_profiles_list_task,
    dlp_organizations_stored_info_types_create_builder, dlp_organizations_stored_info_types_create_task,
    dlp_organizations_stored_info_types_delete_builder, dlp_organizations_stored_info_types_delete_task,
    dlp_organizations_stored_info_types_get_builder, dlp_organizations_stored_info_types_get_task,
    dlp_organizations_stored_info_types_list_builder, dlp_organizations_stored_info_types_list_task,
    dlp_organizations_stored_info_types_patch_builder, dlp_organizations_stored_info_types_patch_task,
    dlp_projects_content_deidentify_builder, dlp_projects_content_deidentify_task,
    dlp_projects_content_inspect_builder, dlp_projects_content_inspect_task,
    dlp_projects_content_reidentify_builder, dlp_projects_content_reidentify_task,
    dlp_projects_deidentify_templates_create_builder, dlp_projects_deidentify_templates_create_task,
    dlp_projects_deidentify_templates_delete_builder, dlp_projects_deidentify_templates_delete_task,
    dlp_projects_deidentify_templates_get_builder, dlp_projects_deidentify_templates_get_task,
    dlp_projects_deidentify_templates_list_builder, dlp_projects_deidentify_templates_list_task,
    dlp_projects_deidentify_templates_patch_builder, dlp_projects_deidentify_templates_patch_task,
    dlp_projects_dlp_jobs_cancel_builder, dlp_projects_dlp_jobs_cancel_task,
    dlp_projects_dlp_jobs_create_builder, dlp_projects_dlp_jobs_create_task,
    dlp_projects_dlp_jobs_delete_builder, dlp_projects_dlp_jobs_delete_task,
    dlp_projects_dlp_jobs_get_builder, dlp_projects_dlp_jobs_get_task,
    dlp_projects_dlp_jobs_list_builder, dlp_projects_dlp_jobs_list_task,
    dlp_projects_image_redact_builder, dlp_projects_image_redact_task,
    dlp_projects_inspect_templates_create_builder, dlp_projects_inspect_templates_create_task,
    dlp_projects_inspect_templates_delete_builder, dlp_projects_inspect_templates_delete_task,
    dlp_projects_inspect_templates_get_builder, dlp_projects_inspect_templates_get_task,
    dlp_projects_inspect_templates_list_builder, dlp_projects_inspect_templates_list_task,
    dlp_projects_inspect_templates_patch_builder, dlp_projects_inspect_templates_patch_task,
    dlp_projects_job_triggers_activate_builder, dlp_projects_job_triggers_activate_task,
    dlp_projects_job_triggers_create_builder, dlp_projects_job_triggers_create_task,
    dlp_projects_job_triggers_delete_builder, dlp_projects_job_triggers_delete_task,
    dlp_projects_job_triggers_get_builder, dlp_projects_job_triggers_get_task,
    dlp_projects_job_triggers_list_builder, dlp_projects_job_triggers_list_task,
    dlp_projects_job_triggers_patch_builder, dlp_projects_job_triggers_patch_task,
    dlp_projects_locations_column_data_profiles_get_builder, dlp_projects_locations_column_data_profiles_get_task,
    dlp_projects_locations_column_data_profiles_list_builder, dlp_projects_locations_column_data_profiles_list_task,
    dlp_projects_locations_connections_create_builder, dlp_projects_locations_connections_create_task,
    dlp_projects_locations_connections_delete_builder, dlp_projects_locations_connections_delete_task,
    dlp_projects_locations_connections_get_builder, dlp_projects_locations_connections_get_task,
    dlp_projects_locations_connections_list_builder, dlp_projects_locations_connections_list_task,
    dlp_projects_locations_connections_patch_builder, dlp_projects_locations_connections_patch_task,
    dlp_projects_locations_connections_search_builder, dlp_projects_locations_connections_search_task,
    dlp_projects_locations_content_deidentify_builder, dlp_projects_locations_content_deidentify_task,
    dlp_projects_locations_content_inspect_builder, dlp_projects_locations_content_inspect_task,
    dlp_projects_locations_content_reidentify_builder, dlp_projects_locations_content_reidentify_task,
    dlp_projects_locations_deidentify_templates_create_builder, dlp_projects_locations_deidentify_templates_create_task,
    dlp_projects_locations_deidentify_templates_delete_builder, dlp_projects_locations_deidentify_templates_delete_task,
    dlp_projects_locations_deidentify_templates_get_builder, dlp_projects_locations_deidentify_templates_get_task,
    dlp_projects_locations_deidentify_templates_list_builder, dlp_projects_locations_deidentify_templates_list_task,
    dlp_projects_locations_deidentify_templates_patch_builder, dlp_projects_locations_deidentify_templates_patch_task,
    dlp_projects_locations_discovery_configs_create_builder, dlp_projects_locations_discovery_configs_create_task,
    dlp_projects_locations_discovery_configs_delete_builder, dlp_projects_locations_discovery_configs_delete_task,
    dlp_projects_locations_discovery_configs_get_builder, dlp_projects_locations_discovery_configs_get_task,
    dlp_projects_locations_discovery_configs_list_builder, dlp_projects_locations_discovery_configs_list_task,
    dlp_projects_locations_discovery_configs_patch_builder, dlp_projects_locations_discovery_configs_patch_task,
    dlp_projects_locations_dlp_jobs_cancel_builder, dlp_projects_locations_dlp_jobs_cancel_task,
    dlp_projects_locations_dlp_jobs_create_builder, dlp_projects_locations_dlp_jobs_create_task,
    dlp_projects_locations_dlp_jobs_delete_builder, dlp_projects_locations_dlp_jobs_delete_task,
    dlp_projects_locations_dlp_jobs_finish_builder, dlp_projects_locations_dlp_jobs_finish_task,
    dlp_projects_locations_dlp_jobs_get_builder, dlp_projects_locations_dlp_jobs_get_task,
    dlp_projects_locations_dlp_jobs_hybrid_inspect_builder, dlp_projects_locations_dlp_jobs_hybrid_inspect_task,
    dlp_projects_locations_dlp_jobs_list_builder, dlp_projects_locations_dlp_jobs_list_task,
    dlp_projects_locations_file_store_data_profiles_delete_builder, dlp_projects_locations_file_store_data_profiles_delete_task,
    dlp_projects_locations_file_store_data_profiles_get_builder, dlp_projects_locations_file_store_data_profiles_get_task,
    dlp_projects_locations_file_store_data_profiles_list_builder, dlp_projects_locations_file_store_data_profiles_list_task,
    dlp_projects_locations_image_redact_builder, dlp_projects_locations_image_redact_task,
    dlp_projects_locations_info_types_list_builder, dlp_projects_locations_info_types_list_task,
    dlp_projects_locations_inspect_templates_create_builder, dlp_projects_locations_inspect_templates_create_task,
    dlp_projects_locations_inspect_templates_delete_builder, dlp_projects_locations_inspect_templates_delete_task,
    dlp_projects_locations_inspect_templates_get_builder, dlp_projects_locations_inspect_templates_get_task,
    dlp_projects_locations_inspect_templates_list_builder, dlp_projects_locations_inspect_templates_list_task,
    dlp_projects_locations_inspect_templates_patch_builder, dlp_projects_locations_inspect_templates_patch_task,
    dlp_projects_locations_job_triggers_activate_builder, dlp_projects_locations_job_triggers_activate_task,
    dlp_projects_locations_job_triggers_create_builder, dlp_projects_locations_job_triggers_create_task,
    dlp_projects_locations_job_triggers_delete_builder, dlp_projects_locations_job_triggers_delete_task,
    dlp_projects_locations_job_triggers_get_builder, dlp_projects_locations_job_triggers_get_task,
    dlp_projects_locations_job_triggers_hybrid_inspect_builder, dlp_projects_locations_job_triggers_hybrid_inspect_task,
    dlp_projects_locations_job_triggers_list_builder, dlp_projects_locations_job_triggers_list_task,
    dlp_projects_locations_job_triggers_patch_builder, dlp_projects_locations_job_triggers_patch_task,
    dlp_projects_locations_project_data_profiles_get_builder, dlp_projects_locations_project_data_profiles_get_task,
    dlp_projects_locations_project_data_profiles_list_builder, dlp_projects_locations_project_data_profiles_list_task,
    dlp_projects_locations_stored_info_types_create_builder, dlp_projects_locations_stored_info_types_create_task,
    dlp_projects_locations_stored_info_types_delete_builder, dlp_projects_locations_stored_info_types_delete_task,
    dlp_projects_locations_stored_info_types_get_builder, dlp_projects_locations_stored_info_types_get_task,
    dlp_projects_locations_stored_info_types_list_builder, dlp_projects_locations_stored_info_types_list_task,
    dlp_projects_locations_stored_info_types_patch_builder, dlp_projects_locations_stored_info_types_patch_task,
    dlp_projects_locations_table_data_profiles_delete_builder, dlp_projects_locations_table_data_profiles_delete_task,
    dlp_projects_locations_table_data_profiles_get_builder, dlp_projects_locations_table_data_profiles_get_task,
    dlp_projects_locations_table_data_profiles_list_builder, dlp_projects_locations_table_data_profiles_list_task,
    dlp_projects_stored_info_types_create_builder, dlp_projects_stored_info_types_create_task,
    dlp_projects_stored_info_types_delete_builder, dlp_projects_stored_info_types_delete_task,
    dlp_projects_stored_info_types_get_builder, dlp_projects_stored_info_types_get_task,
    dlp_projects_stored_info_types_list_builder, dlp_projects_stored_info_types_list_task,
    dlp_projects_stored_info_types_patch_builder, dlp_projects_stored_info_types_patch_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::dlp::GooglePrivacyDlpV2ColumnDataProfile;
use crate::providers::gcp::clients::dlp::GooglePrivacyDlpV2Connection;
use crate::providers::gcp::clients::dlp::GooglePrivacyDlpV2DeidentifyContentResponse;
use crate::providers::gcp::clients::dlp::GooglePrivacyDlpV2DeidentifyTemplate;
use crate::providers::gcp::clients::dlp::GooglePrivacyDlpV2DiscoveryConfig;
use crate::providers::gcp::clients::dlp::GooglePrivacyDlpV2DlpJob;
use crate::providers::gcp::clients::dlp::GooglePrivacyDlpV2FileStoreDataProfile;
use crate::providers::gcp::clients::dlp::GooglePrivacyDlpV2HybridInspectResponse;
use crate::providers::gcp::clients::dlp::GooglePrivacyDlpV2InspectContentResponse;
use crate::providers::gcp::clients::dlp::GooglePrivacyDlpV2InspectTemplate;
use crate::providers::gcp::clients::dlp::GooglePrivacyDlpV2JobTrigger;
use crate::providers::gcp::clients::dlp::GooglePrivacyDlpV2ListColumnDataProfilesResponse;
use crate::providers::gcp::clients::dlp::GooglePrivacyDlpV2ListConnectionsResponse;
use crate::providers::gcp::clients::dlp::GooglePrivacyDlpV2ListDeidentifyTemplatesResponse;
use crate::providers::gcp::clients::dlp::GooglePrivacyDlpV2ListDiscoveryConfigsResponse;
use crate::providers::gcp::clients::dlp::GooglePrivacyDlpV2ListDlpJobsResponse;
use crate::providers::gcp::clients::dlp::GooglePrivacyDlpV2ListFileStoreDataProfilesResponse;
use crate::providers::gcp::clients::dlp::GooglePrivacyDlpV2ListInfoTypesResponse;
use crate::providers::gcp::clients::dlp::GooglePrivacyDlpV2ListInspectTemplatesResponse;
use crate::providers::gcp::clients::dlp::GooglePrivacyDlpV2ListJobTriggersResponse;
use crate::providers::gcp::clients::dlp::GooglePrivacyDlpV2ListProjectDataProfilesResponse;
use crate::providers::gcp::clients::dlp::GooglePrivacyDlpV2ListStoredInfoTypesResponse;
use crate::providers::gcp::clients::dlp::GooglePrivacyDlpV2ListTableDataProfilesResponse;
use crate::providers::gcp::clients::dlp::GooglePrivacyDlpV2ProjectDataProfile;
use crate::providers::gcp::clients::dlp::GooglePrivacyDlpV2RedactImageResponse;
use crate::providers::gcp::clients::dlp::GooglePrivacyDlpV2ReidentifyContentResponse;
use crate::providers::gcp::clients::dlp::GooglePrivacyDlpV2SearchConnectionsResponse;
use crate::providers::gcp::clients::dlp::GooglePrivacyDlpV2StoredInfoType;
use crate::providers::gcp::clients::dlp::GooglePrivacyDlpV2TableDataProfile;
use crate::providers::gcp::clients::dlp::GoogleProtobufEmpty;
use crate::providers::gcp::clients::dlp::DlpInfoTypesListArgs;
use crate::providers::gcp::clients::dlp::DlpLocationsInfoTypesListArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsDeidentifyTemplatesCreateArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsDeidentifyTemplatesDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsDeidentifyTemplatesGetArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsDeidentifyTemplatesListArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsDeidentifyTemplatesPatchArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsInspectTemplatesCreateArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsInspectTemplatesDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsInspectTemplatesGetArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsInspectTemplatesListArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsInspectTemplatesPatchArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsColumnDataProfilesGetArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsColumnDataProfilesListArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsConnectionsCreateArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsConnectionsDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsConnectionsGetArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsConnectionsListArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsConnectionsPatchArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsConnectionsSearchArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsDeidentifyTemplatesCreateArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsDeidentifyTemplatesDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsDeidentifyTemplatesGetArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsDeidentifyTemplatesListArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsDeidentifyTemplatesPatchArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsDiscoveryConfigsCreateArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsDiscoveryConfigsDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsDiscoveryConfigsGetArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsDiscoveryConfigsListArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsDiscoveryConfigsPatchArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsDlpJobsListArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsFileStoreDataProfilesDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsFileStoreDataProfilesGetArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsFileStoreDataProfilesListArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsInfoTypesListArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsInspectTemplatesCreateArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsInspectTemplatesDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsInspectTemplatesGetArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsInspectTemplatesListArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsInspectTemplatesPatchArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsJobTriggersCreateArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsJobTriggersDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsJobTriggersGetArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsJobTriggersListArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsJobTriggersPatchArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsProjectDataProfilesGetArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsProjectDataProfilesListArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsStoredInfoTypesCreateArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsStoredInfoTypesDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsStoredInfoTypesGetArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsStoredInfoTypesListArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsStoredInfoTypesPatchArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsTableDataProfilesDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsTableDataProfilesGetArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsLocationsTableDataProfilesListArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsStoredInfoTypesCreateArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsStoredInfoTypesDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsStoredInfoTypesGetArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsStoredInfoTypesListArgs;
use crate::providers::gcp::clients::dlp::DlpOrganizationsStoredInfoTypesPatchArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsContentDeidentifyArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsContentInspectArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsContentReidentifyArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsDeidentifyTemplatesCreateArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsDeidentifyTemplatesDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsDeidentifyTemplatesGetArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsDeidentifyTemplatesListArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsDeidentifyTemplatesPatchArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsDlpJobsCancelArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsDlpJobsCreateArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsDlpJobsDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsDlpJobsGetArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsDlpJobsListArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsImageRedactArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsInspectTemplatesCreateArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsInspectTemplatesDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsInspectTemplatesGetArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsInspectTemplatesListArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsInspectTemplatesPatchArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsJobTriggersActivateArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsJobTriggersCreateArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsJobTriggersDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsJobTriggersGetArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsJobTriggersListArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsJobTriggersPatchArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsColumnDataProfilesGetArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsColumnDataProfilesListArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsConnectionsCreateArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsConnectionsDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsConnectionsGetArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsConnectionsListArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsConnectionsPatchArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsConnectionsSearchArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsContentDeidentifyArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsContentInspectArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsContentReidentifyArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsDeidentifyTemplatesCreateArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsDeidentifyTemplatesDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsDeidentifyTemplatesGetArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsDeidentifyTemplatesListArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsDeidentifyTemplatesPatchArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsDiscoveryConfigsCreateArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsDiscoveryConfigsDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsDiscoveryConfigsGetArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsDiscoveryConfigsListArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsDiscoveryConfigsPatchArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsDlpJobsCancelArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsDlpJobsCreateArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsDlpJobsDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsDlpJobsFinishArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsDlpJobsGetArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsDlpJobsHybridInspectArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsDlpJobsListArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsFileStoreDataProfilesDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsFileStoreDataProfilesGetArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsFileStoreDataProfilesListArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsImageRedactArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsInfoTypesListArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsInspectTemplatesCreateArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsInspectTemplatesDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsInspectTemplatesGetArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsInspectTemplatesListArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsInspectTemplatesPatchArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsJobTriggersActivateArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsJobTriggersCreateArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsJobTriggersDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsJobTriggersGetArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsJobTriggersHybridInspectArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsJobTriggersListArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsJobTriggersPatchArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsProjectDataProfilesGetArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsProjectDataProfilesListArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsStoredInfoTypesCreateArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsStoredInfoTypesDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsStoredInfoTypesGetArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsStoredInfoTypesListArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsStoredInfoTypesPatchArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsTableDataProfilesDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsTableDataProfilesGetArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsLocationsTableDataProfilesListArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsStoredInfoTypesCreateArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsStoredInfoTypesDeleteArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsStoredInfoTypesGetArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsStoredInfoTypesListArgs;
use crate::providers::gcp::clients::dlp::DlpProjectsStoredInfoTypesPatchArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// DlpProvider with automatic state tracking.
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
/// let provider = DlpProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct DlpProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> DlpProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new DlpProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new DlpProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Dlp info types list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2ListInfoTypesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dlp_info_types_list(
        &self,
        args: &DlpInfoTypesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2ListInfoTypesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_info_types_list_builder(
            &self.http_client,
            &args.filter,
            &args.languageCode,
            &args.locationId,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_info_types_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp locations info types list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2ListInfoTypesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dlp_locations_info_types_list(
        &self,
        args: &DlpLocationsInfoTypesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2ListInfoTypesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_locations_info_types_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.languageCode,
            &args.locationId,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_locations_info_types_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Dlp organizations deidentify templates get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn dlp_organizations_deidentify_templates_get(
        &self,
        args: &DlpOrganizationsDeidentifyTemplatesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2DeidentifyTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_deidentify_templates_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_deidentify_templates_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp organizations deidentify templates list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2ListDeidentifyTemplatesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dlp_organizations_deidentify_templates_list(
        &self,
        args: &DlpOrganizationsDeidentifyTemplatesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2ListDeidentifyTemplatesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_deidentify_templates_list_builder(
            &self.http_client,
            &args.parent,
            &args.locationId,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_deidentify_templates_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp organizations inspect templates get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn dlp_organizations_inspect_templates_get(
        &self,
        args: &DlpOrganizationsInspectTemplatesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2InspectTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_inspect_templates_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_inspect_templates_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp organizations inspect templates list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2ListInspectTemplatesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dlp_organizations_inspect_templates_list(
        &self,
        args: &DlpOrganizationsInspectTemplatesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2ListInspectTemplatesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_inspect_templates_list_builder(
            &self.http_client,
            &args.parent,
            &args.locationId,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_inspect_templates_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp organizations inspect templates patch.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp organizations locations column data profiles get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2ColumnDataProfile result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dlp_organizations_locations_column_data_profiles_get(
        &self,
        args: &DlpOrganizationsLocationsColumnDataProfilesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2ColumnDataProfile, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_locations_column_data_profiles_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_locations_column_data_profiles_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp organizations locations column data profiles list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2ListColumnDataProfilesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dlp_organizations_locations_column_data_profiles_list(
        &self,
        args: &DlpOrganizationsLocationsColumnDataProfilesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2ListColumnDataProfilesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_locations_column_data_profiles_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_locations_column_data_profiles_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Dlp organizations locations connections get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn dlp_organizations_locations_connections_get(
        &self,
        args: &DlpOrganizationsLocationsConnectionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2Connection, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_locations_connections_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_locations_connections_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp organizations locations connections list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2ListConnectionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dlp_organizations_locations_connections_list(
        &self,
        args: &DlpOrganizationsLocationsConnectionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2ListConnectionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_locations_connections_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_locations_connections_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Dlp organizations locations connections search.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2SearchConnectionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dlp_organizations_locations_connections_search(
        &self,
        args: &DlpOrganizationsLocationsConnectionsSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2SearchConnectionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_locations_connections_search_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_locations_connections_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Dlp organizations locations deidentify templates get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn dlp_organizations_locations_deidentify_templates_get(
        &self,
        args: &DlpOrganizationsLocationsDeidentifyTemplatesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2DeidentifyTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_locations_deidentify_templates_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_locations_deidentify_templates_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp organizations locations deidentify templates list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2ListDeidentifyTemplatesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dlp_organizations_locations_deidentify_templates_list(
        &self,
        args: &DlpOrganizationsLocationsDeidentifyTemplatesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2ListDeidentifyTemplatesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_locations_deidentify_templates_list_builder(
            &self.http_client,
            &args.parent,
            &args.locationId,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_locations_deidentify_templates_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Dlp organizations locations discovery configs get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn dlp_organizations_locations_discovery_configs_get(
        &self,
        args: &DlpOrganizationsLocationsDiscoveryConfigsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2DiscoveryConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_locations_discovery_configs_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_locations_discovery_configs_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp organizations locations discovery configs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2ListDiscoveryConfigsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dlp_organizations_locations_discovery_configs_list(
        &self,
        args: &DlpOrganizationsLocationsDiscoveryConfigsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2ListDiscoveryConfigsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_locations_discovery_configs_list_builder(
            &self.http_client,
            &args.parent,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_locations_discovery_configs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Dlp organizations locations dlp jobs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2ListDlpJobsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dlp_organizations_locations_dlp_jobs_list(
        &self,
        args: &DlpOrganizationsLocationsDlpJobsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2ListDlpJobsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_locations_dlp_jobs_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.locationId,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.type_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_locations_dlp_jobs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Dlp organizations locations file store data profiles get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2FileStoreDataProfile result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dlp_organizations_locations_file_store_data_profiles_get(
        &self,
        args: &DlpOrganizationsLocationsFileStoreDataProfilesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2FileStoreDataProfile, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_locations_file_store_data_profiles_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_locations_file_store_data_profiles_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp organizations locations file store data profiles list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2ListFileStoreDataProfilesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dlp_organizations_locations_file_store_data_profiles_list(
        &self,
        args: &DlpOrganizationsLocationsFileStoreDataProfilesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2ListFileStoreDataProfilesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_locations_file_store_data_profiles_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_locations_file_store_data_profiles_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp organizations locations info types list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2ListInfoTypesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dlp_organizations_locations_info_types_list(
        &self,
        args: &DlpOrganizationsLocationsInfoTypesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2ListInfoTypesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_locations_info_types_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.languageCode,
            &args.locationId,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_locations_info_types_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp organizations locations inspect templates get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn dlp_organizations_locations_inspect_templates_get(
        &self,
        args: &DlpOrganizationsLocationsInspectTemplatesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2InspectTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_locations_inspect_templates_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_locations_inspect_templates_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp organizations locations inspect templates list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2ListInspectTemplatesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dlp_organizations_locations_inspect_templates_list(
        &self,
        args: &DlpOrganizationsLocationsInspectTemplatesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2ListInspectTemplatesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_locations_inspect_templates_list_builder(
            &self.http_client,
            &args.parent,
            &args.locationId,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_locations_inspect_templates_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp organizations locations inspect templates patch.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Dlp organizations locations job triggers get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn dlp_organizations_locations_job_triggers_get(
        &self,
        args: &DlpOrganizationsLocationsJobTriggersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2JobTrigger, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_locations_job_triggers_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_locations_job_triggers_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp organizations locations job triggers list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2ListJobTriggersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dlp_organizations_locations_job_triggers_list(
        &self,
        args: &DlpOrganizationsLocationsJobTriggersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2ListJobTriggersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_locations_job_triggers_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.locationId,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.type_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_locations_job_triggers_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Dlp organizations locations project data profiles get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2ProjectDataProfile result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dlp_organizations_locations_project_data_profiles_get(
        &self,
        args: &DlpOrganizationsLocationsProjectDataProfilesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2ProjectDataProfile, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_locations_project_data_profiles_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_locations_project_data_profiles_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp organizations locations project data profiles list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2ListProjectDataProfilesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dlp_organizations_locations_project_data_profiles_list(
        &self,
        args: &DlpOrganizationsLocationsProjectDataProfilesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2ListProjectDataProfilesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_locations_project_data_profiles_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_locations_project_data_profiles_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Dlp organizations locations stored info types get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn dlp_organizations_locations_stored_info_types_get(
        &self,
        args: &DlpOrganizationsLocationsStoredInfoTypesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2StoredInfoType, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_locations_stored_info_types_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_locations_stored_info_types_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp organizations locations stored info types list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2ListStoredInfoTypesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dlp_organizations_locations_stored_info_types_list(
        &self,
        args: &DlpOrganizationsLocationsStoredInfoTypesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2ListStoredInfoTypesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_locations_stored_info_types_list_builder(
            &self.http_client,
            &args.parent,
            &args.locationId,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_locations_stored_info_types_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Dlp organizations locations table data profiles get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2TableDataProfile result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dlp_organizations_locations_table_data_profiles_get(
        &self,
        args: &DlpOrganizationsLocationsTableDataProfilesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2TableDataProfile, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_locations_table_data_profiles_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_locations_table_data_profiles_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp organizations locations table data profiles list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2ListTableDataProfilesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dlp_organizations_locations_table_data_profiles_list(
        &self,
        args: &DlpOrganizationsLocationsTableDataProfilesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2ListTableDataProfilesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_locations_table_data_profiles_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_locations_table_data_profiles_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Dlp organizations stored info types get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn dlp_organizations_stored_info_types_get(
        &self,
        args: &DlpOrganizationsStoredInfoTypesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2StoredInfoType, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_stored_info_types_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_stored_info_types_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp organizations stored info types list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2ListStoredInfoTypesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dlp_organizations_stored_info_types_list(
        &self,
        args: &DlpOrganizationsStoredInfoTypesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2ListStoredInfoTypesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_organizations_stored_info_types_list_builder(
            &self.http_client,
            &args.parent,
            &args.locationId,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_organizations_stored_info_types_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Dlp projects deidentify templates get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn dlp_projects_deidentify_templates_get(
        &self,
        args: &DlpProjectsDeidentifyTemplatesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2DeidentifyTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_deidentify_templates_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_deidentify_templates_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects deidentify templates list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2ListDeidentifyTemplatesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dlp_projects_deidentify_templates_list(
        &self,
        args: &DlpProjectsDeidentifyTemplatesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2ListDeidentifyTemplatesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_deidentify_templates_list_builder(
            &self.http_client,
            &args.parent,
            &args.locationId,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_deidentify_templates_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Dlp projects dlp jobs get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn dlp_projects_dlp_jobs_get(
        &self,
        args: &DlpProjectsDlpJobsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2DlpJob, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_dlp_jobs_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_dlp_jobs_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects dlp jobs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2ListDlpJobsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dlp_projects_dlp_jobs_list(
        &self,
        args: &DlpProjectsDlpJobsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2ListDlpJobsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_dlp_jobs_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.locationId,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.type_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_dlp_jobs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects inspect templates get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn dlp_projects_inspect_templates_get(
        &self,
        args: &DlpProjectsInspectTemplatesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2InspectTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_inspect_templates_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_inspect_templates_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects inspect templates list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2ListInspectTemplatesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dlp_projects_inspect_templates_list(
        &self,
        args: &DlpProjectsInspectTemplatesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2ListInspectTemplatesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_inspect_templates_list_builder(
            &self.http_client,
            &args.parent,
            &args.locationId,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_inspect_templates_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects inspect templates patch.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Dlp projects job triggers get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn dlp_projects_job_triggers_get(
        &self,
        args: &DlpProjectsJobTriggersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2JobTrigger, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_job_triggers_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_job_triggers_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects job triggers list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2ListJobTriggersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dlp_projects_job_triggers_list(
        &self,
        args: &DlpProjectsJobTriggersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2ListJobTriggersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_job_triggers_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.locationId,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.type_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_job_triggers_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Dlp projects locations column data profiles get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2ColumnDataProfile result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dlp_projects_locations_column_data_profiles_get(
        &self,
        args: &DlpProjectsLocationsColumnDataProfilesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2ColumnDataProfile, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_column_data_profiles_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_column_data_profiles_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects locations column data profiles list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2ListColumnDataProfilesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dlp_projects_locations_column_data_profiles_list(
        &self,
        args: &DlpProjectsLocationsColumnDataProfilesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2ListColumnDataProfilesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_column_data_profiles_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_column_data_profiles_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Dlp projects locations connections get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn dlp_projects_locations_connections_get(
        &self,
        args: &DlpProjectsLocationsConnectionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2Connection, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_connections_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_connections_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects locations connections list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2ListConnectionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dlp_projects_locations_connections_list(
        &self,
        args: &DlpProjectsLocationsConnectionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2ListConnectionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_connections_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_connections_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Dlp projects locations connections search.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2SearchConnectionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dlp_projects_locations_connections_search(
        &self,
        args: &DlpProjectsLocationsConnectionsSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2SearchConnectionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_connections_search_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_connections_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Dlp projects locations deidentify templates get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn dlp_projects_locations_deidentify_templates_get(
        &self,
        args: &DlpProjectsLocationsDeidentifyTemplatesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2DeidentifyTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_deidentify_templates_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_deidentify_templates_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects locations deidentify templates list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2ListDeidentifyTemplatesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dlp_projects_locations_deidentify_templates_list(
        &self,
        args: &DlpProjectsLocationsDeidentifyTemplatesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2ListDeidentifyTemplatesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_deidentify_templates_list_builder(
            &self.http_client,
            &args.parent,
            &args.locationId,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_deidentify_templates_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Dlp projects locations discovery configs get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn dlp_projects_locations_discovery_configs_get(
        &self,
        args: &DlpProjectsLocationsDiscoveryConfigsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2DiscoveryConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_discovery_configs_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_discovery_configs_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects locations discovery configs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2ListDiscoveryConfigsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dlp_projects_locations_discovery_configs_list(
        &self,
        args: &DlpProjectsLocationsDiscoveryConfigsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2ListDiscoveryConfigsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_discovery_configs_list_builder(
            &self.http_client,
            &args.parent,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_discovery_configs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Dlp projects locations dlp jobs get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn dlp_projects_locations_dlp_jobs_get(
        &self,
        args: &DlpProjectsLocationsDlpJobsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2DlpJob, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_dlp_jobs_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_dlp_jobs_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects locations dlp jobs hybrid inspect.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects locations dlp jobs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2ListDlpJobsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dlp_projects_locations_dlp_jobs_list(
        &self,
        args: &DlpProjectsLocationsDlpJobsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2ListDlpJobsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_dlp_jobs_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.locationId,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.type_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_dlp_jobs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Dlp projects locations file store data profiles get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2FileStoreDataProfile result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dlp_projects_locations_file_store_data_profiles_get(
        &self,
        args: &DlpProjectsLocationsFileStoreDataProfilesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2FileStoreDataProfile, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_file_store_data_profiles_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_file_store_data_profiles_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects locations file store data profiles list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2ListFileStoreDataProfilesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dlp_projects_locations_file_store_data_profiles_list(
        &self,
        args: &DlpProjectsLocationsFileStoreDataProfilesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2ListFileStoreDataProfilesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_file_store_data_profiles_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_file_store_data_profiles_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Dlp projects locations info types list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2ListInfoTypesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dlp_projects_locations_info_types_list(
        &self,
        args: &DlpProjectsLocationsInfoTypesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2ListInfoTypesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_info_types_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.languageCode,
            &args.locationId,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_info_types_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects locations inspect templates get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn dlp_projects_locations_inspect_templates_get(
        &self,
        args: &DlpProjectsLocationsInspectTemplatesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2InspectTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_inspect_templates_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_inspect_templates_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects locations inspect templates list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2ListInspectTemplatesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dlp_projects_locations_inspect_templates_list(
        &self,
        args: &DlpProjectsLocationsInspectTemplatesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2ListInspectTemplatesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_inspect_templates_list_builder(
            &self.http_client,
            &args.parent,
            &args.locationId,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_inspect_templates_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects locations inspect templates patch.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Dlp projects locations job triggers get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn dlp_projects_locations_job_triggers_get(
        &self,
        args: &DlpProjectsLocationsJobTriggersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2JobTrigger, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_job_triggers_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_job_triggers_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects locations job triggers hybrid inspect.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects locations job triggers list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2ListJobTriggersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dlp_projects_locations_job_triggers_list(
        &self,
        args: &DlpProjectsLocationsJobTriggersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2ListJobTriggersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_job_triggers_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.locationId,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.type_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_job_triggers_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Dlp projects locations project data profiles get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2ProjectDataProfile result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dlp_projects_locations_project_data_profiles_get(
        &self,
        args: &DlpProjectsLocationsProjectDataProfilesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2ProjectDataProfile, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_project_data_profiles_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_project_data_profiles_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects locations project data profiles list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2ListProjectDataProfilesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dlp_projects_locations_project_data_profiles_list(
        &self,
        args: &DlpProjectsLocationsProjectDataProfilesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2ListProjectDataProfilesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_project_data_profiles_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_project_data_profiles_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Dlp projects locations stored info types get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn dlp_projects_locations_stored_info_types_get(
        &self,
        args: &DlpProjectsLocationsStoredInfoTypesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2StoredInfoType, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_stored_info_types_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_stored_info_types_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects locations stored info types list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2ListStoredInfoTypesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dlp_projects_locations_stored_info_types_list(
        &self,
        args: &DlpProjectsLocationsStoredInfoTypesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2ListStoredInfoTypesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_stored_info_types_list_builder(
            &self.http_client,
            &args.parent,
            &args.locationId,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_stored_info_types_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Dlp projects locations table data profiles get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2TableDataProfile result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dlp_projects_locations_table_data_profiles_get(
        &self,
        args: &DlpProjectsLocationsTableDataProfilesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2TableDataProfile, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_table_data_profiles_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_table_data_profiles_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects locations table data profiles list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2ListTableDataProfilesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dlp_projects_locations_table_data_profiles_list(
        &self,
        args: &DlpProjectsLocationsTableDataProfilesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2ListTableDataProfilesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_locations_table_data_profiles_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_locations_table_data_profiles_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Dlp projects stored info types get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn dlp_projects_stored_info_types_get(
        &self,
        args: &DlpProjectsStoredInfoTypesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2StoredInfoType, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_stored_info_types_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_stored_info_types_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dlp projects stored info types list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePrivacyDlpV2ListStoredInfoTypesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dlp_projects_stored_info_types_list(
        &self,
        args: &DlpProjectsStoredInfoTypesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePrivacyDlpV2ListStoredInfoTypesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dlp_projects_stored_info_types_list_builder(
            &self.http_client,
            &args.parent,
            &args.locationId,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dlp_projects_stored_info_types_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
