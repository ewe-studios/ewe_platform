//! CertificatemanagerProvider - State-aware certificatemanager API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       certificatemanager API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::certificatemanager::{
    certificatemanager_projects_locations_certificate_issuance_configs_create_builder, certificatemanager_projects_locations_certificate_issuance_configs_create_task,
    certificatemanager_projects_locations_certificate_issuance_configs_delete_builder, certificatemanager_projects_locations_certificate_issuance_configs_delete_task,
    certificatemanager_projects_locations_certificate_issuance_configs_patch_builder, certificatemanager_projects_locations_certificate_issuance_configs_patch_task,
    certificatemanager_projects_locations_certificate_maps_create_builder, certificatemanager_projects_locations_certificate_maps_create_task,
    certificatemanager_projects_locations_certificate_maps_delete_builder, certificatemanager_projects_locations_certificate_maps_delete_task,
    certificatemanager_projects_locations_certificate_maps_patch_builder, certificatemanager_projects_locations_certificate_maps_patch_task,
    certificatemanager_projects_locations_certificate_maps_certificate_map_entries_create_builder, certificatemanager_projects_locations_certificate_maps_certificate_map_entries_create_task,
    certificatemanager_projects_locations_certificate_maps_certificate_map_entries_delete_builder, certificatemanager_projects_locations_certificate_maps_certificate_map_entries_delete_task,
    certificatemanager_projects_locations_certificate_maps_certificate_map_entries_patch_builder, certificatemanager_projects_locations_certificate_maps_certificate_map_entries_patch_task,
    certificatemanager_projects_locations_certificates_create_builder, certificatemanager_projects_locations_certificates_create_task,
    certificatemanager_projects_locations_certificates_delete_builder, certificatemanager_projects_locations_certificates_delete_task,
    certificatemanager_projects_locations_certificates_patch_builder, certificatemanager_projects_locations_certificates_patch_task,
    certificatemanager_projects_locations_dns_authorizations_create_builder, certificatemanager_projects_locations_dns_authorizations_create_task,
    certificatemanager_projects_locations_dns_authorizations_delete_builder, certificatemanager_projects_locations_dns_authorizations_delete_task,
    certificatemanager_projects_locations_dns_authorizations_patch_builder, certificatemanager_projects_locations_dns_authorizations_patch_task,
    certificatemanager_projects_locations_operations_cancel_builder, certificatemanager_projects_locations_operations_cancel_task,
    certificatemanager_projects_locations_operations_delete_builder, certificatemanager_projects_locations_operations_delete_task,
    certificatemanager_projects_locations_trust_configs_create_builder, certificatemanager_projects_locations_trust_configs_create_task,
    certificatemanager_projects_locations_trust_configs_delete_builder, certificatemanager_projects_locations_trust_configs_delete_task,
    certificatemanager_projects_locations_trust_configs_patch_builder, certificatemanager_projects_locations_trust_configs_patch_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::certificatemanager::Empty;
use crate::providers::gcp::clients::certificatemanager::Operation;
use crate::providers::gcp::clients::certificatemanager::CertificatemanagerProjectsLocationsCertificateIssuanceConfigsCreateArgs;
use crate::providers::gcp::clients::certificatemanager::CertificatemanagerProjectsLocationsCertificateIssuanceConfigsDeleteArgs;
use crate::providers::gcp::clients::certificatemanager::CertificatemanagerProjectsLocationsCertificateIssuanceConfigsPatchArgs;
use crate::providers::gcp::clients::certificatemanager::CertificatemanagerProjectsLocationsCertificateMapsCertificateMapEntriesCreateArgs;
use crate::providers::gcp::clients::certificatemanager::CertificatemanagerProjectsLocationsCertificateMapsCertificateMapEntriesDeleteArgs;
use crate::providers::gcp::clients::certificatemanager::CertificatemanagerProjectsLocationsCertificateMapsCertificateMapEntriesPatchArgs;
use crate::providers::gcp::clients::certificatemanager::CertificatemanagerProjectsLocationsCertificateMapsCreateArgs;
use crate::providers::gcp::clients::certificatemanager::CertificatemanagerProjectsLocationsCertificateMapsDeleteArgs;
use crate::providers::gcp::clients::certificatemanager::CertificatemanagerProjectsLocationsCertificateMapsPatchArgs;
use crate::providers::gcp::clients::certificatemanager::CertificatemanagerProjectsLocationsCertificatesCreateArgs;
use crate::providers::gcp::clients::certificatemanager::CertificatemanagerProjectsLocationsCertificatesDeleteArgs;
use crate::providers::gcp::clients::certificatemanager::CertificatemanagerProjectsLocationsCertificatesPatchArgs;
use crate::providers::gcp::clients::certificatemanager::CertificatemanagerProjectsLocationsDnsAuthorizationsCreateArgs;
use crate::providers::gcp::clients::certificatemanager::CertificatemanagerProjectsLocationsDnsAuthorizationsDeleteArgs;
use crate::providers::gcp::clients::certificatemanager::CertificatemanagerProjectsLocationsDnsAuthorizationsPatchArgs;
use crate::providers::gcp::clients::certificatemanager::CertificatemanagerProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::certificatemanager::CertificatemanagerProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::certificatemanager::CertificatemanagerProjectsLocationsTrustConfigsCreateArgs;
use crate::providers::gcp::clients::certificatemanager::CertificatemanagerProjectsLocationsTrustConfigsDeleteArgs;
use crate::providers::gcp::clients::certificatemanager::CertificatemanagerProjectsLocationsTrustConfigsPatchArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// CertificatemanagerProvider with automatic state tracking.
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
/// let provider = CertificatemanagerProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct CertificatemanagerProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> CertificatemanagerProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new CertificatemanagerProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Certificatemanager projects locations certificate issuance configs create.
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
    pub fn certificatemanager_projects_locations_certificate_issuance_configs_create(
        &self,
        args: &CertificatemanagerProjectsLocationsCertificateIssuanceConfigsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = certificatemanager_projects_locations_certificate_issuance_configs_create_builder(
            &self.http_client,
            &args.parent,
            &args.certificateIssuanceConfigId,
        )
        .map_err(ProviderError::Api)?;

        let task = certificatemanager_projects_locations_certificate_issuance_configs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Certificatemanager projects locations certificate issuance configs delete.
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
    pub fn certificatemanager_projects_locations_certificate_issuance_configs_delete(
        &self,
        args: &CertificatemanagerProjectsLocationsCertificateIssuanceConfigsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = certificatemanager_projects_locations_certificate_issuance_configs_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = certificatemanager_projects_locations_certificate_issuance_configs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Certificatemanager projects locations certificate issuance configs patch.
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
    pub fn certificatemanager_projects_locations_certificate_issuance_configs_patch(
        &self,
        args: &CertificatemanagerProjectsLocationsCertificateIssuanceConfigsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = certificatemanager_projects_locations_certificate_issuance_configs_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = certificatemanager_projects_locations_certificate_issuance_configs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Certificatemanager projects locations certificate maps create.
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
    pub fn certificatemanager_projects_locations_certificate_maps_create(
        &self,
        args: &CertificatemanagerProjectsLocationsCertificateMapsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = certificatemanager_projects_locations_certificate_maps_create_builder(
            &self.http_client,
            &args.parent,
            &args.certificateMapId,
        )
        .map_err(ProviderError::Api)?;

        let task = certificatemanager_projects_locations_certificate_maps_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Certificatemanager projects locations certificate maps delete.
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
    pub fn certificatemanager_projects_locations_certificate_maps_delete(
        &self,
        args: &CertificatemanagerProjectsLocationsCertificateMapsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = certificatemanager_projects_locations_certificate_maps_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = certificatemanager_projects_locations_certificate_maps_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Certificatemanager projects locations certificate maps patch.
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
    pub fn certificatemanager_projects_locations_certificate_maps_patch(
        &self,
        args: &CertificatemanagerProjectsLocationsCertificateMapsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = certificatemanager_projects_locations_certificate_maps_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = certificatemanager_projects_locations_certificate_maps_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Certificatemanager projects locations certificate maps certificate map entries create.
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
    pub fn certificatemanager_projects_locations_certificate_maps_certificate_map_entries_create(
        &self,
        args: &CertificatemanagerProjectsLocationsCertificateMapsCertificateMapEntriesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = certificatemanager_projects_locations_certificate_maps_certificate_map_entries_create_builder(
            &self.http_client,
            &args.parent,
            &args.certificateMapEntryId,
        )
        .map_err(ProviderError::Api)?;

        let task = certificatemanager_projects_locations_certificate_maps_certificate_map_entries_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Certificatemanager projects locations certificate maps certificate map entries delete.
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
    pub fn certificatemanager_projects_locations_certificate_maps_certificate_map_entries_delete(
        &self,
        args: &CertificatemanagerProjectsLocationsCertificateMapsCertificateMapEntriesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = certificatemanager_projects_locations_certificate_maps_certificate_map_entries_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = certificatemanager_projects_locations_certificate_maps_certificate_map_entries_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Certificatemanager projects locations certificate maps certificate map entries patch.
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
    pub fn certificatemanager_projects_locations_certificate_maps_certificate_map_entries_patch(
        &self,
        args: &CertificatemanagerProjectsLocationsCertificateMapsCertificateMapEntriesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = certificatemanager_projects_locations_certificate_maps_certificate_map_entries_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = certificatemanager_projects_locations_certificate_maps_certificate_map_entries_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Certificatemanager projects locations certificates create.
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
    pub fn certificatemanager_projects_locations_certificates_create(
        &self,
        args: &CertificatemanagerProjectsLocationsCertificatesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = certificatemanager_projects_locations_certificates_create_builder(
            &self.http_client,
            &args.parent,
            &args.certificateId,
        )
        .map_err(ProviderError::Api)?;

        let task = certificatemanager_projects_locations_certificates_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Certificatemanager projects locations certificates delete.
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
    pub fn certificatemanager_projects_locations_certificates_delete(
        &self,
        args: &CertificatemanagerProjectsLocationsCertificatesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = certificatemanager_projects_locations_certificates_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = certificatemanager_projects_locations_certificates_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Certificatemanager projects locations certificates patch.
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
    pub fn certificatemanager_projects_locations_certificates_patch(
        &self,
        args: &CertificatemanagerProjectsLocationsCertificatesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = certificatemanager_projects_locations_certificates_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = certificatemanager_projects_locations_certificates_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Certificatemanager projects locations dns authorizations create.
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
    pub fn certificatemanager_projects_locations_dns_authorizations_create(
        &self,
        args: &CertificatemanagerProjectsLocationsDnsAuthorizationsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = certificatemanager_projects_locations_dns_authorizations_create_builder(
            &self.http_client,
            &args.parent,
            &args.dnsAuthorizationId,
        )
        .map_err(ProviderError::Api)?;

        let task = certificatemanager_projects_locations_dns_authorizations_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Certificatemanager projects locations dns authorizations delete.
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
    pub fn certificatemanager_projects_locations_dns_authorizations_delete(
        &self,
        args: &CertificatemanagerProjectsLocationsDnsAuthorizationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = certificatemanager_projects_locations_dns_authorizations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = certificatemanager_projects_locations_dns_authorizations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Certificatemanager projects locations dns authorizations patch.
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
    pub fn certificatemanager_projects_locations_dns_authorizations_patch(
        &self,
        args: &CertificatemanagerProjectsLocationsDnsAuthorizationsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = certificatemanager_projects_locations_dns_authorizations_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = certificatemanager_projects_locations_dns_authorizations_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Certificatemanager projects locations operations cancel.
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
    pub fn certificatemanager_projects_locations_operations_cancel(
        &self,
        args: &CertificatemanagerProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = certificatemanager_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = certificatemanager_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Certificatemanager projects locations operations delete.
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
    pub fn certificatemanager_projects_locations_operations_delete(
        &self,
        args: &CertificatemanagerProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = certificatemanager_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = certificatemanager_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Certificatemanager projects locations trust configs create.
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
    pub fn certificatemanager_projects_locations_trust_configs_create(
        &self,
        args: &CertificatemanagerProjectsLocationsTrustConfigsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = certificatemanager_projects_locations_trust_configs_create_builder(
            &self.http_client,
            &args.parent,
            &args.trustConfigId,
        )
        .map_err(ProviderError::Api)?;

        let task = certificatemanager_projects_locations_trust_configs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Certificatemanager projects locations trust configs delete.
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
    pub fn certificatemanager_projects_locations_trust_configs_delete(
        &self,
        args: &CertificatemanagerProjectsLocationsTrustConfigsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = certificatemanager_projects_locations_trust_configs_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
        )
        .map_err(ProviderError::Api)?;

        let task = certificatemanager_projects_locations_trust_configs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Certificatemanager projects locations trust configs patch.
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
    pub fn certificatemanager_projects_locations_trust_configs_patch(
        &self,
        args: &CertificatemanagerProjectsLocationsTrustConfigsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = certificatemanager_projects_locations_trust_configs_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = certificatemanager_projects_locations_trust_configs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
