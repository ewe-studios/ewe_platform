//! SecuritypostureProvider - State-aware securityposture API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       securityposture API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::securityposture::{
    securityposture_organizations_locations_operations_cancel_builder, securityposture_organizations_locations_operations_cancel_task,
    securityposture_organizations_locations_operations_delete_builder, securityposture_organizations_locations_operations_delete_task,
    securityposture_organizations_locations_posture_deployments_create_builder, securityposture_organizations_locations_posture_deployments_create_task,
    securityposture_organizations_locations_posture_deployments_delete_builder, securityposture_organizations_locations_posture_deployments_delete_task,
    securityposture_organizations_locations_posture_deployments_patch_builder, securityposture_organizations_locations_posture_deployments_patch_task,
    securityposture_organizations_locations_postures_create_builder, securityposture_organizations_locations_postures_create_task,
    securityposture_organizations_locations_postures_delete_builder, securityposture_organizations_locations_postures_delete_task,
    securityposture_organizations_locations_postures_extract_builder, securityposture_organizations_locations_postures_extract_task,
    securityposture_organizations_locations_postures_patch_builder, securityposture_organizations_locations_postures_patch_task,
    securityposture_organizations_locations_reports_create_ia_c_validation_report_builder, securityposture_organizations_locations_reports_create_ia_c_validation_report_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::securityposture::Empty;
use crate::providers::gcp::clients::securityposture::Operation;
use crate::providers::gcp::clients::securityposture::SecuritypostureOrganizationsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::securityposture::SecuritypostureOrganizationsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::securityposture::SecuritypostureOrganizationsLocationsPostureDeploymentsCreateArgs;
use crate::providers::gcp::clients::securityposture::SecuritypostureOrganizationsLocationsPostureDeploymentsDeleteArgs;
use crate::providers::gcp::clients::securityposture::SecuritypostureOrganizationsLocationsPostureDeploymentsPatchArgs;
use crate::providers::gcp::clients::securityposture::SecuritypostureOrganizationsLocationsPosturesCreateArgs;
use crate::providers::gcp::clients::securityposture::SecuritypostureOrganizationsLocationsPosturesDeleteArgs;
use crate::providers::gcp::clients::securityposture::SecuritypostureOrganizationsLocationsPosturesExtractArgs;
use crate::providers::gcp::clients::securityposture::SecuritypostureOrganizationsLocationsPosturesPatchArgs;
use crate::providers::gcp::clients::securityposture::SecuritypostureOrganizationsLocationsReportsCreateIaCValidationReportArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// SecuritypostureProvider with automatic state tracking.
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
/// let provider = SecuritypostureProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct SecuritypostureProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> SecuritypostureProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new SecuritypostureProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Securityposture organizations locations operations cancel.
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
    pub fn securityposture_organizations_locations_operations_cancel(
        &self,
        args: &SecuritypostureOrganizationsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securityposture_organizations_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securityposture_organizations_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securityposture organizations locations operations delete.
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
    pub fn securityposture_organizations_locations_operations_delete(
        &self,
        args: &SecuritypostureOrganizationsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securityposture_organizations_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securityposture_organizations_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securityposture organizations locations posture deployments create.
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
    pub fn securityposture_organizations_locations_posture_deployments_create(
        &self,
        args: &SecuritypostureOrganizationsLocationsPostureDeploymentsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securityposture_organizations_locations_posture_deployments_create_builder(
            &self.http_client,
            &args.parent,
            &args.postureDeploymentId,
        )
        .map_err(ProviderError::Api)?;

        let task = securityposture_organizations_locations_posture_deployments_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securityposture organizations locations posture deployments delete.
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
    pub fn securityposture_organizations_locations_posture_deployments_delete(
        &self,
        args: &SecuritypostureOrganizationsLocationsPostureDeploymentsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securityposture_organizations_locations_posture_deployments_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
        )
        .map_err(ProviderError::Api)?;

        let task = securityposture_organizations_locations_posture_deployments_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securityposture organizations locations posture deployments patch.
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
    pub fn securityposture_organizations_locations_posture_deployments_patch(
        &self,
        args: &SecuritypostureOrganizationsLocationsPostureDeploymentsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securityposture_organizations_locations_posture_deployments_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = securityposture_organizations_locations_posture_deployments_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securityposture organizations locations postures create.
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
    pub fn securityposture_organizations_locations_postures_create(
        &self,
        args: &SecuritypostureOrganizationsLocationsPosturesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securityposture_organizations_locations_postures_create_builder(
            &self.http_client,
            &args.parent,
            &args.postureId,
        )
        .map_err(ProviderError::Api)?;

        let task = securityposture_organizations_locations_postures_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securityposture organizations locations postures delete.
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
    pub fn securityposture_organizations_locations_postures_delete(
        &self,
        args: &SecuritypostureOrganizationsLocationsPosturesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securityposture_organizations_locations_postures_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
        )
        .map_err(ProviderError::Api)?;

        let task = securityposture_organizations_locations_postures_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securityposture organizations locations postures extract.
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
    pub fn securityposture_organizations_locations_postures_extract(
        &self,
        args: &SecuritypostureOrganizationsLocationsPosturesExtractArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securityposture_organizations_locations_postures_extract_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = securityposture_organizations_locations_postures_extract_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securityposture organizations locations postures patch.
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
    pub fn securityposture_organizations_locations_postures_patch(
        &self,
        args: &SecuritypostureOrganizationsLocationsPosturesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securityposture_organizations_locations_postures_patch_builder(
            &self.http_client,
            &args.name,
            &args.revisionId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = securityposture_organizations_locations_postures_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securityposture organizations locations reports create ia c validation report.
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
    pub fn securityposture_organizations_locations_reports_create_ia_c_validation_report(
        &self,
        args: &SecuritypostureOrganizationsLocationsReportsCreateIaCValidationReportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securityposture_organizations_locations_reports_create_ia_c_validation_report_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = securityposture_organizations_locations_reports_create_ia_c_validation_report_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
