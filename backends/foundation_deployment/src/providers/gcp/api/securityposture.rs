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
    securityposture_organizations_locations_operations_get_builder, securityposture_organizations_locations_operations_get_task,
    securityposture_organizations_locations_operations_list_builder, securityposture_organizations_locations_operations_list_task,
    securityposture_organizations_locations_posture_deployments_create_builder, securityposture_organizations_locations_posture_deployments_create_task,
    securityposture_organizations_locations_posture_deployments_delete_builder, securityposture_organizations_locations_posture_deployments_delete_task,
    securityposture_organizations_locations_posture_deployments_get_builder, securityposture_organizations_locations_posture_deployments_get_task,
    securityposture_organizations_locations_posture_deployments_list_builder, securityposture_organizations_locations_posture_deployments_list_task,
    securityposture_organizations_locations_posture_deployments_patch_builder, securityposture_organizations_locations_posture_deployments_patch_task,
    securityposture_organizations_locations_posture_templates_get_builder, securityposture_organizations_locations_posture_templates_get_task,
    securityposture_organizations_locations_posture_templates_list_builder, securityposture_organizations_locations_posture_templates_list_task,
    securityposture_organizations_locations_postures_create_builder, securityposture_organizations_locations_postures_create_task,
    securityposture_organizations_locations_postures_delete_builder, securityposture_organizations_locations_postures_delete_task,
    securityposture_organizations_locations_postures_extract_builder, securityposture_organizations_locations_postures_extract_task,
    securityposture_organizations_locations_postures_get_builder, securityposture_organizations_locations_postures_get_task,
    securityposture_organizations_locations_postures_list_builder, securityposture_organizations_locations_postures_list_task,
    securityposture_organizations_locations_postures_list_revisions_builder, securityposture_organizations_locations_postures_list_revisions_task,
    securityposture_organizations_locations_postures_patch_builder, securityposture_organizations_locations_postures_patch_task,
    securityposture_organizations_locations_reports_create_ia_c_validation_report_builder, securityposture_organizations_locations_reports_create_ia_c_validation_report_task,
    securityposture_organizations_locations_reports_get_builder, securityposture_organizations_locations_reports_get_task,
    securityposture_organizations_locations_reports_list_builder, securityposture_organizations_locations_reports_list_task,
    securityposture_projects_locations_get_builder, securityposture_projects_locations_get_task,
    securityposture_projects_locations_list_builder, securityposture_projects_locations_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::securityposture::Empty;
use crate::providers::gcp::clients::securityposture::ListLocationsResponse;
use crate::providers::gcp::clients::securityposture::ListOperationsResponse;
use crate::providers::gcp::clients::securityposture::ListPostureDeploymentsResponse;
use crate::providers::gcp::clients::securityposture::ListPostureRevisionsResponse;
use crate::providers::gcp::clients::securityposture::ListPostureTemplatesResponse;
use crate::providers::gcp::clients::securityposture::ListPosturesResponse;
use crate::providers::gcp::clients::securityposture::ListReportsResponse;
use crate::providers::gcp::clients::securityposture::Location;
use crate::providers::gcp::clients::securityposture::Operation;
use crate::providers::gcp::clients::securityposture::Posture;
use crate::providers::gcp::clients::securityposture::PostureDeployment;
use crate::providers::gcp::clients::securityposture::PostureTemplate;
use crate::providers::gcp::clients::securityposture::Report;
use crate::providers::gcp::clients::securityposture::SecuritypostureOrganizationsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::securityposture::SecuritypostureOrganizationsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::securityposture::SecuritypostureOrganizationsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::securityposture::SecuritypostureOrganizationsLocationsOperationsListArgs;
use crate::providers::gcp::clients::securityposture::SecuritypostureOrganizationsLocationsPostureDeploymentsCreateArgs;
use crate::providers::gcp::clients::securityposture::SecuritypostureOrganizationsLocationsPostureDeploymentsDeleteArgs;
use crate::providers::gcp::clients::securityposture::SecuritypostureOrganizationsLocationsPostureDeploymentsGetArgs;
use crate::providers::gcp::clients::securityposture::SecuritypostureOrganizationsLocationsPostureDeploymentsListArgs;
use crate::providers::gcp::clients::securityposture::SecuritypostureOrganizationsLocationsPostureDeploymentsPatchArgs;
use crate::providers::gcp::clients::securityposture::SecuritypostureOrganizationsLocationsPostureTemplatesGetArgs;
use crate::providers::gcp::clients::securityposture::SecuritypostureOrganizationsLocationsPostureTemplatesListArgs;
use crate::providers::gcp::clients::securityposture::SecuritypostureOrganizationsLocationsPosturesCreateArgs;
use crate::providers::gcp::clients::securityposture::SecuritypostureOrganizationsLocationsPosturesDeleteArgs;
use crate::providers::gcp::clients::securityposture::SecuritypostureOrganizationsLocationsPosturesExtractArgs;
use crate::providers::gcp::clients::securityposture::SecuritypostureOrganizationsLocationsPosturesGetArgs;
use crate::providers::gcp::clients::securityposture::SecuritypostureOrganizationsLocationsPosturesListArgs;
use crate::providers::gcp::clients::securityposture::SecuritypostureOrganizationsLocationsPosturesListRevisionsArgs;
use crate::providers::gcp::clients::securityposture::SecuritypostureOrganizationsLocationsPosturesPatchArgs;
use crate::providers::gcp::clients::securityposture::SecuritypostureOrganizationsLocationsReportsCreateIaCValidationReportArgs;
use crate::providers::gcp::clients::securityposture::SecuritypostureOrganizationsLocationsReportsGetArgs;
use crate::providers::gcp::clients::securityposture::SecuritypostureOrganizationsLocationsReportsListArgs;
use crate::providers::gcp::clients::securityposture::SecuritypostureProjectsLocationsGetArgs;
use crate::providers::gcp::clients::securityposture::SecuritypostureProjectsLocationsListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// SecuritypostureProvider with automatic state tracking.
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
/// let provider = SecuritypostureProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct SecuritypostureProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> SecuritypostureProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new SecuritypostureProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new SecuritypostureProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
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

    /// Securityposture organizations locations operations get.
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
    pub fn securityposture_organizations_locations_operations_get(
        &self,
        args: &SecuritypostureOrganizationsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securityposture_organizations_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securityposture_organizations_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securityposture organizations locations operations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListOperationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn securityposture_organizations_locations_operations_list(
        &self,
        args: &SecuritypostureOrganizationsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securityposture_organizations_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = securityposture_organizations_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Securityposture organizations locations posture deployments get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PostureDeployment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn securityposture_organizations_locations_posture_deployments_get(
        &self,
        args: &SecuritypostureOrganizationsLocationsPostureDeploymentsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PostureDeployment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securityposture_organizations_locations_posture_deployments_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securityposture_organizations_locations_posture_deployments_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securityposture organizations locations posture deployments list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListPostureDeploymentsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn securityposture_organizations_locations_posture_deployments_list(
        &self,
        args: &SecuritypostureOrganizationsLocationsPostureDeploymentsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListPostureDeploymentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securityposture_organizations_locations_posture_deployments_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = securityposture_organizations_locations_posture_deployments_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Securityposture organizations locations posture templates get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PostureTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn securityposture_organizations_locations_posture_templates_get(
        &self,
        args: &SecuritypostureOrganizationsLocationsPostureTemplatesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PostureTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securityposture_organizations_locations_posture_templates_get_builder(
            &self.http_client,
            &args.name,
            &args.revisionId,
        )
        .map_err(ProviderError::Api)?;

        let task = securityposture_organizations_locations_posture_templates_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securityposture organizations locations posture templates list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListPostureTemplatesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn securityposture_organizations_locations_posture_templates_list(
        &self,
        args: &SecuritypostureOrganizationsLocationsPostureTemplatesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListPostureTemplatesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securityposture_organizations_locations_posture_templates_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = securityposture_organizations_locations_posture_templates_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Securityposture organizations locations postures get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Posture result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn securityposture_organizations_locations_postures_get(
        &self,
        args: &SecuritypostureOrganizationsLocationsPosturesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Posture, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securityposture_organizations_locations_postures_get_builder(
            &self.http_client,
            &args.name,
            &args.revisionId,
        )
        .map_err(ProviderError::Api)?;

        let task = securityposture_organizations_locations_postures_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securityposture organizations locations postures list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListPosturesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn securityposture_organizations_locations_postures_list(
        &self,
        args: &SecuritypostureOrganizationsLocationsPosturesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListPosturesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securityposture_organizations_locations_postures_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = securityposture_organizations_locations_postures_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securityposture organizations locations postures list revisions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListPostureRevisionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn securityposture_organizations_locations_postures_list_revisions(
        &self,
        args: &SecuritypostureOrganizationsLocationsPosturesListRevisionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListPostureRevisionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securityposture_organizations_locations_postures_list_revisions_builder(
            &self.http_client,
            &args.name,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = securityposture_organizations_locations_postures_list_revisions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Securityposture organizations locations reports get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Report result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn securityposture_organizations_locations_reports_get(
        &self,
        args: &SecuritypostureOrganizationsLocationsReportsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Report, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securityposture_organizations_locations_reports_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securityposture_organizations_locations_reports_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securityposture organizations locations reports list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListReportsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn securityposture_organizations_locations_reports_list(
        &self,
        args: &SecuritypostureOrganizationsLocationsReportsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListReportsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securityposture_organizations_locations_reports_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = securityposture_organizations_locations_reports_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securityposture projects locations get.
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
    pub fn securityposture_projects_locations_get(
        &self,
        args: &SecuritypostureProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securityposture_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = securityposture_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Securityposture projects locations list.
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
    pub fn securityposture_projects_locations_list(
        &self,
        args: &SecuritypostureProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = securityposture_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = securityposture_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
