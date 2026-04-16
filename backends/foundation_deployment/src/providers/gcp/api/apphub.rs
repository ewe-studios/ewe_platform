//! ApphubProvider - State-aware apphub API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       apphub API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::apphub::{
    apphub_projects_locations_detach_service_project_attachment_builder, apphub_projects_locations_detach_service_project_attachment_task,
    apphub_projects_locations_get_builder, apphub_projects_locations_get_task,
    apphub_projects_locations_get_boundary_builder, apphub_projects_locations_get_boundary_task,
    apphub_projects_locations_list_builder, apphub_projects_locations_list_task,
    apphub_projects_locations_lookup_service_project_attachment_builder, apphub_projects_locations_lookup_service_project_attachment_task,
    apphub_projects_locations_update_boundary_builder, apphub_projects_locations_update_boundary_task,
    apphub_projects_locations_applications_create_builder, apphub_projects_locations_applications_create_task,
    apphub_projects_locations_applications_delete_builder, apphub_projects_locations_applications_delete_task,
    apphub_projects_locations_applications_get_builder, apphub_projects_locations_applications_get_task,
    apphub_projects_locations_applications_get_iam_policy_builder, apphub_projects_locations_applications_get_iam_policy_task,
    apphub_projects_locations_applications_list_builder, apphub_projects_locations_applications_list_task,
    apphub_projects_locations_applications_patch_builder, apphub_projects_locations_applications_patch_task,
    apphub_projects_locations_applications_set_iam_policy_builder, apphub_projects_locations_applications_set_iam_policy_task,
    apphub_projects_locations_applications_test_iam_permissions_builder, apphub_projects_locations_applications_test_iam_permissions_task,
    apphub_projects_locations_applications_services_create_builder, apphub_projects_locations_applications_services_create_task,
    apphub_projects_locations_applications_services_delete_builder, apphub_projects_locations_applications_services_delete_task,
    apphub_projects_locations_applications_services_get_builder, apphub_projects_locations_applications_services_get_task,
    apphub_projects_locations_applications_services_list_builder, apphub_projects_locations_applications_services_list_task,
    apphub_projects_locations_applications_services_patch_builder, apphub_projects_locations_applications_services_patch_task,
    apphub_projects_locations_applications_workloads_create_builder, apphub_projects_locations_applications_workloads_create_task,
    apphub_projects_locations_applications_workloads_delete_builder, apphub_projects_locations_applications_workloads_delete_task,
    apphub_projects_locations_applications_workloads_get_builder, apphub_projects_locations_applications_workloads_get_task,
    apphub_projects_locations_applications_workloads_list_builder, apphub_projects_locations_applications_workloads_list_task,
    apphub_projects_locations_applications_workloads_patch_builder, apphub_projects_locations_applications_workloads_patch_task,
    apphub_projects_locations_discovered_services_get_builder, apphub_projects_locations_discovered_services_get_task,
    apphub_projects_locations_discovered_services_list_builder, apphub_projects_locations_discovered_services_list_task,
    apphub_projects_locations_discovered_services_lookup_builder, apphub_projects_locations_discovered_services_lookup_task,
    apphub_projects_locations_discovered_workloads_get_builder, apphub_projects_locations_discovered_workloads_get_task,
    apphub_projects_locations_discovered_workloads_list_builder, apphub_projects_locations_discovered_workloads_list_task,
    apphub_projects_locations_discovered_workloads_lookup_builder, apphub_projects_locations_discovered_workloads_lookup_task,
    apphub_projects_locations_extended_metadata_schemas_get_builder, apphub_projects_locations_extended_metadata_schemas_get_task,
    apphub_projects_locations_extended_metadata_schemas_list_builder, apphub_projects_locations_extended_metadata_schemas_list_task,
    apphub_projects_locations_operations_cancel_builder, apphub_projects_locations_operations_cancel_task,
    apphub_projects_locations_operations_delete_builder, apphub_projects_locations_operations_delete_task,
    apphub_projects_locations_operations_get_builder, apphub_projects_locations_operations_get_task,
    apphub_projects_locations_operations_list_builder, apphub_projects_locations_operations_list_task,
    apphub_projects_locations_service_project_attachments_create_builder, apphub_projects_locations_service_project_attachments_create_task,
    apphub_projects_locations_service_project_attachments_delete_builder, apphub_projects_locations_service_project_attachments_delete_task,
    apphub_projects_locations_service_project_attachments_get_builder, apphub_projects_locations_service_project_attachments_get_task,
    apphub_projects_locations_service_project_attachments_list_builder, apphub_projects_locations_service_project_attachments_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::apphub::Application;
use crate::providers::gcp::clients::apphub::Boundary;
use crate::providers::gcp::clients::apphub::DetachServiceProjectAttachmentResponse;
use crate::providers::gcp::clients::apphub::DiscoveredService;
use crate::providers::gcp::clients::apphub::DiscoveredWorkload;
use crate::providers::gcp::clients::apphub::Empty;
use crate::providers::gcp::clients::apphub::ExtendedMetadataSchema;
use crate::providers::gcp::clients::apphub::ListApplicationsResponse;
use crate::providers::gcp::clients::apphub::ListDiscoveredServicesResponse;
use crate::providers::gcp::clients::apphub::ListDiscoveredWorkloadsResponse;
use crate::providers::gcp::clients::apphub::ListExtendedMetadataSchemasResponse;
use crate::providers::gcp::clients::apphub::ListLocationsResponse;
use crate::providers::gcp::clients::apphub::ListOperationsResponse;
use crate::providers::gcp::clients::apphub::ListServiceProjectAttachmentsResponse;
use crate::providers::gcp::clients::apphub::ListServicesResponse;
use crate::providers::gcp::clients::apphub::ListWorkloadsResponse;
use crate::providers::gcp::clients::apphub::Location;
use crate::providers::gcp::clients::apphub::LookupDiscoveredServiceResponse;
use crate::providers::gcp::clients::apphub::LookupDiscoveredWorkloadResponse;
use crate::providers::gcp::clients::apphub::LookupServiceProjectAttachmentResponse;
use crate::providers::gcp::clients::apphub::Operation;
use crate::providers::gcp::clients::apphub::Policy;
use crate::providers::gcp::clients::apphub::Service;
use crate::providers::gcp::clients::apphub::ServiceProjectAttachment;
use crate::providers::gcp::clients::apphub::TestIamPermissionsResponse;
use crate::providers::gcp::clients::apphub::Workload;
use crate::providers::gcp::clients::apphub::ApphubProjectsLocationsApplicationsCreateArgs;
use crate::providers::gcp::clients::apphub::ApphubProjectsLocationsApplicationsDeleteArgs;
use crate::providers::gcp::clients::apphub::ApphubProjectsLocationsApplicationsGetArgs;
use crate::providers::gcp::clients::apphub::ApphubProjectsLocationsApplicationsGetIamPolicyArgs;
use crate::providers::gcp::clients::apphub::ApphubProjectsLocationsApplicationsListArgs;
use crate::providers::gcp::clients::apphub::ApphubProjectsLocationsApplicationsPatchArgs;
use crate::providers::gcp::clients::apphub::ApphubProjectsLocationsApplicationsServicesCreateArgs;
use crate::providers::gcp::clients::apphub::ApphubProjectsLocationsApplicationsServicesDeleteArgs;
use crate::providers::gcp::clients::apphub::ApphubProjectsLocationsApplicationsServicesGetArgs;
use crate::providers::gcp::clients::apphub::ApphubProjectsLocationsApplicationsServicesListArgs;
use crate::providers::gcp::clients::apphub::ApphubProjectsLocationsApplicationsServicesPatchArgs;
use crate::providers::gcp::clients::apphub::ApphubProjectsLocationsApplicationsSetIamPolicyArgs;
use crate::providers::gcp::clients::apphub::ApphubProjectsLocationsApplicationsTestIamPermissionsArgs;
use crate::providers::gcp::clients::apphub::ApphubProjectsLocationsApplicationsWorkloadsCreateArgs;
use crate::providers::gcp::clients::apphub::ApphubProjectsLocationsApplicationsWorkloadsDeleteArgs;
use crate::providers::gcp::clients::apphub::ApphubProjectsLocationsApplicationsWorkloadsGetArgs;
use crate::providers::gcp::clients::apphub::ApphubProjectsLocationsApplicationsWorkloadsListArgs;
use crate::providers::gcp::clients::apphub::ApphubProjectsLocationsApplicationsWorkloadsPatchArgs;
use crate::providers::gcp::clients::apphub::ApphubProjectsLocationsDetachServiceProjectAttachmentArgs;
use crate::providers::gcp::clients::apphub::ApphubProjectsLocationsDiscoveredServicesGetArgs;
use crate::providers::gcp::clients::apphub::ApphubProjectsLocationsDiscoveredServicesListArgs;
use crate::providers::gcp::clients::apphub::ApphubProjectsLocationsDiscoveredServicesLookupArgs;
use crate::providers::gcp::clients::apphub::ApphubProjectsLocationsDiscoveredWorkloadsGetArgs;
use crate::providers::gcp::clients::apphub::ApphubProjectsLocationsDiscoveredWorkloadsListArgs;
use crate::providers::gcp::clients::apphub::ApphubProjectsLocationsDiscoveredWorkloadsLookupArgs;
use crate::providers::gcp::clients::apphub::ApphubProjectsLocationsExtendedMetadataSchemasGetArgs;
use crate::providers::gcp::clients::apphub::ApphubProjectsLocationsExtendedMetadataSchemasListArgs;
use crate::providers::gcp::clients::apphub::ApphubProjectsLocationsGetArgs;
use crate::providers::gcp::clients::apphub::ApphubProjectsLocationsGetBoundaryArgs;
use crate::providers::gcp::clients::apphub::ApphubProjectsLocationsListArgs;
use crate::providers::gcp::clients::apphub::ApphubProjectsLocationsLookupServiceProjectAttachmentArgs;
use crate::providers::gcp::clients::apphub::ApphubProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::apphub::ApphubProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::apphub::ApphubProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::apphub::ApphubProjectsLocationsOperationsListArgs;
use crate::providers::gcp::clients::apphub::ApphubProjectsLocationsServiceProjectAttachmentsCreateArgs;
use crate::providers::gcp::clients::apphub::ApphubProjectsLocationsServiceProjectAttachmentsDeleteArgs;
use crate::providers::gcp::clients::apphub::ApphubProjectsLocationsServiceProjectAttachmentsGetArgs;
use crate::providers::gcp::clients::apphub::ApphubProjectsLocationsServiceProjectAttachmentsListArgs;
use crate::providers::gcp::clients::apphub::ApphubProjectsLocationsUpdateBoundaryArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// ApphubProvider with automatic state tracking.
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
/// let provider = ApphubProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct ApphubProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> ApphubProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new ApphubProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new ApphubProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Apphub projects locations detach service project attachment.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DetachServiceProjectAttachmentResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apphub_projects_locations_detach_service_project_attachment(
        &self,
        args: &ApphubProjectsLocationsDetachServiceProjectAttachmentArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DetachServiceProjectAttachmentResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apphub_projects_locations_detach_service_project_attachment_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apphub_projects_locations_detach_service_project_attachment_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apphub projects locations get.
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
    pub fn apphub_projects_locations_get(
        &self,
        args: &ApphubProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apphub_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apphub_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apphub projects locations get boundary.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Boundary result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apphub_projects_locations_get_boundary(
        &self,
        args: &ApphubProjectsLocationsGetBoundaryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Boundary, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apphub_projects_locations_get_boundary_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apphub_projects_locations_get_boundary_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apphub projects locations list.
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
    pub fn apphub_projects_locations_list(
        &self,
        args: &ApphubProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apphub_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = apphub_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apphub projects locations lookup service project attachment.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LookupServiceProjectAttachmentResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apphub_projects_locations_lookup_service_project_attachment(
        &self,
        args: &ApphubProjectsLocationsLookupServiceProjectAttachmentArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LookupServiceProjectAttachmentResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apphub_projects_locations_lookup_service_project_attachment_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apphub_projects_locations_lookup_service_project_attachment_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apphub projects locations update boundary.
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
    pub fn apphub_projects_locations_update_boundary(
        &self,
        args: &ApphubProjectsLocationsUpdateBoundaryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apphub_projects_locations_update_boundary_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = apphub_projects_locations_update_boundary_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apphub projects locations applications create.
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
    pub fn apphub_projects_locations_applications_create(
        &self,
        args: &ApphubProjectsLocationsApplicationsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apphub_projects_locations_applications_create_builder(
            &self.http_client,
            &args.parent,
            &args.applicationId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = apphub_projects_locations_applications_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apphub projects locations applications delete.
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
    pub fn apphub_projects_locations_applications_delete(
        &self,
        args: &ApphubProjectsLocationsApplicationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apphub_projects_locations_applications_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = apphub_projects_locations_applications_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apphub projects locations applications get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Application result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apphub_projects_locations_applications_get(
        &self,
        args: &ApphubProjectsLocationsApplicationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Application, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apphub_projects_locations_applications_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apphub_projects_locations_applications_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apphub projects locations applications get iam policy.
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
    pub fn apphub_projects_locations_applications_get_iam_policy(
        &self,
        args: &ApphubProjectsLocationsApplicationsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apphub_projects_locations_applications_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options_requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = apphub_projects_locations_applications_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apphub projects locations applications list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListApplicationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apphub_projects_locations_applications_list(
        &self,
        args: &ApphubProjectsLocationsApplicationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListApplicationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apphub_projects_locations_applications_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = apphub_projects_locations_applications_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apphub projects locations applications patch.
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
    pub fn apphub_projects_locations_applications_patch(
        &self,
        args: &ApphubProjectsLocationsApplicationsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apphub_projects_locations_applications_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = apphub_projects_locations_applications_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apphub projects locations applications set iam policy.
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
    pub fn apphub_projects_locations_applications_set_iam_policy(
        &self,
        args: &ApphubProjectsLocationsApplicationsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apphub_projects_locations_applications_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apphub_projects_locations_applications_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apphub projects locations applications test iam permissions.
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
    pub fn apphub_projects_locations_applications_test_iam_permissions(
        &self,
        args: &ApphubProjectsLocationsApplicationsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apphub_projects_locations_applications_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apphub_projects_locations_applications_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apphub projects locations applications services create.
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
    pub fn apphub_projects_locations_applications_services_create(
        &self,
        args: &ApphubProjectsLocationsApplicationsServicesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apphub_projects_locations_applications_services_create_builder(
            &self.http_client,
            &args.parent,
            &args.requestId,
            &args.serviceId,
        )
        .map_err(ProviderError::Api)?;

        let task = apphub_projects_locations_applications_services_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apphub projects locations applications services delete.
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
    pub fn apphub_projects_locations_applications_services_delete(
        &self,
        args: &ApphubProjectsLocationsApplicationsServicesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apphub_projects_locations_applications_services_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = apphub_projects_locations_applications_services_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apphub projects locations applications services get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Service result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apphub_projects_locations_applications_services_get(
        &self,
        args: &ApphubProjectsLocationsApplicationsServicesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Service, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apphub_projects_locations_applications_services_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apphub_projects_locations_applications_services_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apphub projects locations applications services list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListServicesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apphub_projects_locations_applications_services_list(
        &self,
        args: &ApphubProjectsLocationsApplicationsServicesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListServicesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apphub_projects_locations_applications_services_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = apphub_projects_locations_applications_services_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apphub projects locations applications services patch.
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
    pub fn apphub_projects_locations_applications_services_patch(
        &self,
        args: &ApphubProjectsLocationsApplicationsServicesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apphub_projects_locations_applications_services_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = apphub_projects_locations_applications_services_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apphub projects locations applications workloads create.
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
    pub fn apphub_projects_locations_applications_workloads_create(
        &self,
        args: &ApphubProjectsLocationsApplicationsWorkloadsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apphub_projects_locations_applications_workloads_create_builder(
            &self.http_client,
            &args.parent,
            &args.requestId,
            &args.workloadId,
        )
        .map_err(ProviderError::Api)?;

        let task = apphub_projects_locations_applications_workloads_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apphub projects locations applications workloads delete.
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
    pub fn apphub_projects_locations_applications_workloads_delete(
        &self,
        args: &ApphubProjectsLocationsApplicationsWorkloadsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apphub_projects_locations_applications_workloads_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = apphub_projects_locations_applications_workloads_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apphub projects locations applications workloads get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Workload result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apphub_projects_locations_applications_workloads_get(
        &self,
        args: &ApphubProjectsLocationsApplicationsWorkloadsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Workload, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apphub_projects_locations_applications_workloads_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apphub_projects_locations_applications_workloads_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apphub projects locations applications workloads list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListWorkloadsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apphub_projects_locations_applications_workloads_list(
        &self,
        args: &ApphubProjectsLocationsApplicationsWorkloadsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListWorkloadsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apphub_projects_locations_applications_workloads_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = apphub_projects_locations_applications_workloads_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apphub projects locations applications workloads patch.
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
    pub fn apphub_projects_locations_applications_workloads_patch(
        &self,
        args: &ApphubProjectsLocationsApplicationsWorkloadsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apphub_projects_locations_applications_workloads_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = apphub_projects_locations_applications_workloads_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apphub projects locations discovered services get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DiscoveredService result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apphub_projects_locations_discovered_services_get(
        &self,
        args: &ApphubProjectsLocationsDiscoveredServicesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DiscoveredService, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apphub_projects_locations_discovered_services_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apphub_projects_locations_discovered_services_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apphub projects locations discovered services list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDiscoveredServicesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apphub_projects_locations_discovered_services_list(
        &self,
        args: &ApphubProjectsLocationsDiscoveredServicesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDiscoveredServicesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apphub_projects_locations_discovered_services_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = apphub_projects_locations_discovered_services_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apphub projects locations discovered services lookup.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LookupDiscoveredServiceResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apphub_projects_locations_discovered_services_lookup(
        &self,
        args: &ApphubProjectsLocationsDiscoveredServicesLookupArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LookupDiscoveredServiceResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apphub_projects_locations_discovered_services_lookup_builder(
            &self.http_client,
            &args.parent,
            &args.uri,
        )
        .map_err(ProviderError::Api)?;

        let task = apphub_projects_locations_discovered_services_lookup_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apphub projects locations discovered workloads get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DiscoveredWorkload result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apphub_projects_locations_discovered_workloads_get(
        &self,
        args: &ApphubProjectsLocationsDiscoveredWorkloadsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DiscoveredWorkload, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apphub_projects_locations_discovered_workloads_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apphub_projects_locations_discovered_workloads_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apphub projects locations discovered workloads list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDiscoveredWorkloadsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apphub_projects_locations_discovered_workloads_list(
        &self,
        args: &ApphubProjectsLocationsDiscoveredWorkloadsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDiscoveredWorkloadsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apphub_projects_locations_discovered_workloads_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = apphub_projects_locations_discovered_workloads_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apphub projects locations discovered workloads lookup.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LookupDiscoveredWorkloadResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apphub_projects_locations_discovered_workloads_lookup(
        &self,
        args: &ApphubProjectsLocationsDiscoveredWorkloadsLookupArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LookupDiscoveredWorkloadResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apphub_projects_locations_discovered_workloads_lookup_builder(
            &self.http_client,
            &args.parent,
            &args.uri,
        )
        .map_err(ProviderError::Api)?;

        let task = apphub_projects_locations_discovered_workloads_lookup_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apphub projects locations extended metadata schemas get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ExtendedMetadataSchema result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apphub_projects_locations_extended_metadata_schemas_get(
        &self,
        args: &ApphubProjectsLocationsExtendedMetadataSchemasGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ExtendedMetadataSchema, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apphub_projects_locations_extended_metadata_schemas_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apphub_projects_locations_extended_metadata_schemas_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apphub projects locations extended metadata schemas list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListExtendedMetadataSchemasResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apphub_projects_locations_extended_metadata_schemas_list(
        &self,
        args: &ApphubProjectsLocationsExtendedMetadataSchemasListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListExtendedMetadataSchemasResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apphub_projects_locations_extended_metadata_schemas_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = apphub_projects_locations_extended_metadata_schemas_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apphub projects locations operations cancel.
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
    pub fn apphub_projects_locations_operations_cancel(
        &self,
        args: &ApphubProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apphub_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apphub_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apphub projects locations operations delete.
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
    pub fn apphub_projects_locations_operations_delete(
        &self,
        args: &ApphubProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apphub_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apphub_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apphub projects locations operations get.
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
    pub fn apphub_projects_locations_operations_get(
        &self,
        args: &ApphubProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apphub_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apphub_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apphub projects locations operations list.
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
    pub fn apphub_projects_locations_operations_list(
        &self,
        args: &ApphubProjectsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apphub_projects_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = apphub_projects_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apphub projects locations service project attachments create.
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
    pub fn apphub_projects_locations_service_project_attachments_create(
        &self,
        args: &ApphubProjectsLocationsServiceProjectAttachmentsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apphub_projects_locations_service_project_attachments_create_builder(
            &self.http_client,
            &args.parent,
            &args.requestId,
            &args.serviceProjectAttachmentId,
        )
        .map_err(ProviderError::Api)?;

        let task = apphub_projects_locations_service_project_attachments_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apphub projects locations service project attachments delete.
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
    pub fn apphub_projects_locations_service_project_attachments_delete(
        &self,
        args: &ApphubProjectsLocationsServiceProjectAttachmentsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apphub_projects_locations_service_project_attachments_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = apphub_projects_locations_service_project_attachments_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apphub projects locations service project attachments get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ServiceProjectAttachment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apphub_projects_locations_service_project_attachments_get(
        &self,
        args: &ApphubProjectsLocationsServiceProjectAttachmentsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ServiceProjectAttachment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apphub_projects_locations_service_project_attachments_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apphub_projects_locations_service_project_attachments_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apphub projects locations service project attachments list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListServiceProjectAttachmentsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apphub_projects_locations_service_project_attachments_list(
        &self,
        args: &ApphubProjectsLocationsServiceProjectAttachmentsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListServiceProjectAttachmentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apphub_projects_locations_service_project_attachments_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = apphub_projects_locations_service_project_attachments_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
