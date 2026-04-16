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

use crate::providers::fly_io::clients::{
    apps_list_builder, apps_list_task,
    apps_create_builder, apps_create_task,
    apps_show_builder, apps_show_task,
    apps_delete_builder, apps_delete_task,
    app_certificates_list_builder, app_certificates_list_task,
    app_certificates_acme_create_builder, app_certificates_acme_create_task,
    app_certificates_custom_create_builder, app_certificates_custom_create_task,
    app_certificates_show_builder, app_certificates_show_task,
    app_certificates_delete_builder, app_certificates_delete_task,
    app_certificates_acme_delete_builder, app_certificates_acme_delete_task,
    app_certificates_check_builder, app_certificates_check_task,
    app_certificates_custom_delete_builder, app_certificates_custom_delete_task,
    app_create_deploy_token_builder, app_create_deploy_token_task,
    app_i_p_assignments_list_builder, app_i_p_assignments_list_task,
    app_i_p_assignments_create_builder, app_i_p_assignments_create_task,
    app_i_p_assignments_delete_builder, app_i_p_assignments_delete_task,
    machines_list_builder, machines_list_task,
    machines_create_builder, machines_create_task,
    machines_show_builder, machines_show_task,
    machines_update_builder, machines_update_task,
    machines_delete_builder, machines_delete_task,
    machines_cordon_builder, machines_cordon_task,
    machines_list_events_builder, machines_list_events_task,
    machines_exec_builder, machines_exec_task,
    machines_show_lease_builder, machines_show_lease_task,
    machines_create_lease_builder, machines_create_lease_task,
    machines_release_lease_builder, machines_release_lease_task,
    machines_get_memory_builder, machines_get_memory_task,
    machines_set_memory_limit_builder, machines_set_memory_limit_task,
    machines_reclaim_memory_builder, machines_reclaim_memory_task,
    machines_show_metadata_builder, machines_show_metadata_task,
    machines_update_metadata_builder, machines_update_metadata_task,
    machines_get_metadata_key_builder, machines_get_metadata_key_task,
    machines_upsert_metadata_builder, machines_upsert_metadata_task,
    machines_delete_metadata_builder, machines_delete_metadata_task,
    machines_list_processes_builder, machines_list_processes_task,
    machines_restart_builder, machines_restart_task,
    machines_signal_builder, machines_signal_task,
    machines_start_builder, machines_start_task,
    machines_stop_builder, machines_stop_task,
    machines_suspend_builder, machines_suspend_task,
    machines_uncordon_builder, machines_uncordon_task,
    machines_list_versions_builder, machines_list_versions_task,
    machines_wait_builder, machines_wait_task,
    secretkeys_list_builder, secretkeys_list_task,
    secretkey_get_builder, secretkey_get_task,
    secretkey_set_builder, secretkey_set_task,
    secretkey_delete_builder, secretkey_delete_task,
    secretkey_decrypt_builder, secretkey_decrypt_task,
    secretkey_encrypt_builder, secretkey_encrypt_task,
    secretkey_generate_builder, secretkey_generate_task,
    secretkey_sign_builder, secretkey_sign_task,
    secretkey_verify_builder, secretkey_verify_task,
    secrets_list_builder, secrets_list_task,
    secrets_update_builder, secrets_update_task,
    secret_get_builder, secret_get_task,
    secret_create_builder, secret_create_task,
    secret_delete_builder, secret_delete_task,
    volumes_list_builder, volumes_list_task,
    volumes_create_builder, volumes_create_task,
    volumes_get_by_id_builder, volumes_get_by_id_task,
    volumes_update_builder, volumes_update_task,
    volume_delete_builder, volume_delete_task,
    volumes_extend_builder, volumes_extend_task,
    volumes_list_snapshots_builder, volumes_list_snapshots_task,
    create_volume_snapshot_builder, create_volume_snapshot_task,
    machines_org_list_builder, machines_org_list_task,
    volumes_org_list_builder, volumes_org_list_task,
    platform_placements_post_builder, platform_placements_post_task,
    platform_regions_get_builder, platform_regions_get_task,
    tokens_request_kms_builder, tokens_request_kms_task,
    tokens_request_o_i_d_c_builder, tokens_request_o_i_d_c_task,
    current_token_show_builder, current_token_show_task,
};
use crate::providers::fly_io::clients::types::{ApiError, ApiPending};
use crate::providers::fly_io::clients::App;
use crate::providers::fly_io::clients::AppSecret;
use crate::providers::fly_io::clients::AppSecrets;
use crate::providers::fly_io::clients::AppSecretsUpdateResp;
use crate::providers::fly_io::clients::CertificateCheckResponse;
use crate::providers::fly_io::clients::CertificateDetail;
use crate::providers::fly_io::clients::CreateAppResponse;
use crate::providers::fly_io::clients::CurrentTokenResponse;
use crate::providers::fly_io::clients::DecryptSecretkeyResponse;
use crate::providers::fly_io::clients::DeleteAppSecretResponse;
use crate::providers::fly_io::clients::DeleteSecretkeyResponse;
use crate::providers::fly_io::clients::DestroyCustomCertificateResponse;
use crate::providers::fly_io::clients::EncryptSecretkeyResponse;
use crate::providers::fly_io::clients::ExtendVolumeResponse;
use crate::providers::fly_io::clients::Flydv1ExecResponse;
use crate::providers::fly_io::clients::IPAssignment;
use crate::providers::fly_io::clients::Lease;
use crate::providers::fly_io::clients::ListAppsResponse;
use crate::providers::fly_io::clients::ListCertificatesResponse;
use crate::providers::fly_io::clients::ListIPAssignmentsResponse;
use crate::providers::fly_io::clients::Machine;
use crate::providers::fly_io::clients::MainGetPlacementsResponse;
use crate::providers::fly_io::clients::MainMemoryResponse;
use crate::providers::fly_io::clients::MainReclaimMemoryResponse;
use crate::providers::fly_io::clients::MainRegionResponse;
use crate::providers::fly_io::clients::MetadataValueResponse;
use crate::providers::fly_io::clients::OrgMachinesResponse;
use crate::providers::fly_io::clients::OrgVolumesResponse;
use crate::providers::fly_io::clients::SecretKey;
use crate::providers::fly_io::clients::SecretKeys;
use crate::providers::fly_io::clients::SetAppSecretResponse;
use crate::providers::fly_io::clients::SetSecretkeyResponse;
use crate::providers::fly_io::clients::SignSecretkeyResponse;
use crate::providers::fly_io::clients::Volume;
use crate::providers::fly_io::clients::WaitMachineResponse;
use crate::providers::fly_io::clients::AppCertificatesAcmeCreateArgs;
use crate::providers::fly_io::clients::AppCertificatesAcmeDeleteArgs;
use crate::providers::fly_io::clients::AppCertificatesCheckArgs;
use crate::providers::fly_io::clients::AppCertificatesCustomCreateArgs;
use crate::providers::fly_io::clients::AppCertificatesCustomDeleteArgs;
use crate::providers::fly_io::clients::AppCertificatesDeleteArgs;
use crate::providers::fly_io::clients::AppCertificatesListArgs;
use crate::providers::fly_io::clients::AppCertificatesShowArgs;
use crate::providers::fly_io::clients::AppCreateDeployTokenArgs;
use crate::providers::fly_io::clients::AppIPAssignmentsCreateArgs;
use crate::providers::fly_io::clients::AppIPAssignmentsDeleteArgs;
use crate::providers::fly_io::clients::AppIPAssignmentsListArgs;
use crate::providers::fly_io::clients::AppsCreateArgs;
use crate::providers::fly_io::clients::AppsDeleteArgs;
use crate::providers::fly_io::clients::AppsListArgs;
use crate::providers::fly_io::clients::AppsShowArgs;
use crate::providers::fly_io::clients::CreateVolumeSnapshotArgs;
use crate::providers::fly_io::clients::MachinesCordonArgs;
use crate::providers::fly_io::clients::MachinesCreateArgs;
use crate::providers::fly_io::clients::MachinesCreateLeaseArgs;
use crate::providers::fly_io::clients::MachinesDeleteArgs;
use crate::providers::fly_io::clients::MachinesDeleteMetadataArgs;
use crate::providers::fly_io::clients::MachinesExecArgs;
use crate::providers::fly_io::clients::MachinesGetMemoryArgs;
use crate::providers::fly_io::clients::MachinesGetMetadataKeyArgs;
use crate::providers::fly_io::clients::MachinesListArgs;
use crate::providers::fly_io::clients::MachinesListEventsArgs;
use crate::providers::fly_io::clients::MachinesListProcessesArgs;
use crate::providers::fly_io::clients::MachinesListVersionsArgs;
use crate::providers::fly_io::clients::MachinesOrgListArgs;
use crate::providers::fly_io::clients::MachinesReclaimMemoryArgs;
use crate::providers::fly_io::clients::MachinesReleaseLeaseArgs;
use crate::providers::fly_io::clients::MachinesRestartArgs;
use crate::providers::fly_io::clients::MachinesSetMemoryLimitArgs;
use crate::providers::fly_io::clients::MachinesShowArgs;
use crate::providers::fly_io::clients::MachinesShowLeaseArgs;
use crate::providers::fly_io::clients::MachinesShowMetadataArgs;
use crate::providers::fly_io::clients::MachinesSignalArgs;
use crate::providers::fly_io::clients::MachinesStartArgs;
use crate::providers::fly_io::clients::MachinesStopArgs;
use crate::providers::fly_io::clients::MachinesSuspendArgs;
use crate::providers::fly_io::clients::MachinesUncordonArgs;
use crate::providers::fly_io::clients::MachinesUpdateArgs;
use crate::providers::fly_io::clients::MachinesUpdateMetadataArgs;
use crate::providers::fly_io::clients::MachinesUpsertMetadataArgs;
use crate::providers::fly_io::clients::MachinesWaitArgs;
use crate::providers::fly_io::clients::PlatformPlacementsPostArgs;
use crate::providers::fly_io::clients::SecretCreateArgs;
use crate::providers::fly_io::clients::SecretDeleteArgs;
use crate::providers::fly_io::clients::SecretGetArgs;
use crate::providers::fly_io::clients::SecretkeyDecryptArgs;
use crate::providers::fly_io::clients::SecretkeyDeleteArgs;
use crate::providers::fly_io::clients::SecretkeyEncryptArgs;
use crate::providers::fly_io::clients::SecretkeyGenerateArgs;
use crate::providers::fly_io::clients::SecretkeyGetArgs;
use crate::providers::fly_io::clients::SecretkeySetArgs;
use crate::providers::fly_io::clients::SecretkeySignArgs;
use crate::providers::fly_io::clients::SecretkeyVerifyArgs;
use crate::providers::fly_io::clients::SecretkeysListArgs;
use crate::providers::fly_io::clients::SecretsListArgs;
use crate::providers::fly_io::clients::SecretsUpdateArgs;
use crate::providers::fly_io::clients::TokensRequestOIDCArgs;
use crate::providers::fly_io::clients::VolumeDeleteArgs;
use crate::providers::fly_io::clients::VolumesCreateArgs;
use crate::providers::fly_io::clients::VolumesExtendArgs;
use crate::providers::fly_io::clients::VolumesGetByIdArgs;
use crate::providers::fly_io::clients::VolumesListArgs;
use crate::providers::fly_io::clients::VolumesListSnapshotsArgs;
use crate::providers::fly_io::clients::VolumesOrgListArgs;
use crate::providers::fly_io::clients::VolumesUpdateArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// FlyIoProvider with automatic state tracking.
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
/// let provider = FlyIoProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct FlyIoProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> FlyIoProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new FlyIoProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new FlyIoProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Apps list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAppsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apps_list(
        &self,
        args: &AppsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAppsResponse, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apps_list_builder(
            &self.http_client,
            &args.org_slug,
            &args.app_role,
        )
        .map_err(ProviderError::Api)?;

        let task = apps_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Apps show.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the App result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apps_show(
        &self,
        args: &AppsShowArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<App, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apps_show_builder(
            &self.http_client,
            &args.app_name,
        )
        .map_err(ProviderError::Api)?;

        let task = apps_show_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// App certificates list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCertificatesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn app_certificates_list(
        &self,
        args: &AppCertificatesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCertificatesResponse, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = app_certificates_list_builder(
            &self.http_client,
            &args.app_name,
            &args.filter,
            &args.cursor,
            &args.limit,
        )
        .map_err(ProviderError::Api)?;

        let task = app_certificates_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// App certificates show.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn app_certificates_show(
        &self,
        args: &AppCertificatesShowArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CertificateDetail, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = app_certificates_show_builder(
            &self.http_client,
            &args.app_name,
            &args.hostname,
        )
        .map_err(ProviderError::Api)?;

        let task = app_certificates_show_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// App i p assignments list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListIPAssignmentsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn app_i_p_assignments_list(
        &self,
        args: &AppIPAssignmentsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListIPAssignmentsResponse, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = app_i_p_assignments_list_builder(
            &self.http_client,
            &args.app_name,
        )
        .map_err(ProviderError::Api)?;

        let task = app_i_p_assignments_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Machines list.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn machines_list(
        &self,
        args: &MachinesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = machines_list_builder(
            &self.http_client,
            &args.app_name,
            &args.include_deleted,
            &args.region,
            &args.state,
            &args.summary,
        )
        .map_err(ProviderError::Api)?;

        let task = machines_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Machines show.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn machines_show(
        &self,
        args: &MachinesShowArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Machine, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = machines_show_builder(
            &self.http_client,
            &args.app_name,
            &args.machine_id,
        )
        .map_err(ProviderError::Api)?;

        let task = machines_show_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Machines list events.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn machines_list_events(
        &self,
        args: &MachinesListEventsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = machines_list_events_builder(
            &self.http_client,
            &args.app_name,
            &args.machine_id,
            &args.limit,
        )
        .map_err(ProviderError::Api)?;

        let task = machines_list_events_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Machines show lease.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn machines_show_lease(
        &self,
        args: &MachinesShowLeaseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Lease, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = machines_show_lease_builder(
            &self.http_client,
            &args.app_name,
            &args.machine_id,
        )
        .map_err(ProviderError::Api)?;

        let task = machines_show_lease_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Machines get memory.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn machines_get_memory(
        &self,
        args: &MachinesGetMemoryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<MainMemoryResponse, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = machines_get_memory_builder(
            &self.http_client,
            &args.app_name,
            &args.machine_id,
        )
        .map_err(ProviderError::Api)?;

        let task = machines_get_memory_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Machines show metadata.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn machines_show_metadata(
        &self,
        args: &MachinesShowMetadataArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = machines_show_metadata_builder(
            &self.http_client,
            &args.app_name,
            &args.machine_id,
        )
        .map_err(ProviderError::Api)?;

        let task = machines_show_metadata_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Machines get metadata key.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the MetadataValueResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn machines_get_metadata_key(
        &self,
        args: &MachinesGetMetadataKeyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<MetadataValueResponse, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = machines_get_metadata_key_builder(
            &self.http_client,
            &args.app_name,
            &args.machine_id,
            &args.key,
        )
        .map_err(ProviderError::Api)?;

        let task = machines_get_metadata_key_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Machines list processes.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn machines_list_processes(
        &self,
        args: &MachinesListProcessesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = machines_list_processes_builder(
            &self.http_client,
            &args.app_name,
            &args.machine_id,
            &args.sort_by,
            &args.order,
        )
        .map_err(ProviderError::Api)?;

        let task = machines_list_processes_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Machines list versions.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn machines_list_versions(
        &self,
        args: &MachinesListVersionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = machines_list_versions_builder(
            &self.http_client,
            &args.app_name,
            &args.machine_id,
        )
        .map_err(ProviderError::Api)?;

        let task = machines_list_versions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Machines wait.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WaitMachineResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn machines_wait(
        &self,
        args: &MachinesWaitArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WaitMachineResponse, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = machines_wait_builder(
            &self.http_client,
            &args.app_name,
            &args.machine_id,
            &args.version,
            &args.instance_id,
            &args.from_event_id,
            &args.timeout,
            &args.state,
        )
        .map_err(ProviderError::Api)?;

        let task = machines_wait_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Secretkeys list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SecretKeys result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn secretkeys_list(
        &self,
        args: &SecretkeysListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SecretKeys, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = secretkeys_list_builder(
            &self.http_client,
            &args.app_name,
            &args.min_version,
            &args.types,
        )
        .map_err(ProviderError::Api)?;

        let task = secretkeys_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Secretkey get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SecretKey result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn secretkey_get(
        &self,
        args: &SecretkeyGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SecretKey, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = secretkey_get_builder(
            &self.http_client,
            &args.app_name,
            &args.secret_name,
            &args.min_version,
        )
        .map_err(ProviderError::Api)?;

        let task = secretkey_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Secrets list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AppSecrets result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn secrets_list(
        &self,
        args: &SecretsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AppSecrets, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = secrets_list_builder(
            &self.http_client,
            &args.app_name,
            &args.min_version,
            &args.show_secrets,
        )
        .map_err(ProviderError::Api)?;

        let task = secrets_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Secret get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AppSecret result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn secret_get(
        &self,
        args: &SecretGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AppSecret, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = secret_get_builder(
            &self.http_client,
            &args.app_name,
            &args.secret_name,
            &args.min_version,
            &args.show_secrets,
        )
        .map_err(ProviderError::Api)?;

        let task = secret_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Volumes list.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn volumes_list(
        &self,
        args: &VolumesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = volumes_list_builder(
            &self.http_client,
            &args.app_name,
            &args.summary,
        )
        .map_err(ProviderError::Api)?;

        let task = volumes_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Volumes get by id.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn volumes_get_by_id(
        &self,
        args: &VolumesGetByIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Volume, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = volumes_get_by_id_builder(
            &self.http_client,
            &args.app_name,
            &args.volume_id,
        )
        .map_err(ProviderError::Api)?;

        let task = volumes_get_by_id_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Volumes list snapshots.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn volumes_list_snapshots(
        &self,
        args: &VolumesListSnapshotsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = volumes_list_snapshots_builder(
            &self.http_client,
            &args.app_name,
            &args.volume_id,
        )
        .map_err(ProviderError::Api)?;

        let task = volumes_list_snapshots_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Machines org list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the OrgMachinesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn machines_org_list(
        &self,
        args: &MachinesOrgListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<OrgMachinesResponse, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = machines_org_list_builder(
            &self.http_client,
            &args.org_slug,
            &args.include_deleted,
            &args.region,
            &args.state,
            &args.summary,
            &args.updated_after,
            &args.cursor,
            &args.limit,
        )
        .map_err(ProviderError::Api)?;

        let task = machines_org_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Volumes org list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the OrgVolumesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn volumes_org_list(
        &self,
        args: &VolumesOrgListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<OrgVolumesResponse, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = volumes_org_list_builder(
            &self.http_client,
            &args.org_slug,
            &args.include_deleted,
            &args.region,
            &args.state,
            &args.summary,
            &args.updated_after,
            &args.cursor,
            &args.limit,
        )
        .map_err(ProviderError::Api)?;

        let task = volumes_org_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Platform regions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the MainRegionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn platform_regions_get(
        &self,
        args: &PlatformRegionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<MainRegionResponse, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = platform_regions_get_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = platform_regions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Current token show.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CurrentTokenResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn current_token_show(
        &self,
        args: &CurrentTokenShowArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CurrentTokenResponse, ProviderError<ApiError>>,
            P = crate::providers::fly_io::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = current_token_show_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = current_token_show_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
