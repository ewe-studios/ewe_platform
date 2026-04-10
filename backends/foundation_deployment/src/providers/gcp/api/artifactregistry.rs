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
    artifactregistry_projects_update_project_settings_builder, artifactregistry_projects_update_project_settings_task,
    artifactregistry_projects_locations_update_project_config_builder, artifactregistry_projects_locations_update_project_config_task,
    artifactregistry_projects_locations_update_vpcsc_config_builder, artifactregistry_projects_locations_update_vpcsc_config_task,
    artifactregistry_projects_locations_operations_cancel_builder, artifactregistry_projects_locations_operations_cancel_task,
    artifactregistry_projects_locations_repositories_create_builder, artifactregistry_projects_locations_repositories_create_task,
    artifactregistry_projects_locations_repositories_delete_builder, artifactregistry_projects_locations_repositories_delete_task,
    artifactregistry_projects_locations_repositories_export_artifact_builder, artifactregistry_projects_locations_repositories_export_artifact_task,
    artifactregistry_projects_locations_repositories_patch_builder, artifactregistry_projects_locations_repositories_patch_task,
    artifactregistry_projects_locations_repositories_set_iam_policy_builder, artifactregistry_projects_locations_repositories_set_iam_policy_task,
    artifactregistry_projects_locations_repositories_test_iam_permissions_builder, artifactregistry_projects_locations_repositories_test_iam_permissions_task,
    artifactregistry_projects_locations_repositories_apt_artifacts_import_builder, artifactregistry_projects_locations_repositories_apt_artifacts_import_task,
    artifactregistry_projects_locations_repositories_apt_artifacts_upload_builder, artifactregistry_projects_locations_repositories_apt_artifacts_upload_task,
    artifactregistry_projects_locations_repositories_attachments_create_builder, artifactregistry_projects_locations_repositories_attachments_create_task,
    artifactregistry_projects_locations_repositories_attachments_delete_builder, artifactregistry_projects_locations_repositories_attachments_delete_task,
    artifactregistry_projects_locations_repositories_files_delete_builder, artifactregistry_projects_locations_repositories_files_delete_task,
    artifactregistry_projects_locations_repositories_files_patch_builder, artifactregistry_projects_locations_repositories_files_patch_task,
    artifactregistry_projects_locations_repositories_files_upload_builder, artifactregistry_projects_locations_repositories_files_upload_task,
    artifactregistry_projects_locations_repositories_generic_artifacts_upload_builder, artifactregistry_projects_locations_repositories_generic_artifacts_upload_task,
    artifactregistry_projects_locations_repositories_go_modules_upload_builder, artifactregistry_projects_locations_repositories_go_modules_upload_task,
    artifactregistry_projects_locations_repositories_googet_artifacts_import_builder, artifactregistry_projects_locations_repositories_googet_artifacts_import_task,
    artifactregistry_projects_locations_repositories_googet_artifacts_upload_builder, artifactregistry_projects_locations_repositories_googet_artifacts_upload_task,
    artifactregistry_projects_locations_repositories_kfp_artifacts_upload_builder, artifactregistry_projects_locations_repositories_kfp_artifacts_upload_task,
    artifactregistry_projects_locations_repositories_packages_delete_builder, artifactregistry_projects_locations_repositories_packages_delete_task,
    artifactregistry_projects_locations_repositories_packages_patch_builder, artifactregistry_projects_locations_repositories_packages_patch_task,
    artifactregistry_projects_locations_repositories_packages_tags_create_builder, artifactregistry_projects_locations_repositories_packages_tags_create_task,
    artifactregistry_projects_locations_repositories_packages_tags_delete_builder, artifactregistry_projects_locations_repositories_packages_tags_delete_task,
    artifactregistry_projects_locations_repositories_packages_tags_patch_builder, artifactregistry_projects_locations_repositories_packages_tags_patch_task,
    artifactregistry_projects_locations_repositories_packages_versions_batch_delete_builder, artifactregistry_projects_locations_repositories_packages_versions_batch_delete_task,
    artifactregistry_projects_locations_repositories_packages_versions_delete_builder, artifactregistry_projects_locations_repositories_packages_versions_delete_task,
    artifactregistry_projects_locations_repositories_packages_versions_patch_builder, artifactregistry_projects_locations_repositories_packages_versions_patch_task,
    artifactregistry_projects_locations_repositories_rules_create_builder, artifactregistry_projects_locations_repositories_rules_create_task,
    artifactregistry_projects_locations_repositories_rules_delete_builder, artifactregistry_projects_locations_repositories_rules_delete_task,
    artifactregistry_projects_locations_repositories_rules_patch_builder, artifactregistry_projects_locations_repositories_rules_patch_task,
    artifactregistry_projects_locations_repositories_yum_artifacts_import_builder, artifactregistry_projects_locations_repositories_yum_artifacts_import_task,
    artifactregistry_projects_locations_repositories_yum_artifacts_upload_builder, artifactregistry_projects_locations_repositories_yum_artifacts_upload_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::artifactregistry::Empty;
use crate::providers::gcp::clients::artifactregistry::GoogleDevtoolsArtifactregistryV1File;
use crate::providers::gcp::clients::artifactregistry::GoogleDevtoolsArtifactregistryV1Rule;
use crate::providers::gcp::clients::artifactregistry::Operation;
use crate::providers::gcp::clients::artifactregistry::Package;
use crate::providers::gcp::clients::artifactregistry::Policy;
use crate::providers::gcp::clients::artifactregistry::ProjectConfig;
use crate::providers::gcp::clients::artifactregistry::ProjectSettings;
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
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesAptArtifactsImportArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesAptArtifactsUploadArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesAttachmentsCreateArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesAttachmentsDeleteArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesCreateArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesDeleteArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesExportArtifactArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesFilesDeleteArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesFilesPatchArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesFilesUploadArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesGenericArtifactsUploadArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesGoModulesUploadArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesGoogetArtifactsImportArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesGoogetArtifactsUploadArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesKfpArtifactsUploadArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesPackagesDeleteArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesPackagesPatchArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesPackagesTagsCreateArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesPackagesTagsDeleteArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesPackagesTagsPatchArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesPackagesVersionsBatchDeleteArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesPackagesVersionsDeleteArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesPackagesVersionsPatchArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesPatchArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesRulesCreateArgs;
use crate::providers::gcp::clients::artifactregistry::ArtifactregistryProjectsLocationsRepositoriesRulesDeleteArgs;
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Artifactregistry projects locations repositories googet artifacts upload.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
