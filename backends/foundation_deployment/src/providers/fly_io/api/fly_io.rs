//! FlyIoProvider - State-aware fly_io API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       fly_io API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "fly_io")]

use crate::providers::fly_io::clients::fly_io::{
    apps_create_builder, apps_create_task,
    apps_delete_builder, apps_delete_task,
    app_certificates_acme_create_builder, app_certificates_acme_create_task,
    app_certificates_custom_create_builder, app_certificates_custom_create_task,
    app_certificates_delete_builder, app_certificates_delete_task,
    app_certificates_acme_delete_builder, app_certificates_acme_delete_task,
    app_certificates_check_builder, app_certificates_check_task,
    app_certificates_custom_delete_builder, app_certificates_custom_delete_task,
    app_create_deploy_token_builder, app_create_deploy_token_task,
    app_i_p_assignments_create_builder, app_i_p_assignments_create_task,
    app_i_p_assignments_delete_builder, app_i_p_assignments_delete_task,
    machines_create_builder, machines_create_task,
    machines_update_builder, machines_update_task,
    machines_delete_builder, machines_delete_task,
    machines_cordon_builder, machines_cordon_task,
    machines_exec_builder, machines_exec_task,
    machines_create_lease_builder, machines_create_lease_task,
    machines_release_lease_builder, machines_release_lease_task,
    machines_set_memory_limit_builder, machines_set_memory_limit_task,
    machines_reclaim_memory_builder, machines_reclaim_memory_task,
    machines_update_metadata_builder, machines_update_metadata_task,
    machines_upsert_metadata_builder, machines_upsert_metadata_task,
    machines_delete_metadata_builder, machines_delete_metadata_task,
    machines_restart_builder, machines_restart_task,
    machines_signal_builder, machines_signal_task,
    machines_start_builder, machines_start_task,
    machines_stop_builder, machines_stop_task,
    machines_suspend_builder, machines_suspend_task,
    machines_uncordon_builder, machines_uncordon_task,
    secretkey_set_builder, secretkey_set_task,
    secretkey_delete_builder, secretkey_delete_task,
    secretkey_decrypt_builder, secretkey_decrypt_task,
    secretkey_encrypt_builder, secretkey_encrypt_task,
    secretkey_generate_builder, secretkey_generate_task,
    secretkey_sign_builder, secretkey_sign_task,
    secretkey_verify_builder, secretkey_verify_task,
    secrets_update_builder, secrets_update_task,
    secret_create_builder, secret_create_task,
    secret_delete_builder, secret_delete_task,
    volumes_create_builder, volumes_create_task,
    volumes_update_builder, volumes_update_task,
    volume_delete_builder, volume_delete_task,
    volumes_extend_builder, volumes_extend_task,
    create_volume_snapshot_builder, create_volume_snapshot_task,
    platform_placements_post_builder, platform_placements_post_task,
    tokens_request_kms_builder, tokens_request_kms_task,
    tokens_request_o_i_d_c_builder, tokens_request_o_i_d_c_task,
};
use crate::providers::fly_io::clients::types::{ApiError, ApiPending};
use crate::providers::fly_io::clients::fly_io::AppSecretsUpdateResp;
use crate::providers::fly_io::clients::fly_io::CertificateCheckResponse;
use crate::providers::fly_io::clients::fly_io::CertificateDetail;
use crate::providers::fly_io::clients::fly_io::CreateAppResponse;
use crate::providers::fly_io::clients::fly_io::DecryptSecretkeyResponse;
use crate::providers::fly_io::clients::fly_io::DeleteAppSecretResponse;
use crate::providers::fly_io::clients::fly_io::DeleteSecretkeyResponse;
use crate::providers::fly_io::clients::fly_io::DestroyCustomCertificateResponse;
use crate::providers::fly_io::clients::fly_io::EncryptSecretkeyResponse;
use crate::providers::fly_io::clients::fly_io::ExtendVolumeResponse;
use crate::providers::fly_io::clients::fly_io::Flydv1ExecResponse;
use crate::providers::fly_io::clients::fly_io::IPAssignment;
use crate::providers::fly_io::clients::fly_io::Lease;
use crate::providers::fly_io::clients::fly_io::Machine;
use crate::providers::fly_io::clients::fly_io::MainGetPlacementsResponse;
use crate::providers::fly_io::clients::fly_io::MainMemoryResponse;
use crate::providers::fly_io::clients::fly_io::MainReclaimMemoryResponse;
use crate::providers::fly_io::clients::fly_io::SetAppSecretResponse;
use crate::providers::fly_io::clients::fly_io::SetSecretkeyResponse;
use crate::providers::fly_io::clients::fly_io::SignSecretkeyResponse;
use crate::providers::fly_io::clients::fly_io::Volume;
use crate::providers::fly_io::clients::fly_io::AppCertificatesAcmeCreateArgs;
use crate::providers::fly_io::clients::fly_io::AppCertificatesAcmeDeleteArgs;
use crate::providers::fly_io::clients::fly_io::AppCertificatesCheckArgs;
use crate::providers::fly_io::clients::fly_io::AppCertificatesCustomCreateArgs;
use crate::providers::fly_io::clients::fly_io::AppCertificatesCustomDeleteArgs;
use crate::providers::fly_io::clients::fly_io::AppCertificatesDeleteArgs;
use crate::providers::fly_io::clients::fly_io::AppCreateDeployTokenArgs;
use crate::providers::fly_io::clients::fly_io::AppIPAssignmentsCreateArgs;
use crate::providers::fly_io::clients::fly_io::AppIPAssignmentsDeleteArgs;
use crate::providers::fly_io::clients::fly_io::AppsCreateArgs;
use crate::providers::fly_io::clients::fly_io::AppsDeleteArgs;
use crate::providers::fly_io::clients::fly_io::CreateVolumeSnapshotArgs;
use crate::providers::fly_io::clients::fly_io::MachinesCordonArgs;
use crate::providers::fly_io::clients::fly_io::MachinesCreateArgs;
use crate::providers::fly_io::clients::fly_io::MachinesCreateLeaseArgs;
use crate::providers::fly_io::clients::fly_io::MachinesDeleteArgs;
use crate::providers::fly_io::clients::fly_io::MachinesDeleteMetadataArgs;
use crate::providers::fly_io::clients::fly_io::MachinesExecArgs;
use crate::providers::fly_io::clients::fly_io::MachinesReclaimMemoryArgs;
use crate::providers::fly_io::clients::fly_io::MachinesReleaseLeaseArgs;
use crate::providers::fly_io::clients::fly_io::MachinesRestartArgs;
use crate::providers::fly_io::clients::fly_io::MachinesSetMemoryLimitArgs;
use crate::providers::fly_io::clients::fly_io::MachinesSignalArgs;
use crate::providers::fly_io::clients::fly_io::MachinesStartArgs;
use crate::providers::fly_io::clients::fly_io::MachinesStopArgs;
use crate::providers::fly_io::clients::fly_io::MachinesSuspendArgs;
use crate::providers::fly_io::clients::fly_io::MachinesUncordonArgs;
use crate::providers::fly_io::clients::fly_io::MachinesUpdateArgs;
use crate::providers::fly_io::clients::fly_io::MachinesUpdateMetadataArgs;
use crate::providers::fly_io::clients::fly_io::MachinesUpsertMetadataArgs;
use crate::providers::fly_io::clients::fly_io::PlatformPlacementsPostArgs;
use crate::providers::fly_io::clients::fly_io::SecretCreateArgs;
use crate::providers::fly_io::clients::fly_io::SecretDeleteArgs;
use crate::providers::fly_io::clients::fly_io::SecretkeyDecryptArgs;
use crate::providers::fly_io::clients::fly_io::SecretkeyDeleteArgs;
use crate::providers::fly_io::clients::fly_io::SecretkeyEncryptArgs;
use crate::providers::fly_io::clients::fly_io::SecretkeyGenerateArgs;
use crate::providers::fly_io::clients::fly_io::SecretkeySetArgs;
use crate::providers::fly_io::clients::fly_io::SecretkeySignArgs;
use crate::providers::fly_io::clients::fly_io::SecretkeyVerifyArgs;
use crate::providers::fly_io::clients::fly_io::SecretsUpdateArgs;
use crate::providers::fly_io::clients::fly_io::TokensRequestKmsArgs;
use crate::providers::fly_io::clients::fly_io::TokensRequestOIDCArgs;
use crate::providers::fly_io::clients::fly_io::VolumeDeleteArgs;
use crate::providers::fly_io::clients::fly_io::VolumesCreateArgs;
use crate::providers::fly_io::clients::fly_io::VolumesExtendArgs;
use crate::providers::fly_io::clients::fly_io::VolumesUpdateArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// FlyIoProvider with automatic state tracking.
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
/// let provider = FlyIoProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct FlyIoProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> FlyIoProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new FlyIoProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Apps create.
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
    pub fn apps_create(
        &self,
        args: &AppsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apps_create_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = apps_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apps delete.
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
    pub fn apps_delete(
        &self,
        args: &AppsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apps_delete_builder(
            &self.http_client,
            &args.app_name,
        )
        .map_err(ProviderError::Api)?;

        let task = apps_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// App certificates acme create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CertificateDetail result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn app_certificates_acme_create(
        &self,
        args: &AppCertificatesAcmeCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CertificateDetail, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = app_certificates_acme_create_builder(
            &self.http_client,
            &args.app_name,
        )
        .map_err(ProviderError::Api)?;

        let task = app_certificates_acme_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// App certificates custom create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CertificateDetail result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn app_certificates_custom_create(
        &self,
        args: &AppCertificatesCustomCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CertificateDetail, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = app_certificates_custom_create_builder(
            &self.http_client,
            &args.app_name,
        )
        .map_err(ProviderError::Api)?;

        let task = app_certificates_custom_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// App certificates delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the () result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn app_certificates_delete(
        &self,
        args: &AppCertificatesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = app_certificates_delete_builder(
            &self.http_client,
            &args.app_name,
            &args.hostname,
        )
        .map_err(ProviderError::Api)?;

        let task = app_certificates_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// App certificates acme delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CertificateDetail result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn app_certificates_acme_delete(
        &self,
        args: &AppCertificatesAcmeDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CertificateDetail, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = app_certificates_acme_delete_builder(
            &self.http_client,
            &args.app_name,
            &args.hostname,
        )
        .map_err(ProviderError::Api)?;

        let task = app_certificates_acme_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// App certificates check.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CertificateCheckResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn app_certificates_check(
        &self,
        args: &AppCertificatesCheckArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CertificateCheckResponse, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = app_certificates_check_builder(
            &self.http_client,
            &args.app_name,
            &args.hostname,
        )
        .map_err(ProviderError::Api)?;

        let task = app_certificates_check_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// App certificates custom delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DestroyCustomCertificateResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn app_certificates_custom_delete(
        &self,
        args: &AppCertificatesCustomDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DestroyCustomCertificateResponse, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = app_certificates_custom_delete_builder(
            &self.http_client,
            &args.app_name,
            &args.hostname,
        )
        .map_err(ProviderError::Api)?;

        let task = app_certificates_custom_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// App create deploy token.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CreateAppResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn app_create_deploy_token(
        &self,
        args: &AppCreateDeployTokenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CreateAppResponse, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = app_create_deploy_token_builder(
            &self.http_client,
            &args.app_name,
        )
        .map_err(ProviderError::Api)?;

        let task = app_create_deploy_token_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// App i p assignments create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IPAssignment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn app_i_p_assignments_create(
        &self,
        args: &AppIPAssignmentsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IPAssignment, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = app_i_p_assignments_create_builder(
            &self.http_client,
            &args.app_name,
        )
        .map_err(ProviderError::Api)?;

        let task = app_i_p_assignments_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// App i p assignments delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the () result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn app_i_p_assignments_delete(
        &self,
        args: &AppIPAssignmentsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = app_i_p_assignments_delete_builder(
            &self.http_client,
            &args.app_name,
            &args.ip,
        )
        .map_err(ProviderError::Api)?;

        let task = app_i_p_assignments_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Machines create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Machine result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn machines_create(
        &self,
        args: &MachinesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Machine, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = machines_create_builder(
            &self.http_client,
            &args.app_name,
        )
        .map_err(ProviderError::Api)?;

        let task = machines_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Machines update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Machine result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn machines_update(
        &self,
        args: &MachinesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Machine, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = machines_update_builder(
            &self.http_client,
            &args.app_name,
            &args.machine_id,
        )
        .map_err(ProviderError::Api)?;

        let task = machines_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Machines delete.
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
    pub fn machines_delete(
        &self,
        args: &MachinesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = machines_delete_builder(
            &self.http_client,
            &args.app_name,
            &args.machine_id,
            &args.force,
        )
        .map_err(ProviderError::Api)?;

        let task = machines_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Machines cordon.
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
    pub fn machines_cordon(
        &self,
        args: &MachinesCordonArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = machines_cordon_builder(
            &self.http_client,
            &args.app_name,
            &args.machine_id,
        )
        .map_err(ProviderError::Api)?;

        let task = machines_cordon_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Machines exec.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Flydv1ExecResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn machines_exec(
        &self,
        args: &MachinesExecArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Flydv1ExecResponse, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = machines_exec_builder(
            &self.http_client,
            &args.app_name,
            &args.machine_id,
        )
        .map_err(ProviderError::Api)?;

        let task = machines_exec_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Machines create lease.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Lease result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn machines_create_lease(
        &self,
        args: &MachinesCreateLeaseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Lease, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = machines_create_lease_builder(
            &self.http_client,
            &args.app_name,
            &args.machine_id,
        )
        .map_err(ProviderError::Api)?;

        let task = machines_create_lease_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Machines release lease.
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
    pub fn machines_release_lease(
        &self,
        args: &MachinesReleaseLeaseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = machines_release_lease_builder(
            &self.http_client,
            &args.app_name,
            &args.machine_id,
        )
        .map_err(ProviderError::Api)?;

        let task = machines_release_lease_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Machines set memory limit.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the MainMemoryResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn machines_set_memory_limit(
        &self,
        args: &MachinesSetMemoryLimitArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<MainMemoryResponse, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = machines_set_memory_limit_builder(
            &self.http_client,
            &args.app_name,
            &args.machine_id,
        )
        .map_err(ProviderError::Api)?;

        let task = machines_set_memory_limit_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Machines reclaim memory.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the MainReclaimMemoryResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn machines_reclaim_memory(
        &self,
        args: &MachinesReclaimMemoryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<MainReclaimMemoryResponse, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = machines_reclaim_memory_builder(
            &self.http_client,
            &args.app_name,
            &args.machine_id,
        )
        .map_err(ProviderError::Api)?;

        let task = machines_reclaim_memory_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Machines update metadata.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the () result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn machines_update_metadata(
        &self,
        args: &MachinesUpdateMetadataArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = machines_update_metadata_builder(
            &self.http_client,
            &args.app_name,
            &args.machine_id,
        )
        .map_err(ProviderError::Api)?;

        let task = machines_update_metadata_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Machines upsert metadata.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the () result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn machines_upsert_metadata(
        &self,
        args: &MachinesUpsertMetadataArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = machines_upsert_metadata_builder(
            &self.http_client,
            &args.app_name,
            &args.machine_id,
            &args.key,
        )
        .map_err(ProviderError::Api)?;

        let task = machines_upsert_metadata_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Machines delete metadata.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the () result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn machines_delete_metadata(
        &self,
        args: &MachinesDeleteMetadataArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = machines_delete_metadata_builder(
            &self.http_client,
            &args.app_name,
            &args.machine_id,
            &args.key,
        )
        .map_err(ProviderError::Api)?;

        let task = machines_delete_metadata_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Machines restart.
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
    pub fn machines_restart(
        &self,
        args: &MachinesRestartArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = machines_restart_builder(
            &self.http_client,
            &args.app_name,
            &args.machine_id,
            &args.timeout,
            &args.signal,
        )
        .map_err(ProviderError::Api)?;

        let task = machines_restart_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Machines signal.
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
    pub fn machines_signal(
        &self,
        args: &MachinesSignalArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = machines_signal_builder(
            &self.http_client,
            &args.app_name,
            &args.machine_id,
        )
        .map_err(ProviderError::Api)?;

        let task = machines_signal_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Machines start.
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
    pub fn machines_start(
        &self,
        args: &MachinesStartArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = machines_start_builder(
            &self.http_client,
            &args.app_name,
            &args.machine_id,
        )
        .map_err(ProviderError::Api)?;

        let task = machines_start_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Machines stop.
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
    pub fn machines_stop(
        &self,
        args: &MachinesStopArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = machines_stop_builder(
            &self.http_client,
            &args.app_name,
            &args.machine_id,
        )
        .map_err(ProviderError::Api)?;

        let task = machines_stop_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Machines suspend.
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
    pub fn machines_suspend(
        &self,
        args: &MachinesSuspendArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = machines_suspend_builder(
            &self.http_client,
            &args.app_name,
            &args.machine_id,
        )
        .map_err(ProviderError::Api)?;

        let task = machines_suspend_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Machines uncordon.
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
    pub fn machines_uncordon(
        &self,
        args: &MachinesUncordonArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = machines_uncordon_builder(
            &self.http_client,
            &args.app_name,
            &args.machine_id,
        )
        .map_err(ProviderError::Api)?;

        let task = machines_uncordon_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Secretkey set.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SetSecretkeyResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn secretkey_set(
        &self,
        args: &SecretkeySetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SetSecretkeyResponse, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = secretkey_set_builder(
            &self.http_client,
            &args.app_name,
            &args.secret_name,
        )
        .map_err(ProviderError::Api)?;

        let task = secretkey_set_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Secretkey delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeleteSecretkeyResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn secretkey_delete(
        &self,
        args: &SecretkeyDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeleteSecretkeyResponse, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = secretkey_delete_builder(
            &self.http_client,
            &args.app_name,
            &args.secret_name,
        )
        .map_err(ProviderError::Api)?;

        let task = secretkey_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Secretkey decrypt.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DecryptSecretkeyResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn secretkey_decrypt(
        &self,
        args: &SecretkeyDecryptArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DecryptSecretkeyResponse, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = secretkey_decrypt_builder(
            &self.http_client,
            &args.app_name,
            &args.secret_name,
            &args.min_version,
        )
        .map_err(ProviderError::Api)?;

        let task = secretkey_decrypt_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Secretkey encrypt.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EncryptSecretkeyResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn secretkey_encrypt(
        &self,
        args: &SecretkeyEncryptArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EncryptSecretkeyResponse, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = secretkey_encrypt_builder(
            &self.http_client,
            &args.app_name,
            &args.secret_name,
            &args.min_version,
        )
        .map_err(ProviderError::Api)?;

        let task = secretkey_encrypt_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Secretkey generate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SetSecretkeyResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn secretkey_generate(
        &self,
        args: &SecretkeyGenerateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SetSecretkeyResponse, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = secretkey_generate_builder(
            &self.http_client,
            &args.app_name,
            &args.secret_name,
        )
        .map_err(ProviderError::Api)?;

        let task = secretkey_generate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Secretkey sign.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SignSecretkeyResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn secretkey_sign(
        &self,
        args: &SecretkeySignArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SignSecretkeyResponse, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = secretkey_sign_builder(
            &self.http_client,
            &args.app_name,
            &args.secret_name,
            &args.min_version,
        )
        .map_err(ProviderError::Api)?;

        let task = secretkey_sign_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Secretkey verify.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the () result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn secretkey_verify(
        &self,
        args: &SecretkeyVerifyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = secretkey_verify_builder(
            &self.http_client,
            &args.app_name,
            &args.secret_name,
            &args.min_version,
        )
        .map_err(ProviderError::Api)?;

        let task = secretkey_verify_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Secrets update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AppSecretsUpdateResp result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn secrets_update(
        &self,
        args: &SecretsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AppSecretsUpdateResp, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = secrets_update_builder(
            &self.http_client,
            &args.app_name,
        )
        .map_err(ProviderError::Api)?;

        let task = secrets_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Secret create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SetAppSecretResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn secret_create(
        &self,
        args: &SecretCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SetAppSecretResponse, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = secret_create_builder(
            &self.http_client,
            &args.app_name,
            &args.secret_name,
        )
        .map_err(ProviderError::Api)?;

        let task = secret_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Secret delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeleteAppSecretResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn secret_delete(
        &self,
        args: &SecretDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeleteAppSecretResponse, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = secret_delete_builder(
            &self.http_client,
            &args.app_name,
            &args.secret_name,
        )
        .map_err(ProviderError::Api)?;

        let task = secret_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Volumes create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Volume result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn volumes_create(
        &self,
        args: &VolumesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Volume, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = volumes_create_builder(
            &self.http_client,
            &args.app_name,
        )
        .map_err(ProviderError::Api)?;

        let task = volumes_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Volumes update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Volume result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn volumes_update(
        &self,
        args: &VolumesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Volume, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = volumes_update_builder(
            &self.http_client,
            &args.app_name,
            &args.volume_id,
        )
        .map_err(ProviderError::Api)?;

        let task = volumes_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Volume delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Volume result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn volume_delete(
        &self,
        args: &VolumeDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Volume, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = volume_delete_builder(
            &self.http_client,
            &args.app_name,
            &args.volume_id,
        )
        .map_err(ProviderError::Api)?;

        let task = volume_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Volumes extend.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ExtendVolumeResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn volumes_extend(
        &self,
        args: &VolumesExtendArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ExtendVolumeResponse, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = volumes_extend_builder(
            &self.http_client,
            &args.app_name,
            &args.volume_id,
        )
        .map_err(ProviderError::Api)?;

        let task = volumes_extend_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Create volume snapshot.
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
    pub fn create_volume_snapshot(
        &self,
        args: &CreateVolumeSnapshotArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = create_volume_snapshot_builder(
            &self.http_client,
            &args.app_name,
            &args.volume_id,
        )
        .map_err(ProviderError::Api)?;

        let task = create_volume_snapshot_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Platform placements post.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the MainGetPlacementsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn platform_placements_post(
        &self,
        args: &PlatformPlacementsPostArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<MainGetPlacementsResponse, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = platform_placements_post_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = platform_placements_post_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tokens request kms.
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
    pub fn tokens_request_kms(
        &self,
        args: &TokensRequestKmsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tokens_request_kms_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = tokens_request_kms_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tokens request o i d c.
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
    pub fn tokens_request_o_i_d_c(
        &self,
        args: &TokensRequestOIDCArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tokens_request_o_i_d_c_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = tokens_request_o_i_d_c_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
