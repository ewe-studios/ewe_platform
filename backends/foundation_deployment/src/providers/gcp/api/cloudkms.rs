//! CloudkmsProvider - State-aware cloudkms API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       cloudkms API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::cloudkms::{
    cloudkms_folders_update_autokey_config_builder, cloudkms_folders_update_autokey_config_task,
    cloudkms_folders_update_kaj_policy_config_builder, cloudkms_folders_update_kaj_policy_config_task,
    cloudkms_organizations_update_kaj_policy_config_builder, cloudkms_organizations_update_kaj_policy_config_task,
    cloudkms_projects_update_autokey_config_builder, cloudkms_projects_update_autokey_config_task,
    cloudkms_projects_update_kaj_policy_config_builder, cloudkms_projects_update_kaj_policy_config_task,
    cloudkms_projects_locations_generate_random_bytes_builder, cloudkms_projects_locations_generate_random_bytes_task,
    cloudkms_projects_locations_update_ekm_config_builder, cloudkms_projects_locations_update_ekm_config_task,
    cloudkms_projects_locations_ekm_config_set_iam_policy_builder, cloudkms_projects_locations_ekm_config_set_iam_policy_task,
    cloudkms_projects_locations_ekm_config_test_iam_permissions_builder, cloudkms_projects_locations_ekm_config_test_iam_permissions_task,
    cloudkms_projects_locations_ekm_connections_create_builder, cloudkms_projects_locations_ekm_connections_create_task,
    cloudkms_projects_locations_ekm_connections_patch_builder, cloudkms_projects_locations_ekm_connections_patch_task,
    cloudkms_projects_locations_ekm_connections_set_iam_policy_builder, cloudkms_projects_locations_ekm_connections_set_iam_policy_task,
    cloudkms_projects_locations_ekm_connections_test_iam_permissions_builder, cloudkms_projects_locations_ekm_connections_test_iam_permissions_task,
    cloudkms_projects_locations_key_handles_create_builder, cloudkms_projects_locations_key_handles_create_task,
    cloudkms_projects_locations_key_rings_create_builder, cloudkms_projects_locations_key_rings_create_task,
    cloudkms_projects_locations_key_rings_set_iam_policy_builder, cloudkms_projects_locations_key_rings_set_iam_policy_task,
    cloudkms_projects_locations_key_rings_test_iam_permissions_builder, cloudkms_projects_locations_key_rings_test_iam_permissions_task,
    cloudkms_projects_locations_key_rings_crypto_keys_create_builder, cloudkms_projects_locations_key_rings_crypto_keys_create_task,
    cloudkms_projects_locations_key_rings_crypto_keys_decrypt_builder, cloudkms_projects_locations_key_rings_crypto_keys_decrypt_task,
    cloudkms_projects_locations_key_rings_crypto_keys_delete_builder, cloudkms_projects_locations_key_rings_crypto_keys_delete_task,
    cloudkms_projects_locations_key_rings_crypto_keys_encrypt_builder, cloudkms_projects_locations_key_rings_crypto_keys_encrypt_task,
    cloudkms_projects_locations_key_rings_crypto_keys_patch_builder, cloudkms_projects_locations_key_rings_crypto_keys_patch_task,
    cloudkms_projects_locations_key_rings_crypto_keys_set_iam_policy_builder, cloudkms_projects_locations_key_rings_crypto_keys_set_iam_policy_task,
    cloudkms_projects_locations_key_rings_crypto_keys_test_iam_permissions_builder, cloudkms_projects_locations_key_rings_crypto_keys_test_iam_permissions_task,
    cloudkms_projects_locations_key_rings_crypto_keys_update_primary_version_builder, cloudkms_projects_locations_key_rings_crypto_keys_update_primary_version_task,
    cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_asymmetric_decrypt_builder, cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_asymmetric_decrypt_task,
    cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_asymmetric_sign_builder, cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_asymmetric_sign_task,
    cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_create_builder, cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_create_task,
    cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_decapsulate_builder, cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_decapsulate_task,
    cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_delete_builder, cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_delete_task,
    cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_destroy_builder, cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_destroy_task,
    cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_import_builder, cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_import_task,
    cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_mac_sign_builder, cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_mac_sign_task,
    cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_mac_verify_builder, cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_mac_verify_task,
    cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_patch_builder, cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_patch_task,
    cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_raw_decrypt_builder, cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_raw_decrypt_task,
    cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_raw_encrypt_builder, cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_raw_encrypt_task,
    cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_restore_builder, cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_restore_task,
    cloudkms_projects_locations_key_rings_import_jobs_create_builder, cloudkms_projects_locations_key_rings_import_jobs_create_task,
    cloudkms_projects_locations_key_rings_import_jobs_set_iam_policy_builder, cloudkms_projects_locations_key_rings_import_jobs_set_iam_policy_task,
    cloudkms_projects_locations_key_rings_import_jobs_test_iam_permissions_builder, cloudkms_projects_locations_key_rings_import_jobs_test_iam_permissions_task,
    cloudkms_projects_locations_single_tenant_hsm_instances_create_builder, cloudkms_projects_locations_single_tenant_hsm_instances_create_task,
    cloudkms_projects_locations_single_tenant_hsm_instances_proposals_approve_builder, cloudkms_projects_locations_single_tenant_hsm_instances_proposals_approve_task,
    cloudkms_projects_locations_single_tenant_hsm_instances_proposals_create_builder, cloudkms_projects_locations_single_tenant_hsm_instances_proposals_create_task,
    cloudkms_projects_locations_single_tenant_hsm_instances_proposals_delete_builder, cloudkms_projects_locations_single_tenant_hsm_instances_proposals_delete_task,
    cloudkms_projects_locations_single_tenant_hsm_instances_proposals_execute_builder, cloudkms_projects_locations_single_tenant_hsm_instances_proposals_execute_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::cloudkms::ApproveSingleTenantHsmInstanceProposalResponse;
use crate::providers::gcp::clients::cloudkms::AsymmetricDecryptResponse;
use crate::providers::gcp::clients::cloudkms::AsymmetricSignResponse;
use crate::providers::gcp::clients::cloudkms::AutokeyConfig;
use crate::providers::gcp::clients::cloudkms::CryptoKey;
use crate::providers::gcp::clients::cloudkms::CryptoKeyVersion;
use crate::providers::gcp::clients::cloudkms::DecapsulateResponse;
use crate::providers::gcp::clients::cloudkms::DecryptResponse;
use crate::providers::gcp::clients::cloudkms::EkmConfig;
use crate::providers::gcp::clients::cloudkms::EkmConnection;
use crate::providers::gcp::clients::cloudkms::Empty;
use crate::providers::gcp::clients::cloudkms::EncryptResponse;
use crate::providers::gcp::clients::cloudkms::GenerateRandomBytesResponse;
use crate::providers::gcp::clients::cloudkms::ImportJob;
use crate::providers::gcp::clients::cloudkms::KeyAccessJustificationsPolicyConfig;
use crate::providers::gcp::clients::cloudkms::KeyRing;
use crate::providers::gcp::clients::cloudkms::MacSignResponse;
use crate::providers::gcp::clients::cloudkms::MacVerifyResponse;
use crate::providers::gcp::clients::cloudkms::Operation;
use crate::providers::gcp::clients::cloudkms::Policy;
use crate::providers::gcp::clients::cloudkms::RawDecryptResponse;
use crate::providers::gcp::clients::cloudkms::RawEncryptResponse;
use crate::providers::gcp::clients::cloudkms::TestIamPermissionsResponse;
use crate::providers::gcp::clients::cloudkms::CloudkmsFoldersUpdateAutokeyConfigArgs;
use crate::providers::gcp::clients::cloudkms::CloudkmsFoldersUpdateKajPolicyConfigArgs;
use crate::providers::gcp::clients::cloudkms::CloudkmsOrganizationsUpdateKajPolicyConfigArgs;
use crate::providers::gcp::clients::cloudkms::CloudkmsProjectsLocationsEkmConfigSetIamPolicyArgs;
use crate::providers::gcp::clients::cloudkms::CloudkmsProjectsLocationsEkmConfigTestIamPermissionsArgs;
use crate::providers::gcp::clients::cloudkms::CloudkmsProjectsLocationsEkmConnectionsCreateArgs;
use crate::providers::gcp::clients::cloudkms::CloudkmsProjectsLocationsEkmConnectionsPatchArgs;
use crate::providers::gcp::clients::cloudkms::CloudkmsProjectsLocationsEkmConnectionsSetIamPolicyArgs;
use crate::providers::gcp::clients::cloudkms::CloudkmsProjectsLocationsEkmConnectionsTestIamPermissionsArgs;
use crate::providers::gcp::clients::cloudkms::CloudkmsProjectsLocationsGenerateRandomBytesArgs;
use crate::providers::gcp::clients::cloudkms::CloudkmsProjectsLocationsKeyHandlesCreateArgs;
use crate::providers::gcp::clients::cloudkms::CloudkmsProjectsLocationsKeyRingsCreateArgs;
use crate::providers::gcp::clients::cloudkms::CloudkmsProjectsLocationsKeyRingsCryptoKeysCreateArgs;
use crate::providers::gcp::clients::cloudkms::CloudkmsProjectsLocationsKeyRingsCryptoKeysCryptoKeyVersionsAsymmetricDecryptArgs;
use crate::providers::gcp::clients::cloudkms::CloudkmsProjectsLocationsKeyRingsCryptoKeysCryptoKeyVersionsAsymmetricSignArgs;
use crate::providers::gcp::clients::cloudkms::CloudkmsProjectsLocationsKeyRingsCryptoKeysCryptoKeyVersionsCreateArgs;
use crate::providers::gcp::clients::cloudkms::CloudkmsProjectsLocationsKeyRingsCryptoKeysCryptoKeyVersionsDecapsulateArgs;
use crate::providers::gcp::clients::cloudkms::CloudkmsProjectsLocationsKeyRingsCryptoKeysCryptoKeyVersionsDeleteArgs;
use crate::providers::gcp::clients::cloudkms::CloudkmsProjectsLocationsKeyRingsCryptoKeysCryptoKeyVersionsDestroyArgs;
use crate::providers::gcp::clients::cloudkms::CloudkmsProjectsLocationsKeyRingsCryptoKeysCryptoKeyVersionsImportArgs;
use crate::providers::gcp::clients::cloudkms::CloudkmsProjectsLocationsKeyRingsCryptoKeysCryptoKeyVersionsMacSignArgs;
use crate::providers::gcp::clients::cloudkms::CloudkmsProjectsLocationsKeyRingsCryptoKeysCryptoKeyVersionsMacVerifyArgs;
use crate::providers::gcp::clients::cloudkms::CloudkmsProjectsLocationsKeyRingsCryptoKeysCryptoKeyVersionsPatchArgs;
use crate::providers::gcp::clients::cloudkms::CloudkmsProjectsLocationsKeyRingsCryptoKeysCryptoKeyVersionsRawDecryptArgs;
use crate::providers::gcp::clients::cloudkms::CloudkmsProjectsLocationsKeyRingsCryptoKeysCryptoKeyVersionsRawEncryptArgs;
use crate::providers::gcp::clients::cloudkms::CloudkmsProjectsLocationsKeyRingsCryptoKeysCryptoKeyVersionsRestoreArgs;
use crate::providers::gcp::clients::cloudkms::CloudkmsProjectsLocationsKeyRingsCryptoKeysDecryptArgs;
use crate::providers::gcp::clients::cloudkms::CloudkmsProjectsLocationsKeyRingsCryptoKeysDeleteArgs;
use crate::providers::gcp::clients::cloudkms::CloudkmsProjectsLocationsKeyRingsCryptoKeysEncryptArgs;
use crate::providers::gcp::clients::cloudkms::CloudkmsProjectsLocationsKeyRingsCryptoKeysPatchArgs;
use crate::providers::gcp::clients::cloudkms::CloudkmsProjectsLocationsKeyRingsCryptoKeysSetIamPolicyArgs;
use crate::providers::gcp::clients::cloudkms::CloudkmsProjectsLocationsKeyRingsCryptoKeysTestIamPermissionsArgs;
use crate::providers::gcp::clients::cloudkms::CloudkmsProjectsLocationsKeyRingsCryptoKeysUpdatePrimaryVersionArgs;
use crate::providers::gcp::clients::cloudkms::CloudkmsProjectsLocationsKeyRingsImportJobsCreateArgs;
use crate::providers::gcp::clients::cloudkms::CloudkmsProjectsLocationsKeyRingsImportJobsSetIamPolicyArgs;
use crate::providers::gcp::clients::cloudkms::CloudkmsProjectsLocationsKeyRingsImportJobsTestIamPermissionsArgs;
use crate::providers::gcp::clients::cloudkms::CloudkmsProjectsLocationsKeyRingsSetIamPolicyArgs;
use crate::providers::gcp::clients::cloudkms::CloudkmsProjectsLocationsKeyRingsTestIamPermissionsArgs;
use crate::providers::gcp::clients::cloudkms::CloudkmsProjectsLocationsSingleTenantHsmInstancesCreateArgs;
use crate::providers::gcp::clients::cloudkms::CloudkmsProjectsLocationsSingleTenantHsmInstancesProposalsApproveArgs;
use crate::providers::gcp::clients::cloudkms::CloudkmsProjectsLocationsSingleTenantHsmInstancesProposalsCreateArgs;
use crate::providers::gcp::clients::cloudkms::CloudkmsProjectsLocationsSingleTenantHsmInstancesProposalsDeleteArgs;
use crate::providers::gcp::clients::cloudkms::CloudkmsProjectsLocationsSingleTenantHsmInstancesProposalsExecuteArgs;
use crate::providers::gcp::clients::cloudkms::CloudkmsProjectsLocationsUpdateEkmConfigArgs;
use crate::providers::gcp::clients::cloudkms::CloudkmsProjectsUpdateAutokeyConfigArgs;
use crate::providers::gcp::clients::cloudkms::CloudkmsProjectsUpdateKajPolicyConfigArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// CloudkmsProvider with automatic state tracking.
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
/// let provider = CloudkmsProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct CloudkmsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> CloudkmsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new CloudkmsProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Cloudkms folders update autokey config.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AutokeyConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudkms_folders_update_autokey_config(
        &self,
        args: &CloudkmsFoldersUpdateAutokeyConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AutokeyConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudkms_folders_update_autokey_config_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudkms_folders_update_autokey_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudkms folders update kaj policy config.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the KeyAccessJustificationsPolicyConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudkms_folders_update_kaj_policy_config(
        &self,
        args: &CloudkmsFoldersUpdateKajPolicyConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<KeyAccessJustificationsPolicyConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudkms_folders_update_kaj_policy_config_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudkms_folders_update_kaj_policy_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudkms organizations update kaj policy config.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the KeyAccessJustificationsPolicyConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudkms_organizations_update_kaj_policy_config(
        &self,
        args: &CloudkmsOrganizationsUpdateKajPolicyConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<KeyAccessJustificationsPolicyConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudkms_organizations_update_kaj_policy_config_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudkms_organizations_update_kaj_policy_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudkms projects update autokey config.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AutokeyConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudkms_projects_update_autokey_config(
        &self,
        args: &CloudkmsProjectsUpdateAutokeyConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AutokeyConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudkms_projects_update_autokey_config_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudkms_projects_update_autokey_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudkms projects update kaj policy config.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the KeyAccessJustificationsPolicyConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudkms_projects_update_kaj_policy_config(
        &self,
        args: &CloudkmsProjectsUpdateKajPolicyConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<KeyAccessJustificationsPolicyConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudkms_projects_update_kaj_policy_config_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudkms_projects_update_kaj_policy_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudkms projects locations generate random bytes.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GenerateRandomBytesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudkms_projects_locations_generate_random_bytes(
        &self,
        args: &CloudkmsProjectsLocationsGenerateRandomBytesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GenerateRandomBytesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudkms_projects_locations_generate_random_bytes_builder(
            &self.http_client,
            &args.location,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudkms_projects_locations_generate_random_bytes_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudkms projects locations update ekm config.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EkmConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudkms_projects_locations_update_ekm_config(
        &self,
        args: &CloudkmsProjectsLocationsUpdateEkmConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EkmConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudkms_projects_locations_update_ekm_config_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudkms_projects_locations_update_ekm_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudkms projects locations ekm config set iam policy.
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
    pub fn cloudkms_projects_locations_ekm_config_set_iam_policy(
        &self,
        args: &CloudkmsProjectsLocationsEkmConfigSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudkms_projects_locations_ekm_config_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudkms_projects_locations_ekm_config_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudkms projects locations ekm config test iam permissions.
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
    pub fn cloudkms_projects_locations_ekm_config_test_iam_permissions(
        &self,
        args: &CloudkmsProjectsLocationsEkmConfigTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudkms_projects_locations_ekm_config_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudkms_projects_locations_ekm_config_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudkms projects locations ekm connections create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EkmConnection result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudkms_projects_locations_ekm_connections_create(
        &self,
        args: &CloudkmsProjectsLocationsEkmConnectionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EkmConnection, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudkms_projects_locations_ekm_connections_create_builder(
            &self.http_client,
            &args.parent,
            &args.ekmConnectionId,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudkms_projects_locations_ekm_connections_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudkms projects locations ekm connections patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EkmConnection result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudkms_projects_locations_ekm_connections_patch(
        &self,
        args: &CloudkmsProjectsLocationsEkmConnectionsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EkmConnection, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudkms_projects_locations_ekm_connections_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudkms_projects_locations_ekm_connections_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudkms projects locations ekm connections set iam policy.
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
    pub fn cloudkms_projects_locations_ekm_connections_set_iam_policy(
        &self,
        args: &CloudkmsProjectsLocationsEkmConnectionsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudkms_projects_locations_ekm_connections_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudkms_projects_locations_ekm_connections_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudkms projects locations ekm connections test iam permissions.
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
    pub fn cloudkms_projects_locations_ekm_connections_test_iam_permissions(
        &self,
        args: &CloudkmsProjectsLocationsEkmConnectionsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudkms_projects_locations_ekm_connections_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudkms_projects_locations_ekm_connections_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudkms projects locations key handles create.
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
    pub fn cloudkms_projects_locations_key_handles_create(
        &self,
        args: &CloudkmsProjectsLocationsKeyHandlesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudkms_projects_locations_key_handles_create_builder(
            &self.http_client,
            &args.parent,
            &args.keyHandleId,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudkms_projects_locations_key_handles_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudkms projects locations key rings create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the KeyRing result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudkms_projects_locations_key_rings_create(
        &self,
        args: &CloudkmsProjectsLocationsKeyRingsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<KeyRing, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudkms_projects_locations_key_rings_create_builder(
            &self.http_client,
            &args.parent,
            &args.keyRingId,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudkms_projects_locations_key_rings_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudkms projects locations key rings set iam policy.
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
    pub fn cloudkms_projects_locations_key_rings_set_iam_policy(
        &self,
        args: &CloudkmsProjectsLocationsKeyRingsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudkms_projects_locations_key_rings_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudkms_projects_locations_key_rings_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudkms projects locations key rings test iam permissions.
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
    pub fn cloudkms_projects_locations_key_rings_test_iam_permissions(
        &self,
        args: &CloudkmsProjectsLocationsKeyRingsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudkms_projects_locations_key_rings_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudkms_projects_locations_key_rings_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudkms projects locations key rings crypto keys create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CryptoKey result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudkms_projects_locations_key_rings_crypto_keys_create(
        &self,
        args: &CloudkmsProjectsLocationsKeyRingsCryptoKeysCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CryptoKey, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudkms_projects_locations_key_rings_crypto_keys_create_builder(
            &self.http_client,
            &args.parent,
            &args.cryptoKeyId,
            &args.skipInitialVersionCreation,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudkms_projects_locations_key_rings_crypto_keys_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudkms projects locations key rings crypto keys decrypt.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DecryptResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudkms_projects_locations_key_rings_crypto_keys_decrypt(
        &self,
        args: &CloudkmsProjectsLocationsKeyRingsCryptoKeysDecryptArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DecryptResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudkms_projects_locations_key_rings_crypto_keys_decrypt_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudkms_projects_locations_key_rings_crypto_keys_decrypt_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudkms projects locations key rings crypto keys delete.
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
    pub fn cloudkms_projects_locations_key_rings_crypto_keys_delete(
        &self,
        args: &CloudkmsProjectsLocationsKeyRingsCryptoKeysDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudkms_projects_locations_key_rings_crypto_keys_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudkms_projects_locations_key_rings_crypto_keys_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudkms projects locations key rings crypto keys encrypt.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EncryptResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudkms_projects_locations_key_rings_crypto_keys_encrypt(
        &self,
        args: &CloudkmsProjectsLocationsKeyRingsCryptoKeysEncryptArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EncryptResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudkms_projects_locations_key_rings_crypto_keys_encrypt_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudkms_projects_locations_key_rings_crypto_keys_encrypt_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudkms projects locations key rings crypto keys patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CryptoKey result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudkms_projects_locations_key_rings_crypto_keys_patch(
        &self,
        args: &CloudkmsProjectsLocationsKeyRingsCryptoKeysPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CryptoKey, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudkms_projects_locations_key_rings_crypto_keys_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudkms_projects_locations_key_rings_crypto_keys_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudkms projects locations key rings crypto keys set iam policy.
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
    pub fn cloudkms_projects_locations_key_rings_crypto_keys_set_iam_policy(
        &self,
        args: &CloudkmsProjectsLocationsKeyRingsCryptoKeysSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudkms_projects_locations_key_rings_crypto_keys_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudkms_projects_locations_key_rings_crypto_keys_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudkms projects locations key rings crypto keys test iam permissions.
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
    pub fn cloudkms_projects_locations_key_rings_crypto_keys_test_iam_permissions(
        &self,
        args: &CloudkmsProjectsLocationsKeyRingsCryptoKeysTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudkms_projects_locations_key_rings_crypto_keys_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudkms_projects_locations_key_rings_crypto_keys_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudkms projects locations key rings crypto keys update primary version.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CryptoKey result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudkms_projects_locations_key_rings_crypto_keys_update_primary_version(
        &self,
        args: &CloudkmsProjectsLocationsKeyRingsCryptoKeysUpdatePrimaryVersionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CryptoKey, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudkms_projects_locations_key_rings_crypto_keys_update_primary_version_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudkms_projects_locations_key_rings_crypto_keys_update_primary_version_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudkms projects locations key rings crypto keys crypto key versions asymmetric decrypt.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AsymmetricDecryptResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_asymmetric_decrypt(
        &self,
        args: &CloudkmsProjectsLocationsKeyRingsCryptoKeysCryptoKeyVersionsAsymmetricDecryptArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AsymmetricDecryptResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_asymmetric_decrypt_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_asymmetric_decrypt_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudkms projects locations key rings crypto keys crypto key versions asymmetric sign.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AsymmetricSignResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_asymmetric_sign(
        &self,
        args: &CloudkmsProjectsLocationsKeyRingsCryptoKeysCryptoKeyVersionsAsymmetricSignArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AsymmetricSignResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_asymmetric_sign_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_asymmetric_sign_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudkms projects locations key rings crypto keys crypto key versions create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CryptoKeyVersion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_create(
        &self,
        args: &CloudkmsProjectsLocationsKeyRingsCryptoKeysCryptoKeyVersionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CryptoKeyVersion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudkms projects locations key rings crypto keys crypto key versions decapsulate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DecapsulateResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_decapsulate(
        &self,
        args: &CloudkmsProjectsLocationsKeyRingsCryptoKeysCryptoKeyVersionsDecapsulateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DecapsulateResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_decapsulate_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_decapsulate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudkms projects locations key rings crypto keys crypto key versions delete.
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
    pub fn cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_delete(
        &self,
        args: &CloudkmsProjectsLocationsKeyRingsCryptoKeysCryptoKeyVersionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudkms projects locations key rings crypto keys crypto key versions destroy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CryptoKeyVersion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_destroy(
        &self,
        args: &CloudkmsProjectsLocationsKeyRingsCryptoKeysCryptoKeyVersionsDestroyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CryptoKeyVersion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_destroy_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_destroy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudkms projects locations key rings crypto keys crypto key versions import.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CryptoKeyVersion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_import(
        &self,
        args: &CloudkmsProjectsLocationsKeyRingsCryptoKeysCryptoKeyVersionsImportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CryptoKeyVersion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_import_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_import_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudkms projects locations key rings crypto keys crypto key versions mac sign.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the MacSignResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_mac_sign(
        &self,
        args: &CloudkmsProjectsLocationsKeyRingsCryptoKeysCryptoKeyVersionsMacSignArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<MacSignResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_mac_sign_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_mac_sign_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudkms projects locations key rings crypto keys crypto key versions mac verify.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the MacVerifyResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_mac_verify(
        &self,
        args: &CloudkmsProjectsLocationsKeyRingsCryptoKeysCryptoKeyVersionsMacVerifyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<MacVerifyResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_mac_verify_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_mac_verify_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudkms projects locations key rings crypto keys crypto key versions patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CryptoKeyVersion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_patch(
        &self,
        args: &CloudkmsProjectsLocationsKeyRingsCryptoKeysCryptoKeyVersionsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CryptoKeyVersion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudkms projects locations key rings crypto keys crypto key versions raw decrypt.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RawDecryptResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_raw_decrypt(
        &self,
        args: &CloudkmsProjectsLocationsKeyRingsCryptoKeysCryptoKeyVersionsRawDecryptArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RawDecryptResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_raw_decrypt_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_raw_decrypt_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudkms projects locations key rings crypto keys crypto key versions raw encrypt.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RawEncryptResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_raw_encrypt(
        &self,
        args: &CloudkmsProjectsLocationsKeyRingsCryptoKeysCryptoKeyVersionsRawEncryptArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RawEncryptResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_raw_encrypt_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_raw_encrypt_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudkms projects locations key rings crypto keys crypto key versions restore.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CryptoKeyVersion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_restore(
        &self,
        args: &CloudkmsProjectsLocationsKeyRingsCryptoKeysCryptoKeyVersionsRestoreArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CryptoKeyVersion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_restore_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudkms_projects_locations_key_rings_crypto_keys_crypto_key_versions_restore_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudkms projects locations key rings import jobs create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ImportJob result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudkms_projects_locations_key_rings_import_jobs_create(
        &self,
        args: &CloudkmsProjectsLocationsKeyRingsImportJobsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ImportJob, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudkms_projects_locations_key_rings_import_jobs_create_builder(
            &self.http_client,
            &args.parent,
            &args.importJobId,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudkms_projects_locations_key_rings_import_jobs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudkms projects locations key rings import jobs set iam policy.
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
    pub fn cloudkms_projects_locations_key_rings_import_jobs_set_iam_policy(
        &self,
        args: &CloudkmsProjectsLocationsKeyRingsImportJobsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudkms_projects_locations_key_rings_import_jobs_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudkms_projects_locations_key_rings_import_jobs_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudkms projects locations key rings import jobs test iam permissions.
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
    pub fn cloudkms_projects_locations_key_rings_import_jobs_test_iam_permissions(
        &self,
        args: &CloudkmsProjectsLocationsKeyRingsImportJobsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudkms_projects_locations_key_rings_import_jobs_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudkms_projects_locations_key_rings_import_jobs_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudkms projects locations single tenant hsm instances create.
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
    pub fn cloudkms_projects_locations_single_tenant_hsm_instances_create(
        &self,
        args: &CloudkmsProjectsLocationsSingleTenantHsmInstancesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudkms_projects_locations_single_tenant_hsm_instances_create_builder(
            &self.http_client,
            &args.parent,
            &args.singleTenantHsmInstanceId,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudkms_projects_locations_single_tenant_hsm_instances_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudkms projects locations single tenant hsm instances proposals approve.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApproveSingleTenantHsmInstanceProposalResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudkms_projects_locations_single_tenant_hsm_instances_proposals_approve(
        &self,
        args: &CloudkmsProjectsLocationsSingleTenantHsmInstancesProposalsApproveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApproveSingleTenantHsmInstanceProposalResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudkms_projects_locations_single_tenant_hsm_instances_proposals_approve_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudkms_projects_locations_single_tenant_hsm_instances_proposals_approve_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudkms projects locations single tenant hsm instances proposals create.
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
    pub fn cloudkms_projects_locations_single_tenant_hsm_instances_proposals_create(
        &self,
        args: &CloudkmsProjectsLocationsSingleTenantHsmInstancesProposalsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudkms_projects_locations_single_tenant_hsm_instances_proposals_create_builder(
            &self.http_client,
            &args.parent,
            &args.singleTenantHsmInstanceProposalId,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudkms_projects_locations_single_tenant_hsm_instances_proposals_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudkms projects locations single tenant hsm instances proposals delete.
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
    pub fn cloudkms_projects_locations_single_tenant_hsm_instances_proposals_delete(
        &self,
        args: &CloudkmsProjectsLocationsSingleTenantHsmInstancesProposalsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudkms_projects_locations_single_tenant_hsm_instances_proposals_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudkms_projects_locations_single_tenant_hsm_instances_proposals_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudkms projects locations single tenant hsm instances proposals execute.
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
    pub fn cloudkms_projects_locations_single_tenant_hsm_instances_proposals_execute(
        &self,
        args: &CloudkmsProjectsLocationsSingleTenantHsmInstancesProposalsExecuteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudkms_projects_locations_single_tenant_hsm_instances_proposals_execute_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudkms_projects_locations_single_tenant_hsm_instances_proposals_execute_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
