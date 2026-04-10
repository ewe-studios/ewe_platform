//! TagmanagerProvider - State-aware tagmanager API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       tagmanager API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::tagmanager::{
    tagmanager_accounts_get_builder, tagmanager_accounts_get_task,
    tagmanager_accounts_list_builder, tagmanager_accounts_list_task,
    tagmanager_accounts_update_builder, tagmanager_accounts_update_task,
    tagmanager_accounts_containers_combine_builder, tagmanager_accounts_containers_combine_task,
    tagmanager_accounts_containers_create_builder, tagmanager_accounts_containers_create_task,
    tagmanager_accounts_containers_delete_builder, tagmanager_accounts_containers_delete_task,
    tagmanager_accounts_containers_get_builder, tagmanager_accounts_containers_get_task,
    tagmanager_accounts_containers_list_builder, tagmanager_accounts_containers_list_task,
    tagmanager_accounts_containers_lookup_builder, tagmanager_accounts_containers_lookup_task,
    tagmanager_accounts_containers_move_tag_id_builder, tagmanager_accounts_containers_move_tag_id_task,
    tagmanager_accounts_containers_snippet_builder, tagmanager_accounts_containers_snippet_task,
    tagmanager_accounts_containers_update_builder, tagmanager_accounts_containers_update_task,
    tagmanager_accounts_containers_destinations_get_builder, tagmanager_accounts_containers_destinations_get_task,
    tagmanager_accounts_containers_destinations_link_builder, tagmanager_accounts_containers_destinations_link_task,
    tagmanager_accounts_containers_destinations_list_builder, tagmanager_accounts_containers_destinations_list_task,
    tagmanager_accounts_containers_environments_create_builder, tagmanager_accounts_containers_environments_create_task,
    tagmanager_accounts_containers_environments_delete_builder, tagmanager_accounts_containers_environments_delete_task,
    tagmanager_accounts_containers_environments_get_builder, tagmanager_accounts_containers_environments_get_task,
    tagmanager_accounts_containers_environments_list_builder, tagmanager_accounts_containers_environments_list_task,
    tagmanager_accounts_containers_environments_reauthorize_builder, tagmanager_accounts_containers_environments_reauthorize_task,
    tagmanager_accounts_containers_environments_update_builder, tagmanager_accounts_containers_environments_update_task,
    tagmanager_accounts_containers_version_headers_latest_builder, tagmanager_accounts_containers_version_headers_latest_task,
    tagmanager_accounts_containers_version_headers_list_builder, tagmanager_accounts_containers_version_headers_list_task,
    tagmanager_accounts_containers_versions_delete_builder, tagmanager_accounts_containers_versions_delete_task,
    tagmanager_accounts_containers_versions_get_builder, tagmanager_accounts_containers_versions_get_task,
    tagmanager_accounts_containers_versions_live_builder, tagmanager_accounts_containers_versions_live_task,
    tagmanager_accounts_containers_versions_publish_builder, tagmanager_accounts_containers_versions_publish_task,
    tagmanager_accounts_containers_versions_set_latest_builder, tagmanager_accounts_containers_versions_set_latest_task,
    tagmanager_accounts_containers_versions_undelete_builder, tagmanager_accounts_containers_versions_undelete_task,
    tagmanager_accounts_containers_versions_update_builder, tagmanager_accounts_containers_versions_update_task,
    tagmanager_accounts_containers_workspaces_bulk_update_builder, tagmanager_accounts_containers_workspaces_bulk_update_task,
    tagmanager_accounts_containers_workspaces_create_builder, tagmanager_accounts_containers_workspaces_create_task,
    tagmanager_accounts_containers_workspaces_create_version_builder, tagmanager_accounts_containers_workspaces_create_version_task,
    tagmanager_accounts_containers_workspaces_delete_builder, tagmanager_accounts_containers_workspaces_delete_task,
    tagmanager_accounts_containers_workspaces_get_builder, tagmanager_accounts_containers_workspaces_get_task,
    tagmanager_accounts_containers_workspaces_get_status_builder, tagmanager_accounts_containers_workspaces_get_status_task,
    tagmanager_accounts_containers_workspaces_list_builder, tagmanager_accounts_containers_workspaces_list_task,
    tagmanager_accounts_containers_workspaces_quick_preview_builder, tagmanager_accounts_containers_workspaces_quick_preview_task,
    tagmanager_accounts_containers_workspaces_resolve_conflict_builder, tagmanager_accounts_containers_workspaces_resolve_conflict_task,
    tagmanager_accounts_containers_workspaces_sync_builder, tagmanager_accounts_containers_workspaces_sync_task,
    tagmanager_accounts_containers_workspaces_update_builder, tagmanager_accounts_containers_workspaces_update_task,
    tagmanager_accounts_containers_workspaces_built_in_variables_create_builder, tagmanager_accounts_containers_workspaces_built_in_variables_create_task,
    tagmanager_accounts_containers_workspaces_built_in_variables_delete_builder, tagmanager_accounts_containers_workspaces_built_in_variables_delete_task,
    tagmanager_accounts_containers_workspaces_built_in_variables_list_builder, tagmanager_accounts_containers_workspaces_built_in_variables_list_task,
    tagmanager_accounts_containers_workspaces_built_in_variables_revert_builder, tagmanager_accounts_containers_workspaces_built_in_variables_revert_task,
    tagmanager_accounts_containers_workspaces_clients_create_builder, tagmanager_accounts_containers_workspaces_clients_create_task,
    tagmanager_accounts_containers_workspaces_clients_delete_builder, tagmanager_accounts_containers_workspaces_clients_delete_task,
    tagmanager_accounts_containers_workspaces_clients_get_builder, tagmanager_accounts_containers_workspaces_clients_get_task,
    tagmanager_accounts_containers_workspaces_clients_list_builder, tagmanager_accounts_containers_workspaces_clients_list_task,
    tagmanager_accounts_containers_workspaces_clients_revert_builder, tagmanager_accounts_containers_workspaces_clients_revert_task,
    tagmanager_accounts_containers_workspaces_clients_update_builder, tagmanager_accounts_containers_workspaces_clients_update_task,
    tagmanager_accounts_containers_workspaces_folders_create_builder, tagmanager_accounts_containers_workspaces_folders_create_task,
    tagmanager_accounts_containers_workspaces_folders_delete_builder, tagmanager_accounts_containers_workspaces_folders_delete_task,
    tagmanager_accounts_containers_workspaces_folders_entities_builder, tagmanager_accounts_containers_workspaces_folders_entities_task,
    tagmanager_accounts_containers_workspaces_folders_get_builder, tagmanager_accounts_containers_workspaces_folders_get_task,
    tagmanager_accounts_containers_workspaces_folders_list_builder, tagmanager_accounts_containers_workspaces_folders_list_task,
    tagmanager_accounts_containers_workspaces_folders_move_entities_to_folder_builder, tagmanager_accounts_containers_workspaces_folders_move_entities_to_folder_task,
    tagmanager_accounts_containers_workspaces_folders_revert_builder, tagmanager_accounts_containers_workspaces_folders_revert_task,
    tagmanager_accounts_containers_workspaces_folders_update_builder, tagmanager_accounts_containers_workspaces_folders_update_task,
    tagmanager_accounts_containers_workspaces_gtag_config_create_builder, tagmanager_accounts_containers_workspaces_gtag_config_create_task,
    tagmanager_accounts_containers_workspaces_gtag_config_delete_builder, tagmanager_accounts_containers_workspaces_gtag_config_delete_task,
    tagmanager_accounts_containers_workspaces_gtag_config_get_builder, tagmanager_accounts_containers_workspaces_gtag_config_get_task,
    tagmanager_accounts_containers_workspaces_gtag_config_list_builder, tagmanager_accounts_containers_workspaces_gtag_config_list_task,
    tagmanager_accounts_containers_workspaces_gtag_config_update_builder, tagmanager_accounts_containers_workspaces_gtag_config_update_task,
    tagmanager_accounts_containers_workspaces_tags_create_builder, tagmanager_accounts_containers_workspaces_tags_create_task,
    tagmanager_accounts_containers_workspaces_tags_delete_builder, tagmanager_accounts_containers_workspaces_tags_delete_task,
    tagmanager_accounts_containers_workspaces_tags_get_builder, tagmanager_accounts_containers_workspaces_tags_get_task,
    tagmanager_accounts_containers_workspaces_tags_list_builder, tagmanager_accounts_containers_workspaces_tags_list_task,
    tagmanager_accounts_containers_workspaces_tags_revert_builder, tagmanager_accounts_containers_workspaces_tags_revert_task,
    tagmanager_accounts_containers_workspaces_tags_update_builder, tagmanager_accounts_containers_workspaces_tags_update_task,
    tagmanager_accounts_containers_workspaces_templates_create_builder, tagmanager_accounts_containers_workspaces_templates_create_task,
    tagmanager_accounts_containers_workspaces_templates_delete_builder, tagmanager_accounts_containers_workspaces_templates_delete_task,
    tagmanager_accounts_containers_workspaces_templates_get_builder, tagmanager_accounts_containers_workspaces_templates_get_task,
    tagmanager_accounts_containers_workspaces_templates_import_from_gallery_builder, tagmanager_accounts_containers_workspaces_templates_import_from_gallery_task,
    tagmanager_accounts_containers_workspaces_templates_list_builder, tagmanager_accounts_containers_workspaces_templates_list_task,
    tagmanager_accounts_containers_workspaces_templates_revert_builder, tagmanager_accounts_containers_workspaces_templates_revert_task,
    tagmanager_accounts_containers_workspaces_templates_update_builder, tagmanager_accounts_containers_workspaces_templates_update_task,
    tagmanager_accounts_containers_workspaces_transformations_create_builder, tagmanager_accounts_containers_workspaces_transformations_create_task,
    tagmanager_accounts_containers_workspaces_transformations_delete_builder, tagmanager_accounts_containers_workspaces_transformations_delete_task,
    tagmanager_accounts_containers_workspaces_transformations_get_builder, tagmanager_accounts_containers_workspaces_transformations_get_task,
    tagmanager_accounts_containers_workspaces_transformations_list_builder, tagmanager_accounts_containers_workspaces_transformations_list_task,
    tagmanager_accounts_containers_workspaces_transformations_revert_builder, tagmanager_accounts_containers_workspaces_transformations_revert_task,
    tagmanager_accounts_containers_workspaces_transformations_update_builder, tagmanager_accounts_containers_workspaces_transformations_update_task,
    tagmanager_accounts_containers_workspaces_triggers_create_builder, tagmanager_accounts_containers_workspaces_triggers_create_task,
    tagmanager_accounts_containers_workspaces_triggers_delete_builder, tagmanager_accounts_containers_workspaces_triggers_delete_task,
    tagmanager_accounts_containers_workspaces_triggers_get_builder, tagmanager_accounts_containers_workspaces_triggers_get_task,
    tagmanager_accounts_containers_workspaces_triggers_list_builder, tagmanager_accounts_containers_workspaces_triggers_list_task,
    tagmanager_accounts_containers_workspaces_triggers_revert_builder, tagmanager_accounts_containers_workspaces_triggers_revert_task,
    tagmanager_accounts_containers_workspaces_triggers_update_builder, tagmanager_accounts_containers_workspaces_triggers_update_task,
    tagmanager_accounts_containers_workspaces_variables_create_builder, tagmanager_accounts_containers_workspaces_variables_create_task,
    tagmanager_accounts_containers_workspaces_variables_delete_builder, tagmanager_accounts_containers_workspaces_variables_delete_task,
    tagmanager_accounts_containers_workspaces_variables_get_builder, tagmanager_accounts_containers_workspaces_variables_get_task,
    tagmanager_accounts_containers_workspaces_variables_list_builder, tagmanager_accounts_containers_workspaces_variables_list_task,
    tagmanager_accounts_containers_workspaces_variables_revert_builder, tagmanager_accounts_containers_workspaces_variables_revert_task,
    tagmanager_accounts_containers_workspaces_variables_update_builder, tagmanager_accounts_containers_workspaces_variables_update_task,
    tagmanager_accounts_containers_workspaces_zones_create_builder, tagmanager_accounts_containers_workspaces_zones_create_task,
    tagmanager_accounts_containers_workspaces_zones_delete_builder, tagmanager_accounts_containers_workspaces_zones_delete_task,
    tagmanager_accounts_containers_workspaces_zones_get_builder, tagmanager_accounts_containers_workspaces_zones_get_task,
    tagmanager_accounts_containers_workspaces_zones_list_builder, tagmanager_accounts_containers_workspaces_zones_list_task,
    tagmanager_accounts_containers_workspaces_zones_revert_builder, tagmanager_accounts_containers_workspaces_zones_revert_task,
    tagmanager_accounts_containers_workspaces_zones_update_builder, tagmanager_accounts_containers_workspaces_zones_update_task,
    tagmanager_accounts_user_permissions_create_builder, tagmanager_accounts_user_permissions_create_task,
    tagmanager_accounts_user_permissions_delete_builder, tagmanager_accounts_user_permissions_delete_task,
    tagmanager_accounts_user_permissions_get_builder, tagmanager_accounts_user_permissions_get_task,
    tagmanager_accounts_user_permissions_list_builder, tagmanager_accounts_user_permissions_list_task,
    tagmanager_accounts_user_permissions_update_builder, tagmanager_accounts_user_permissions_update_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::tagmanager::Account;
use crate::providers::gcp::clients::tagmanager::BulkUpdateWorkspaceResponse;
use crate::providers::gcp::clients::tagmanager::Client;
use crate::providers::gcp::clients::tagmanager::Container;
use crate::providers::gcp::clients::tagmanager::ContainerVersion;
use crate::providers::gcp::clients::tagmanager::ContainerVersionHeader;
use crate::providers::gcp::clients::tagmanager::CreateBuiltInVariableResponse;
use crate::providers::gcp::clients::tagmanager::CreateContainerVersionResponse;
use crate::providers::gcp::clients::tagmanager::CustomTemplate;
use crate::providers::gcp::clients::tagmanager::Destination;
use crate::providers::gcp::clients::tagmanager::Environment;
use crate::providers::gcp::clients::tagmanager::Folder;
use crate::providers::gcp::clients::tagmanager::FolderEntities;
use crate::providers::gcp::clients::tagmanager::GetContainerSnippetResponse;
use crate::providers::gcp::clients::tagmanager::GetWorkspaceStatusResponse;
use crate::providers::gcp::clients::tagmanager::GtagConfig;
use crate::providers::gcp::clients::tagmanager::ListAccountsResponse;
use crate::providers::gcp::clients::tagmanager::ListClientsResponse;
use crate::providers::gcp::clients::tagmanager::ListContainerVersionsResponse;
use crate::providers::gcp::clients::tagmanager::ListContainersResponse;
use crate::providers::gcp::clients::tagmanager::ListDestinationsResponse;
use crate::providers::gcp::clients::tagmanager::ListEnabledBuiltInVariablesResponse;
use crate::providers::gcp::clients::tagmanager::ListEnvironmentsResponse;
use crate::providers::gcp::clients::tagmanager::ListFoldersResponse;
use crate::providers::gcp::clients::tagmanager::ListGtagConfigResponse;
use crate::providers::gcp::clients::tagmanager::ListTagsResponse;
use crate::providers::gcp::clients::tagmanager::ListTemplatesResponse;
use crate::providers::gcp::clients::tagmanager::ListTransformationsResponse;
use crate::providers::gcp::clients::tagmanager::ListTriggersResponse;
use crate::providers::gcp::clients::tagmanager::ListUserPermissionsResponse;
use crate::providers::gcp::clients::tagmanager::ListVariablesResponse;
use crate::providers::gcp::clients::tagmanager::ListWorkspacesResponse;
use crate::providers::gcp::clients::tagmanager::ListZonesResponse;
use crate::providers::gcp::clients::tagmanager::PublishContainerVersionResponse;
use crate::providers::gcp::clients::tagmanager::QuickPreviewResponse;
use crate::providers::gcp::clients::tagmanager::RevertBuiltInVariableResponse;
use crate::providers::gcp::clients::tagmanager::RevertClientResponse;
use crate::providers::gcp::clients::tagmanager::RevertFolderResponse;
use crate::providers::gcp::clients::tagmanager::RevertTagResponse;
use crate::providers::gcp::clients::tagmanager::RevertTemplateResponse;
use crate::providers::gcp::clients::tagmanager::RevertTransformationResponse;
use crate::providers::gcp::clients::tagmanager::RevertTriggerResponse;
use crate::providers::gcp::clients::tagmanager::RevertVariableResponse;
use crate::providers::gcp::clients::tagmanager::RevertZoneResponse;
use crate::providers::gcp::clients::tagmanager::SyncWorkspaceResponse;
use crate::providers::gcp::clients::tagmanager::Tag;
use crate::providers::gcp::clients::tagmanager::Transformation;
use crate::providers::gcp::clients::tagmanager::Trigger;
use crate::providers::gcp::clients::tagmanager::UserPermission;
use crate::providers::gcp::clients::tagmanager::Variable;
use crate::providers::gcp::clients::tagmanager::Workspace;
use crate::providers::gcp::clients::tagmanager::Zone;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersCombineArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersCreateArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersDeleteArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersDestinationsGetArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersDestinationsLinkArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersDestinationsListArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersEnvironmentsCreateArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersEnvironmentsDeleteArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersEnvironmentsGetArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersEnvironmentsListArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersEnvironmentsReauthorizeArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersEnvironmentsUpdateArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersGetArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersListArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersLookupArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersMoveTagIdArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersSnippetArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersUpdateArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersVersionHeadersLatestArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersVersionHeadersListArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersVersionsDeleteArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersVersionsGetArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersVersionsLiveArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersVersionsPublishArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersVersionsSetLatestArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersVersionsUndeleteArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersVersionsUpdateArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesBuiltInVariablesCreateArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesBuiltInVariablesDeleteArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesBuiltInVariablesListArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesBuiltInVariablesRevertArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesBulkUpdateArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesClientsCreateArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesClientsDeleteArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesClientsGetArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesClientsListArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesClientsRevertArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesClientsUpdateArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesCreateArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesCreateVersionArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesDeleteArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesFoldersCreateArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesFoldersDeleteArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesFoldersEntitiesArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesFoldersGetArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesFoldersListArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesFoldersMoveEntitiesToFolderArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesFoldersRevertArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesFoldersUpdateArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesGetArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesGetStatusArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesGtagConfigCreateArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesGtagConfigDeleteArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesGtagConfigGetArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesGtagConfigListArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesGtagConfigUpdateArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesListArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesQuickPreviewArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesResolveConflictArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesSyncArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesTagsCreateArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesTagsDeleteArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesTagsGetArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesTagsListArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesTagsRevertArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesTagsUpdateArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesTemplatesCreateArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesTemplatesDeleteArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesTemplatesGetArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesTemplatesImportFromGalleryArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesTemplatesListArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesTemplatesRevertArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesTemplatesUpdateArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesTransformationsCreateArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesTransformationsDeleteArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesTransformationsGetArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesTransformationsListArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesTransformationsRevertArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesTransformationsUpdateArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesTriggersCreateArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesTriggersDeleteArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesTriggersGetArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesTriggersListArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesTriggersRevertArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesTriggersUpdateArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesUpdateArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesVariablesCreateArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesVariablesDeleteArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesVariablesGetArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesVariablesListArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesVariablesRevertArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesVariablesUpdateArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesZonesCreateArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesZonesDeleteArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesZonesGetArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesZonesListArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesZonesRevertArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsContainersWorkspacesZonesUpdateArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsGetArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsListArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsUpdateArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsUserPermissionsCreateArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsUserPermissionsDeleteArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsUserPermissionsGetArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsUserPermissionsListArgs;
use crate::providers::gcp::clients::tagmanager::TagmanagerAccountsUserPermissionsUpdateArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// TagmanagerProvider with automatic state tracking.
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
/// let provider = TagmanagerProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct TagmanagerProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> TagmanagerProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new TagmanagerProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Tagmanager accounts get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Account result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tagmanager_accounts_get(
        &self,
        args: &TagmanagerAccountsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Account, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_get_builder(
            &self.http_client,
            &args.path,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAccountsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tagmanager_accounts_list(
        &self,
        args: &TagmanagerAccountsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAccountsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_list_builder(
            &self.http_client,
            &args.includeGoogleTags,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Account result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_update(
        &self,
        args: &TagmanagerAccountsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Account, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_update_builder(
            &self.http_client,
            &args.path,
            &args.fingerprint,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers combine.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Container result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_containers_combine(
        &self,
        args: &TagmanagerAccountsContainersCombineArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Container, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_combine_builder(
            &self.http_client,
            &args.path,
            &args.allowUserPermissionFeatureUpdate,
            &args.containerId,
            &args.settingSource,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_combine_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Container result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_containers_create(
        &self,
        args: &TagmanagerAccountsContainersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Container, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers delete.
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
    pub fn tagmanager_accounts_containers_delete(
        &self,
        args: &TagmanagerAccountsContainersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_delete_builder(
            &self.http_client,
            &args.path,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Container result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tagmanager_accounts_containers_get(
        &self,
        args: &TagmanagerAccountsContainersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Container, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_get_builder(
            &self.http_client,
            &args.path,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListContainersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tagmanager_accounts_containers_list(
        &self,
        args: &TagmanagerAccountsContainersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListContainersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers lookup.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Container result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tagmanager_accounts_containers_lookup(
        &self,
        args: &TagmanagerAccountsContainersLookupArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Container, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_lookup_builder(
            &self.http_client,
            &args.destinationId,
            &args.tagId,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_lookup_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers move tag id.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Container result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_containers_move_tag_id(
        &self,
        args: &TagmanagerAccountsContainersMoveTagIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Container, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_move_tag_id_builder(
            &self.http_client,
            &args.path,
            &args.allowUserPermissionFeatureUpdate,
            &args.copySettings,
            &args.copyTermsOfService,
            &args.copyUsers,
            &args.tagId,
            &args.tagName,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_move_tag_id_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers snippet.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GetContainerSnippetResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tagmanager_accounts_containers_snippet(
        &self,
        args: &TagmanagerAccountsContainersSnippetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GetContainerSnippetResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_snippet_builder(
            &self.http_client,
            &args.path,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_snippet_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Container result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_containers_update(
        &self,
        args: &TagmanagerAccountsContainersUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Container, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_update_builder(
            &self.http_client,
            &args.path,
            &args.fingerprint,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers destinations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Destination result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tagmanager_accounts_containers_destinations_get(
        &self,
        args: &TagmanagerAccountsContainersDestinationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Destination, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_destinations_get_builder(
            &self.http_client,
            &args.path,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_destinations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers destinations link.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Destination result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_containers_destinations_link(
        &self,
        args: &TagmanagerAccountsContainersDestinationsLinkArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Destination, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_destinations_link_builder(
            &self.http_client,
            &args.parent,
            &args.allowUserPermissionFeatureUpdate,
            &args.destinationId,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_destinations_link_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers destinations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDestinationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tagmanager_accounts_containers_destinations_list(
        &self,
        args: &TagmanagerAccountsContainersDestinationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDestinationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_destinations_list_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_destinations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers environments create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Environment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_containers_environments_create(
        &self,
        args: &TagmanagerAccountsContainersEnvironmentsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Environment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_environments_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_environments_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers environments delete.
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
    pub fn tagmanager_accounts_containers_environments_delete(
        &self,
        args: &TagmanagerAccountsContainersEnvironmentsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_environments_delete_builder(
            &self.http_client,
            &args.path,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_environments_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers environments get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Environment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tagmanager_accounts_containers_environments_get(
        &self,
        args: &TagmanagerAccountsContainersEnvironmentsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Environment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_environments_get_builder(
            &self.http_client,
            &args.path,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_environments_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers environments list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListEnvironmentsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tagmanager_accounts_containers_environments_list(
        &self,
        args: &TagmanagerAccountsContainersEnvironmentsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListEnvironmentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_environments_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_environments_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers environments reauthorize.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Environment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_containers_environments_reauthorize(
        &self,
        args: &TagmanagerAccountsContainersEnvironmentsReauthorizeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Environment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_environments_reauthorize_builder(
            &self.http_client,
            &args.path,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_environments_reauthorize_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers environments update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Environment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_containers_environments_update(
        &self,
        args: &TagmanagerAccountsContainersEnvironmentsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Environment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_environments_update_builder(
            &self.http_client,
            &args.path,
            &args.fingerprint,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_environments_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers version headers latest.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ContainerVersionHeader result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tagmanager_accounts_containers_version_headers_latest(
        &self,
        args: &TagmanagerAccountsContainersVersionHeadersLatestArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ContainerVersionHeader, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_version_headers_latest_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_version_headers_latest_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers version headers list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListContainerVersionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tagmanager_accounts_containers_version_headers_list(
        &self,
        args: &TagmanagerAccountsContainersVersionHeadersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListContainerVersionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_version_headers_list_builder(
            &self.http_client,
            &args.parent,
            &args.includeDeleted,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_version_headers_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers versions delete.
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
    pub fn tagmanager_accounts_containers_versions_delete(
        &self,
        args: &TagmanagerAccountsContainersVersionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_versions_delete_builder(
            &self.http_client,
            &args.path,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_versions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers versions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ContainerVersion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tagmanager_accounts_containers_versions_get(
        &self,
        args: &TagmanagerAccountsContainersVersionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ContainerVersion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_versions_get_builder(
            &self.http_client,
            &args.path,
            &args.containerVersionId,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_versions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers versions live.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ContainerVersion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tagmanager_accounts_containers_versions_live(
        &self,
        args: &TagmanagerAccountsContainersVersionsLiveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ContainerVersion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_versions_live_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_versions_live_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers versions publish.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PublishContainerVersionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_containers_versions_publish(
        &self,
        args: &TagmanagerAccountsContainersVersionsPublishArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PublishContainerVersionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_versions_publish_builder(
            &self.http_client,
            &args.path,
            &args.fingerprint,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_versions_publish_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers versions set latest.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ContainerVersion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_containers_versions_set_latest(
        &self,
        args: &TagmanagerAccountsContainersVersionsSetLatestArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ContainerVersion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_versions_set_latest_builder(
            &self.http_client,
            &args.path,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_versions_set_latest_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers versions undelete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ContainerVersion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_containers_versions_undelete(
        &self,
        args: &TagmanagerAccountsContainersVersionsUndeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ContainerVersion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_versions_undelete_builder(
            &self.http_client,
            &args.path,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_versions_undelete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers versions update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ContainerVersion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_containers_versions_update(
        &self,
        args: &TagmanagerAccountsContainersVersionsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ContainerVersion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_versions_update_builder(
            &self.http_client,
            &args.path,
            &args.fingerprint,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_versions_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces bulk update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BulkUpdateWorkspaceResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_containers_workspaces_bulk_update(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesBulkUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BulkUpdateWorkspaceResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_bulk_update_builder(
            &self.http_client,
            &args.path,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_bulk_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Workspace result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_containers_workspaces_create(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Workspace, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces create version.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CreateContainerVersionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_containers_workspaces_create_version(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesCreateVersionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CreateContainerVersionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_create_version_builder(
            &self.http_client,
            &args.path,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_create_version_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces delete.
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
    pub fn tagmanager_accounts_containers_workspaces_delete(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_delete_builder(
            &self.http_client,
            &args.path,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Workspace result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tagmanager_accounts_containers_workspaces_get(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Workspace, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_get_builder(
            &self.http_client,
            &args.path,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces get status.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GetWorkspaceStatusResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tagmanager_accounts_containers_workspaces_get_status(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesGetStatusArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GetWorkspaceStatusResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_get_status_builder(
            &self.http_client,
            &args.path,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_get_status_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListWorkspacesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tagmanager_accounts_containers_workspaces_list(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListWorkspacesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces quick preview.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the QuickPreviewResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_containers_workspaces_quick_preview(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesQuickPreviewArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<QuickPreviewResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_quick_preview_builder(
            &self.http_client,
            &args.path,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_quick_preview_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces resolve conflict.
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
    pub fn tagmanager_accounts_containers_workspaces_resolve_conflict(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesResolveConflictArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_resolve_conflict_builder(
            &self.http_client,
            &args.path,
            &args.fingerprint,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_resolve_conflict_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces sync.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SyncWorkspaceResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_containers_workspaces_sync(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesSyncArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SyncWorkspaceResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_sync_builder(
            &self.http_client,
            &args.path,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_sync_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Workspace result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_containers_workspaces_update(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Workspace, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_update_builder(
            &self.http_client,
            &args.path,
            &args.fingerprint,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces built in variables create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CreateBuiltInVariableResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_containers_workspaces_built_in_variables_create(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesBuiltInVariablesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CreateBuiltInVariableResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_built_in_variables_create_builder(
            &self.http_client,
            &args.parent,
            &args.type_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_built_in_variables_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces built in variables delete.
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
    pub fn tagmanager_accounts_containers_workspaces_built_in_variables_delete(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesBuiltInVariablesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_built_in_variables_delete_builder(
            &self.http_client,
            &args.path,
            &args.type_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_built_in_variables_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces built in variables list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListEnabledBuiltInVariablesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tagmanager_accounts_containers_workspaces_built_in_variables_list(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesBuiltInVariablesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListEnabledBuiltInVariablesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_built_in_variables_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_built_in_variables_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces built in variables revert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RevertBuiltInVariableResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_containers_workspaces_built_in_variables_revert(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesBuiltInVariablesRevertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RevertBuiltInVariableResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_built_in_variables_revert_builder(
            &self.http_client,
            &args.path,
            &args.type_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_built_in_variables_revert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces clients create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Client result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_containers_workspaces_clients_create(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesClientsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Client, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_clients_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_clients_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces clients delete.
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
    pub fn tagmanager_accounts_containers_workspaces_clients_delete(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesClientsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_clients_delete_builder(
            &self.http_client,
            &args.path,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_clients_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces clients get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Client result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tagmanager_accounts_containers_workspaces_clients_get(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesClientsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Client, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_clients_get_builder(
            &self.http_client,
            &args.path,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_clients_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces clients list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListClientsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tagmanager_accounts_containers_workspaces_clients_list(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesClientsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListClientsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_clients_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_clients_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces clients revert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RevertClientResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_containers_workspaces_clients_revert(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesClientsRevertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RevertClientResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_clients_revert_builder(
            &self.http_client,
            &args.path,
            &args.fingerprint,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_clients_revert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces clients update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Client result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_containers_workspaces_clients_update(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesClientsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Client, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_clients_update_builder(
            &self.http_client,
            &args.path,
            &args.fingerprint,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_clients_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces folders create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Folder result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_containers_workspaces_folders_create(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesFoldersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Folder, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_folders_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_folders_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces folders delete.
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
    pub fn tagmanager_accounts_containers_workspaces_folders_delete(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesFoldersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_folders_delete_builder(
            &self.http_client,
            &args.path,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_folders_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces folders entities.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FolderEntities result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_containers_workspaces_folders_entities(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesFoldersEntitiesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FolderEntities, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_folders_entities_builder(
            &self.http_client,
            &args.path,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_folders_entities_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces folders get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Folder result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tagmanager_accounts_containers_workspaces_folders_get(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesFoldersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Folder, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_folders_get_builder(
            &self.http_client,
            &args.path,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_folders_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces folders list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListFoldersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tagmanager_accounts_containers_workspaces_folders_list(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesFoldersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListFoldersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_folders_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_folders_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces folders move entities to folder.
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
    pub fn tagmanager_accounts_containers_workspaces_folders_move_entities_to_folder(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesFoldersMoveEntitiesToFolderArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_folders_move_entities_to_folder_builder(
            &self.http_client,
            &args.path,
            &args.tagId,
            &args.triggerId,
            &args.variableId,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_folders_move_entities_to_folder_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces folders revert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RevertFolderResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_containers_workspaces_folders_revert(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesFoldersRevertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RevertFolderResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_folders_revert_builder(
            &self.http_client,
            &args.path,
            &args.fingerprint,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_folders_revert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces folders update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Folder result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_containers_workspaces_folders_update(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesFoldersUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Folder, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_folders_update_builder(
            &self.http_client,
            &args.path,
            &args.fingerprint,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_folders_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces gtag config create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GtagConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_containers_workspaces_gtag_config_create(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesGtagConfigCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GtagConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_gtag_config_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_gtag_config_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces gtag config delete.
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
    pub fn tagmanager_accounts_containers_workspaces_gtag_config_delete(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesGtagConfigDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_gtag_config_delete_builder(
            &self.http_client,
            &args.path,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_gtag_config_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces gtag config get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GtagConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tagmanager_accounts_containers_workspaces_gtag_config_get(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesGtagConfigGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GtagConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_gtag_config_get_builder(
            &self.http_client,
            &args.path,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_gtag_config_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces gtag config list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListGtagConfigResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tagmanager_accounts_containers_workspaces_gtag_config_list(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesGtagConfigListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListGtagConfigResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_gtag_config_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_gtag_config_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces gtag config update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GtagConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_containers_workspaces_gtag_config_update(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesGtagConfigUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GtagConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_gtag_config_update_builder(
            &self.http_client,
            &args.path,
            &args.fingerprint,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_gtag_config_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces tags create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Tag result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_containers_workspaces_tags_create(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesTagsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Tag, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_tags_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_tags_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces tags delete.
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
    pub fn tagmanager_accounts_containers_workspaces_tags_delete(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesTagsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_tags_delete_builder(
            &self.http_client,
            &args.path,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_tags_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces tags get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Tag result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tagmanager_accounts_containers_workspaces_tags_get(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesTagsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Tag, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_tags_get_builder(
            &self.http_client,
            &args.path,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_tags_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces tags list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListTagsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tagmanager_accounts_containers_workspaces_tags_list(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesTagsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListTagsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_tags_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_tags_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces tags revert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RevertTagResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_containers_workspaces_tags_revert(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesTagsRevertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RevertTagResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_tags_revert_builder(
            &self.http_client,
            &args.path,
            &args.fingerprint,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_tags_revert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces tags update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Tag result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_containers_workspaces_tags_update(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesTagsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Tag, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_tags_update_builder(
            &self.http_client,
            &args.path,
            &args.fingerprint,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_tags_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces templates create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CustomTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_containers_workspaces_templates_create(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesTemplatesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CustomTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_templates_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_templates_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces templates delete.
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
    pub fn tagmanager_accounts_containers_workspaces_templates_delete(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesTemplatesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_templates_delete_builder(
            &self.http_client,
            &args.path,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_templates_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces templates get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CustomTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tagmanager_accounts_containers_workspaces_templates_get(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesTemplatesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CustomTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_templates_get_builder(
            &self.http_client,
            &args.path,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_templates_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces templates import from gallery.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CustomTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_containers_workspaces_templates_import_from_gallery(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesTemplatesImportFromGalleryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CustomTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_templates_import_from_gallery_builder(
            &self.http_client,
            &args.parent,
            &args.acknowledgePermissions,
            &args.galleryOwner,
            &args.galleryRepository,
            &args.gallerySha,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_templates_import_from_gallery_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces templates list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListTemplatesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tagmanager_accounts_containers_workspaces_templates_list(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesTemplatesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListTemplatesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_templates_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_templates_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces templates revert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RevertTemplateResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_containers_workspaces_templates_revert(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesTemplatesRevertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RevertTemplateResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_templates_revert_builder(
            &self.http_client,
            &args.path,
            &args.fingerprint,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_templates_revert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces templates update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CustomTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_containers_workspaces_templates_update(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesTemplatesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CustomTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_templates_update_builder(
            &self.http_client,
            &args.path,
            &args.fingerprint,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_templates_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces transformations create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Transformation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_containers_workspaces_transformations_create(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesTransformationsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Transformation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_transformations_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_transformations_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces transformations delete.
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
    pub fn tagmanager_accounts_containers_workspaces_transformations_delete(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesTransformationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_transformations_delete_builder(
            &self.http_client,
            &args.path,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_transformations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces transformations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Transformation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tagmanager_accounts_containers_workspaces_transformations_get(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesTransformationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Transformation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_transformations_get_builder(
            &self.http_client,
            &args.path,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_transformations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces transformations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListTransformationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tagmanager_accounts_containers_workspaces_transformations_list(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesTransformationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListTransformationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_transformations_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_transformations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces transformations revert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RevertTransformationResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_containers_workspaces_transformations_revert(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesTransformationsRevertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RevertTransformationResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_transformations_revert_builder(
            &self.http_client,
            &args.path,
            &args.fingerprint,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_transformations_revert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces transformations update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Transformation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_containers_workspaces_transformations_update(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesTransformationsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Transformation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_transformations_update_builder(
            &self.http_client,
            &args.path,
            &args.fingerprint,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_transformations_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces triggers create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Trigger result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_containers_workspaces_triggers_create(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesTriggersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Trigger, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_triggers_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_triggers_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces triggers delete.
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
    pub fn tagmanager_accounts_containers_workspaces_triggers_delete(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesTriggersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_triggers_delete_builder(
            &self.http_client,
            &args.path,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_triggers_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces triggers get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Trigger result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tagmanager_accounts_containers_workspaces_triggers_get(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesTriggersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Trigger, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_triggers_get_builder(
            &self.http_client,
            &args.path,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_triggers_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces triggers list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListTriggersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tagmanager_accounts_containers_workspaces_triggers_list(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesTriggersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListTriggersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_triggers_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_triggers_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces triggers revert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RevertTriggerResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_containers_workspaces_triggers_revert(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesTriggersRevertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RevertTriggerResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_triggers_revert_builder(
            &self.http_client,
            &args.path,
            &args.fingerprint,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_triggers_revert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces triggers update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Trigger result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_containers_workspaces_triggers_update(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesTriggersUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Trigger, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_triggers_update_builder(
            &self.http_client,
            &args.path,
            &args.fingerprint,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_triggers_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces variables create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Variable result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_containers_workspaces_variables_create(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesVariablesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Variable, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_variables_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_variables_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces variables delete.
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
    pub fn tagmanager_accounts_containers_workspaces_variables_delete(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesVariablesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_variables_delete_builder(
            &self.http_client,
            &args.path,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_variables_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces variables get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Variable result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tagmanager_accounts_containers_workspaces_variables_get(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesVariablesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Variable, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_variables_get_builder(
            &self.http_client,
            &args.path,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_variables_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces variables list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListVariablesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tagmanager_accounts_containers_workspaces_variables_list(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesVariablesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListVariablesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_variables_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_variables_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces variables revert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RevertVariableResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_containers_workspaces_variables_revert(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesVariablesRevertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RevertVariableResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_variables_revert_builder(
            &self.http_client,
            &args.path,
            &args.fingerprint,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_variables_revert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces variables update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Variable result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_containers_workspaces_variables_update(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesVariablesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Variable, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_variables_update_builder(
            &self.http_client,
            &args.path,
            &args.fingerprint,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_variables_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces zones create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Zone result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_containers_workspaces_zones_create(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesZonesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Zone, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_zones_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_zones_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces zones delete.
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
    pub fn tagmanager_accounts_containers_workspaces_zones_delete(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesZonesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_zones_delete_builder(
            &self.http_client,
            &args.path,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_zones_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces zones get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Zone result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tagmanager_accounts_containers_workspaces_zones_get(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesZonesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Zone, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_zones_get_builder(
            &self.http_client,
            &args.path,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_zones_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces zones list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListZonesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tagmanager_accounts_containers_workspaces_zones_list(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesZonesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListZonesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_zones_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_zones_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces zones revert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RevertZoneResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_containers_workspaces_zones_revert(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesZonesRevertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RevertZoneResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_zones_revert_builder(
            &self.http_client,
            &args.path,
            &args.fingerprint,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_zones_revert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts containers workspaces zones update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Zone result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_containers_workspaces_zones_update(
        &self,
        args: &TagmanagerAccountsContainersWorkspacesZonesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Zone, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_containers_workspaces_zones_update_builder(
            &self.http_client,
            &args.path,
            &args.fingerprint,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_containers_workspaces_zones_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts user permissions create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UserPermission result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_user_permissions_create(
        &self,
        args: &TagmanagerAccountsUserPermissionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UserPermission, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_user_permissions_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_user_permissions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts user permissions delete.
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
    pub fn tagmanager_accounts_user_permissions_delete(
        &self,
        args: &TagmanagerAccountsUserPermissionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_user_permissions_delete_builder(
            &self.http_client,
            &args.path,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_user_permissions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts user permissions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UserPermission result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tagmanager_accounts_user_permissions_get(
        &self,
        args: &TagmanagerAccountsUserPermissionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UserPermission, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_user_permissions_get_builder(
            &self.http_client,
            &args.path,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_user_permissions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts user permissions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListUserPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tagmanager_accounts_user_permissions_list(
        &self,
        args: &TagmanagerAccountsUserPermissionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListUserPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_user_permissions_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_user_permissions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tagmanager accounts user permissions update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UserPermission result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tagmanager_accounts_user_permissions_update(
        &self,
        args: &TagmanagerAccountsUserPermissionsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UserPermission, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tagmanager_accounts_user_permissions_update_builder(
            &self.http_client,
            &args.path,
        )
        .map_err(ProviderError::Api)?;

        let task = tagmanager_accounts_user_permissions_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
