//! ArtifactregistryProvider - State-aware artifactregistry API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       artifactregistry API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::artifactregistry::{
    artifactregistry_projects_get_project_settings_builder, artifactregistry_projects_get_project_settings_task,
    artifactregistry_projects_update_project_settings_builder, artifactregistry_projects_update_project_settings_task,
    artifactregistry_projects_locations_get_builder, artifactregistry_projects_locations_get_task,
    artifactregistry_projects_locations_get_project_config_builder, artifactregistry_projects_locations_get_project_config_task,
    artifactregistry_projects_locations_get_vpcsc_config_builder, artifactregistry_projects_locations_get_vpcsc_config_task,
    artifactregistry_projects_locations_list_builder, artifactregistry_projects_locations_list_task,
    artifactregistry_projects_locations_update_project_config_builder, artifactregistry_projects_locations_update_project_config_task,
    artifactregistry_projects_locations_update_vpcsc_config_builder, artifactregistry_projects_locations_update_vpcsc_config_task,
    artifactregistry_projects_locations_operations_cancel_builder, artifactregistry_projects_locations_operations_cancel_task,
    artifactregistry_projects_locations_operations_get_builder, artifactregistry_projects_locations_operations_get_task,
    artifactregistry_projects_locations_repositories_create_builder, artifactregistry_projects_locations_repositories_create_task,
    artifactregistry_projects_locations_repositories_delete_builder, artifactregistry_projects_locations_repositories_delete_task,
    artifactregistry_projects_locations_repositories_export_artifact_builder, artifactregistry_projects_locations_repositories_export_artifact_task,
    artifactregistry_projects_locations_repositories_get_builder, artifactregistry_projects_locations_repositories_get_task,
    artifactregistry_projects_locations_repositories_get_iam_policy_builder, artifactregistry_projects_locations_repositories_get_iam_policy_task,
    artifactregistry_projects_locations_repositories_list_builder, artifactregistry_projects_locations_repositories_list_task,
    artifactregistry_projects_locations_repositories_patch_builder, artifactregistry_projects_locations_repositories_patch_task,
    artifactregistry_projects_locations_repositories_set_iam_policy_builder, artifactregistry_projects_locations_repositories_set_iam_policy_task,
    artifactregistry_projects_locations_repositories_test_iam_permissions_builder, artifactregistry_projects_locations_repositories_test_iam_permissions_task,
    artifactregistry_projects_locations_repositories_apt_artifacts_import_builder, artifactregistry_projects_locations_repositories_apt_artifacts_import_task,
    artifactregistry_projects_locations_repositories_apt_artifacts_upload_builder, artifactregistry_projects_locations_repositories_apt_artifacts_upload_task,
    artifactregistry_projects_locations_repositories_attachments_create_builder, artifactregistry_projects_locations_repositories_attachments_create_task,
    artifactregistry_projects_locations_repositories_attachments_delete_builder, artifactregistry_projects_locations_repositories_attachments_delete_task,
    artifactregistry_projects_locations_repositories_attachments_get_builder, artifactregistry_projects_locations_repositories_attachments_get_task,
    artifactregistry_projects_locations_repositories_attachments_list_builder, artifactregistry_projects_locations_repositories_attachments_list_task,
    artifactregistry_projects_locations_repositories_docker_images_get_builder, artifactregistry_projects_locations_repositories_docker_images_get_task,
    artifactregistry_projects_locations_repositories_docker_images_list_builder, artifactregistry_projects_locations_repositories_docker_images_list_task,
    artifactregistry_projects_locations_repositories_files_delete_builder, artifactregistry_projects_locations_repositories_files_delete_task,
    artifactregistry_projects_locations_repositories_files_download_builder, artifactregistry_projects_locations_repositories_files_download_task,
    artifactregistry_projects_locations_repositories_files_get_builder, artifactregistry_projects_locations_repositories_files_get_task,
    artifactregistry_projects_locations_repositories_files_list_builder, artifactregistry_projects_locations_repositories_files_list_task,
    artifactregistry_projects_locations_repositories_files_patch_builder, artifactregistry_projects_locations_repositories_files_patch_task,
    artifactregistry_projects_locations_repositories_files_upload_builder, artifactregistry_projects_locations_repositories_files_upload_task,
    artifactregistry_projects_locations_repositories_generic_artifacts_upload_builder, artifactregistry_projects_locations_repositories_generic_artifacts_upload_task,
    artifactregistry_projects_locations_repositories_go_modules_upload_builder, artifactregistry_projects_locations_repositories_go_modules_upload_task,
    artifactregistry_projects_locations_repositories_googet_artifacts_import_builder, artifactregistry_projects_locations_repositories_googet_artifacts_import_task,
    artifactregistry_projects_locations_repositories_googet_artifacts_upload_builder, artifactregistry_projects_locations_repositories_googet_artifacts_upload_task,
    artifactregistry_projects_locations_repositories_kfp_artifacts_upload_builder, artifactregistry_projects_locations_repositories_kfp_artifacts_upload_task,
    artifactregistry_projects_locations_repositories_maven_artifacts_get_builder, artifactregistry_projects_locations_repositories_maven_artifacts_get_task,
    artifactregistry_projects_locations_repositories_maven_artifacts_list_builder, artifactregistry_projects_locations_repositories_maven_artifacts_list_task,
    artifactregistry_projects_locations_repositories_npm_packages_get_builder, artifactregistry_projects_locations_repositories_npm_packages_get_task,
    artifactregistry_projects_locations_repositories_npm_packages_list_builder, artifactregistry_projects_locations_repositories_npm_packages_list_task,
    artifactregistry_projects_locations_repositories_packages_delete_builder, artifactregistry_projects_locations_repositories_packages_delete_task,
    artifactregistry_projects_locations_repositories_packages_get_builder, artifactregistry_projects_locations_repositories_packages_get_task,
    artifactregistry_projects_locations_repositories_packages_list_builder, artifactregistry_projects_locations_repositories_packages_list_task,
    artifactregistry_projects_locations_repositories_packages_patch_builder, artifactregistry_projects_locations_repositories_packages_patch_task,
    artifactregistry_projects_locations_repositories_packages_tags_create_builder, artifactregistry_projects_locations_repositories_packages_tags_create_task,
    artifactregistry_projects_locations_repositories_packages_tags_delete_builder, artifactregistry_projects_locations_repositories_packages_tags_delete_task,
    artifactregistry_projects_locations_repositories_packages_tags_get_builder, artifactregistry_projects_locations_repositories_packages_tags_get_task,
    artifactregistry_projects_locations_repositories_packages_tags_list_builder, artifactregistry_projects_locations_repositories_packages_tags_list_task,
    artifactregistry_projects_locations_repositories_packages_tags_patch_builder, artifactregistry_projects_locations_repositories_packages_tags_patch_task,
    artifactregistry_projects_locations_repositories_packages_versions_batch_delete_builder, artifactregistry_projects_locations_repositories_packages_versions_batch_delete_task,
    artifactregistry_projects_locations_repositories_packages_versions_delete_builder, artifactregistry_projects_locations_repositories_packages_versions_delete_task,
    artifactregistry_projects_locations_repositories_packages_versions_get_builder, artifactregistry_projects_locations_repositories_packages_versions_get_task,
    artifactregistry_projects_locations_repositories_packages_versions_list_builder, artifactregistry_projects_locations_repositories_packages_versions_list_task,
    artifactregistry_projects_locations_repositories_packages_versions_patch_builder, artifactregistry_projects_locations_repositories_packages_versions_patch_task,
    artifactregistry_projects_locations_repositories_python_packages_get_builder, artifactregistry_projects_locations_repositories_python_packages_get_task,
    artifactregistry_projects_locations_repositories_python_packages_list_builder, artifactregistry_projects_locations_repositories_python_packages_list_task,
    artifactregistry_projects_locations_repositories_rules_create_builder, artifactregistry_projects_locations_repositories_rules_create_task,
    artifactregistry_projects_locations_repositories_rules_delete_builder, artifactregistry_projects_locations_repositories_rules_delete_task,
    artifactregistry_projects_locations_repositories_rules_get_builder, artifactregistry_projects_locations_repositories_rules_get_task,
    artifactregistry_projects_locations_repositories_rules_list_builder, artifactregistry_projects_locations_repositories_rules_list_task,
    artifactregistry_projects_locations_repositories_rules_patch_builder, artifactregistry_projects_locations_repositories_rules_patch_task,
    artifactregistry_projects_locations_repositories_yum_artifacts_import_builder, artifactregistry_projects_locations_repositories_yum_artifacts_import_task,
    artifactregistry_projects_locations_repositories_yum_artifacts_upload_builder, artifactregistry_projects_locations_repositories_yum_artifacts_upload_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::artifactregistry::Attachment;
use crate::providers::gcp::clients::artifactregistry::DockerImage;
use crate::providers::gcp::clients::artifactregistry::DownloadFileResponse;
use crate::providers::gcp::clients::artifactregistry::Empty;
use crate::providers::gcp::clients::artifactregistry::GoogleDevtoolsArtifactregistryV1File;
use crate::providers::gcp::clients::artifactregistry::GoogleDevtoolsArtifactregistryV1Rule;
use crate::providers::gcp::clients::artifactregistry::ListAttachmentsResponse;
use crate::providers::gcp::clients::artifactregistry::ListDockerImagesResponse;
use crate::providers::gcp::clients::artifactregistry::ListFilesResponse;
use crate::providers::gcp::clients::artifactregistry::ListLocationsResponse;
use crate::providers::gcp::clients::artifactregistry::ListMavenArtifactsResponse;
use crate::providers::gcp::clients::artifactregistry::ListNpmPackagesResponse;
use crate::providers::gcp::clients::artifactregistry::ListPackagesResponse;
use crate::providers::gcp::clients::artifactregistry::ListPythonPackagesResponse;
use crate::providers::gcp::clients::artifactregistry::ListRepositoriesResponse;
use crate::providers::gcp::clients::artifactregistry::ListRulesResponse;
use crate::providers::gcp::clients::artifactregistry::ListTagsResponse;
use crate::providers::gcp::clients::artifactregistry::ListVersionsResponse;
use crate::providers::gcp::clients::artifactregistry::Location;
use crate::providers::gcp::clients::artifactregistry::MavenArtifact;
use crate::providers::gcp::clients::artifactregistry::NpmPackage;
use crate::providers::gcp::clients::artifactregistry::Operation;
use crate::providers::gcp::clients::artifactregistry::Package;
use crate::providers::gcp::clients::artifactregistry::Policy;
use crate::providers::gcp::clients::artifactregistry::ProjectConfig;
use crate::providers::gcp::clients::artifactregistry::ProjectSettings;
use crate::providers::gcp::clients::artifactregistry::PythonPackage;
use crate::providers::gcp::clients::artifactregistry::Repository;
use crate::providers::gcp::clients::artifactregistry::Tag;
use crate::providers::gcp::clients::artifactregistry::TestIamPermissionsResponse;
use crate::providers::gcp::clients::artifactregistry::UploadAptArtifactMediaResponse;
use crate::providers::gcp::clients::artifactregistry::UploadFileMediaResponse;
use crate::providers::gcp::clients::artifactregistry::UploadGenericArtifactMediaResponse;
use crate::providers::gcp::clients::artifactregistry::UploadGoModuleMediaResponse;
use crate::providers::gcp::clients::artifactregistry::UploadGoogetArtifactMediaResponse;
use crate::providers::gcp::clients::artifactregistry::UploadKfpArtifactMediaResponse;
use crate::providers::gcp::clients::artifactregistry::UploadYumArtifactMediaResponse;
use crate::providers::gcp::clients::artifactregistry::VPCSCConfig;
use crate::providers::gcp::clients::artifactregistry::Version;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsGetProjectSettingsArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsGetArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsGetProjectConfigArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsGetVpcscConfigArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsListArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesAptArtifactsImportArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesAptArtifactsUploadArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesAttachmentsCreateArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesAttachmentsDeleteArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesAttachmentsGetArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesAttachmentsListArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesCreateArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesDeleteArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesDockerImagesGetArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesDockerImagesListArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesExportArtifactArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesFilesDeleteArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesFilesDownloadArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesFilesGetArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesFilesListArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesFilesPatchArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesFilesUploadArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesGenericArtifactsUploadArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesGetArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesGetIamPolicyArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesGoModulesUploadArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesGoogetArtifactsImportArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesGoogetArtifactsUploadArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesKfpArtifactsUploadArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesListArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesMavenArtifactsGetArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesMavenArtifactsListArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesNpmPackagesGetArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesNpmPackagesListArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesPackagesDeleteArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesPackagesGetArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesPackagesListArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesPackagesPatchArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesPackagesTagsCreateArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesPackagesTagsDeleteArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesPackagesTagsGetArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesPackagesTagsListArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesPackagesTagsPatchArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesPackagesVersionsBatchDeleteArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesPackagesVersionsDeleteArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesPackagesVersionsGetArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesPackagesVersionsListArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesPackagesVersionsPatchArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesPatchArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesPythonPackagesGetArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesPythonPackagesListArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesRulesCreateArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesRulesDeleteArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesRulesGetArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesRulesListArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesRulesPatchArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesSetIamPolicyArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesTestIamPermissionsArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesYumArtifactsImportArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesYumArtifactsUploadArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsUpdateProjectConfigArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsUpdateVpcscConfigArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsUpdateProjectSettingsArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// ArtifactregistryProvider with automatic state tracking.
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
/// let provider = ArtifactregistryProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct ArtifactregistryProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> ArtifactregistryProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new ArtifactregistryProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Artifactregistry projects get project settings.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProjectSettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn artifactregistry_projects_get_project_settings(
        &self,
        args: &ArtifactregistryProjectsGetProjectSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProjectSettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_get_project_settings_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_get_project_settings_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects update project settings.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProjectSettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn artifactregistry_projects_update_project_settings(
        &self,
        args: &ArtifactregistryProjectsUpdateProjectSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProjectSettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_update_project_settings_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_update_project_settings_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Location result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn artifactregistry_projects_locations_get(
        &self,
        args: &ArtifactregistryProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations get project config.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProjectConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn artifactregistry_projects_locations_get_project_config(
        &self,
        args: &ArtifactregistryProjectsLocationsGetProjectConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProjectConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_get_project_config_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_get_project_config_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations get vpcsc config.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the VPCSCConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn artifactregistry_projects_locations_get_vpcsc_config(
        &self,
        args: &ArtifactregistryProjectsLocationsGetVpcscConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<VPCSCConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_get_vpcsc_config_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_get_vpcsc_config_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListLocationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn artifactregistry_projects_locations_list(
        &self,
        args: &ArtifactregistryProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations update project config.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProjectConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn artifactregistry_projects_locations_update_project_config(
        &self,
        args: &ArtifactregistryProjectsLocationsUpdateProjectConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProjectConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_update_project_config_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_update_project_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations update vpcsc config.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the VPCSCConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn artifactregistry_projects_locations_update_vpcsc_config(
        &self,
        args: &ArtifactregistryProjectsLocationsUpdateVpcscConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<VPCSCConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_update_vpcsc_config_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_update_vpcsc_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations operations cancel.
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
    pub fn artifactregistry_projects_locations_operations_cancel(
        &self,
        args: &ArtifactregistryProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations operations get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn artifactregistry_projects_locations_operations_get(
        &self,
        args: &ArtifactregistryProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories create.
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
    pub fn artifactregistry_projects_locations_repositories_create(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_create_builder(
            &self.http_client,
            &args.parent,
            &args.repositoryId,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories delete.
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
    pub fn artifactregistry_projects_locations_repositories_delete(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories export artifact.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn artifactregistry_projects_locations_repositories_export_artifact(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesExportArtifactArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_export_artifact_builder(
            &self.http_client,
            &args.repository,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_export_artifact_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Repository result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn artifactregistry_projects_locations_repositories_get(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Repository, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories get iam policy.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn artifactregistry_projects_locations_repositories_get_iam_policy(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListRepositoriesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn artifactregistry_projects_locations_repositories_list(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListRepositoriesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Repository result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn artifactregistry_projects_locations_repositories_patch(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Repository, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories set iam policy.
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
    pub fn artifactregistry_projects_locations_repositories_set_iam_policy(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories test iam permissions.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn artifactregistry_projects_locations_repositories_test_iam_permissions(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories apt artifacts import.
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
    pub fn artifactregistry_projects_locations_repositories_apt_artifacts_import(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesAptArtifactsImportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_apt_artifacts_import_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_apt_artifacts_import_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories apt artifacts upload.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UploadAptArtifactMediaResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn artifactregistry_projects_locations_repositories_apt_artifacts_upload(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesAptArtifactsUploadArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UploadAptArtifactMediaResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_apt_artifacts_upload_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_apt_artifacts_upload_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories attachments create.
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
    pub fn artifactregistry_projects_locations_repositories_attachments_create(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesAttachmentsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_attachments_create_builder(
            &self.http_client,
            &args.parent,
            &args.attachmentId,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_attachments_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories attachments delete.
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
    pub fn artifactregistry_projects_locations_repositories_attachments_delete(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesAttachmentsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_attachments_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_attachments_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories attachments get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Attachment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn artifactregistry_projects_locations_repositories_attachments_get(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesAttachmentsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Attachment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_attachments_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_attachments_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories attachments list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAttachmentsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn artifactregistry_projects_locations_repositories_attachments_list(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesAttachmentsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAttachmentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_attachments_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_attachments_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories docker images get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DockerImage result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn artifactregistry_projects_locations_repositories_docker_images_get(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesDockerImagesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DockerImage, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_docker_images_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_docker_images_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories docker images list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDockerImagesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn artifactregistry_projects_locations_repositories_docker_images_list(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesDockerImagesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDockerImagesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_docker_images_list_builder(
            &self.http_client,
            &args.parent,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_docker_images_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories files delete.
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
    pub fn artifactregistry_projects_locations_repositories_files_delete(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesFilesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_files_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_files_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories files download.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DownloadFileResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn artifactregistry_projects_locations_repositories_files_download(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesFilesDownloadArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DownloadFileResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_files_download_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_files_download_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories files get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleDevtoolsArtifactregistryV1File result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn artifactregistry_projects_locations_repositories_files_get(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesFilesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleDevtoolsArtifactregistryV1File, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_files_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_files_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories files list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListFilesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn artifactregistry_projects_locations_repositories_files_list(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesFilesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListFilesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_files_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_files_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories files patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleDevtoolsArtifactregistryV1File result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn artifactregistry_projects_locations_repositories_files_patch(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesFilesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleDevtoolsArtifactregistryV1File, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_files_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_files_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories files upload.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UploadFileMediaResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn artifactregistry_projects_locations_repositories_files_upload(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesFilesUploadArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UploadFileMediaResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_files_upload_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_files_upload_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories generic artifacts upload.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UploadGenericArtifactMediaResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn artifactregistry_projects_locations_repositories_generic_artifacts_upload(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesGenericArtifactsUploadArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UploadGenericArtifactMediaResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_generic_artifacts_upload_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_generic_artifacts_upload_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories go modules upload.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UploadGoModuleMediaResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn artifactregistry_projects_locations_repositories_go_modules_upload(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesGoModulesUploadArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UploadGoModuleMediaResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_go_modules_upload_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_go_modules_upload_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories googet artifacts import.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn artifactregistry_projects_locations_repositories_googet_artifacts_import(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesGoogetArtifactsImportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_googet_artifacts_import_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_googet_artifacts_import_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories googet artifacts upload.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UploadGoogetArtifactMediaResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn artifactregistry_projects_locations_repositories_googet_artifacts_upload(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesGoogetArtifactsUploadArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UploadGoogetArtifactMediaResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_googet_artifacts_upload_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_googet_artifacts_upload_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories kfp artifacts upload.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UploadKfpArtifactMediaResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn artifactregistry_projects_locations_repositories_kfp_artifacts_upload(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesKfpArtifactsUploadArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UploadKfpArtifactMediaResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_kfp_artifacts_upload_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_kfp_artifacts_upload_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories maven artifacts get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the MavenArtifact result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn artifactregistry_projects_locations_repositories_maven_artifacts_get(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesMavenArtifactsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<MavenArtifact, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_maven_artifacts_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_maven_artifacts_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories maven artifacts list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListMavenArtifactsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn artifactregistry_projects_locations_repositories_maven_artifacts_list(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesMavenArtifactsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListMavenArtifactsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_maven_artifacts_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_maven_artifacts_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories npm packages get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NpmPackage result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn artifactregistry_projects_locations_repositories_npm_packages_get(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesNpmPackagesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NpmPackage, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_npm_packages_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_npm_packages_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories npm packages list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListNpmPackagesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn artifactregistry_projects_locations_repositories_npm_packages_list(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesNpmPackagesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListNpmPackagesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_npm_packages_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_npm_packages_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories packages delete.
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
    pub fn artifactregistry_projects_locations_repositories_packages_delete(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesPackagesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_packages_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_packages_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories packages get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Package result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn artifactregistry_projects_locations_repositories_packages_get(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesPackagesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Package, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_packages_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_packages_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories packages list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListPackagesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn artifactregistry_projects_locations_repositories_packages_list(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesPackagesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListPackagesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_packages_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_packages_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories packages patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Package result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn artifactregistry_projects_locations_repositories_packages_patch(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesPackagesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Package, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_packages_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_packages_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories packages tags create.
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
    pub fn artifactregistry_projects_locations_repositories_packages_tags_create(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesPackagesTagsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Tag, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_packages_tags_create_builder(
            &self.http_client,
            &args.parent,
            &args.tagId,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_packages_tags_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories packages tags delete.
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
    pub fn artifactregistry_projects_locations_repositories_packages_tags_delete(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesPackagesTagsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_packages_tags_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_packages_tags_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories packages tags get.
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
    pub fn artifactregistry_projects_locations_repositories_packages_tags_get(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesPackagesTagsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Tag, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_packages_tags_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_packages_tags_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories packages tags list.
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
    pub fn artifactregistry_projects_locations_repositories_packages_tags_list(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesPackagesTagsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListTagsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_packages_tags_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_packages_tags_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories packages tags patch.
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
    pub fn artifactregistry_projects_locations_repositories_packages_tags_patch(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesPackagesTagsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Tag, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_packages_tags_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_packages_tags_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories packages versions batch delete.
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
    pub fn artifactregistry_projects_locations_repositories_packages_versions_batch_delete(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesPackagesVersionsBatchDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_packages_versions_batch_delete_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_packages_versions_batch_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories packages versions delete.
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
    pub fn artifactregistry_projects_locations_repositories_packages_versions_delete(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesPackagesVersionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_packages_versions_delete_builder(
            &self.http_client,
            &args.name,
            &args.force,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_packages_versions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories packages versions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Version result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn artifactregistry_projects_locations_repositories_packages_versions_get(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesPackagesVersionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Version, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_packages_versions_get_builder(
            &self.http_client,
            &args.name,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_packages_versions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories packages versions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListVersionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn artifactregistry_projects_locations_repositories_packages_versions_list(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesPackagesVersionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListVersionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_packages_versions_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_packages_versions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories packages versions patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Version result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn artifactregistry_projects_locations_repositories_packages_versions_patch(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesPackagesVersionsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Version, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_packages_versions_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_packages_versions_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories python packages get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PythonPackage result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn artifactregistry_projects_locations_repositories_python_packages_get(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesPythonPackagesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PythonPackage, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_python_packages_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_python_packages_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories python packages list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListPythonPackagesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn artifactregistry_projects_locations_repositories_python_packages_list(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesPythonPackagesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListPythonPackagesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_python_packages_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_python_packages_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories rules create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleDevtoolsArtifactregistryV1Rule result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn artifactregistry_projects_locations_repositories_rules_create(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesRulesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleDevtoolsArtifactregistryV1Rule, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_rules_create_builder(
            &self.http_client,
            &args.parent,
            &args.ruleId,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_rules_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories rules delete.
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
    pub fn artifactregistry_projects_locations_repositories_rules_delete(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesRulesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_rules_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_rules_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories rules get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleDevtoolsArtifactregistryV1Rule result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn artifactregistry_projects_locations_repositories_rules_get(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesRulesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleDevtoolsArtifactregistryV1Rule, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_rules_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_rules_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories rules list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListRulesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn artifactregistry_projects_locations_repositories_rules_list(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesRulesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListRulesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_rules_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_rules_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories rules patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleDevtoolsArtifactregistryV1Rule result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn artifactregistry_projects_locations_repositories_rules_patch(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesRulesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleDevtoolsArtifactregistryV1Rule, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_rules_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_rules_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories yum artifacts import.
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
    pub fn artifactregistry_projects_locations_repositories_yum_artifacts_import(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesYumArtifactsImportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_yum_artifacts_import_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_yum_artifacts_import_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories yum artifacts upload.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UploadYumArtifactMediaResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn artifactregistry_projects_locations_repositories_yum_artifacts_upload(
        &self,
        args: &ArtifactregistryProjectsLocationsRepositoriesYumArtifactsUploadArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UploadYumArtifactMediaResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = artifactregistry_projects_locations_repositories_yum_artifacts_upload_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = artifactregistry_projects_locations_repositories_yum_artifacts_upload_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
