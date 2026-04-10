//! ContactcenteraiplatformProvider - State-aware contactcenteraiplatform API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       contactcenteraiplatform API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::contactcenteraiplatform::{
    contactcenteraiplatform_projects_locations_generate_shifts_builder, contactcenteraiplatform_projects_locations_generate_shifts_task,
    contactcenteraiplatform_projects_locations_get_builder, contactcenteraiplatform_projects_locations_get_task,
    contactcenteraiplatform_projects_locations_list_builder, contactcenteraiplatform_projects_locations_list_task,
    contactcenteraiplatform_projects_locations_query_contact_center_quota_builder, contactcenteraiplatform_projects_locations_query_contact_center_quota_task,
    contactcenteraiplatform_projects_locations_contact_centers_create_builder, contactcenteraiplatform_projects_locations_contact_centers_create_task,
    contactcenteraiplatform_projects_locations_contact_centers_delete_builder, contactcenteraiplatform_projects_locations_contact_centers_delete_task,
    contactcenteraiplatform_projects_locations_contact_centers_get_builder, contactcenteraiplatform_projects_locations_contact_centers_get_task,
    contactcenteraiplatform_projects_locations_contact_centers_list_builder, contactcenteraiplatform_projects_locations_contact_centers_list_task,
    contactcenteraiplatform_projects_locations_contact_centers_patch_builder, contactcenteraiplatform_projects_locations_contact_centers_patch_task,
    contactcenteraiplatform_projects_locations_operations_cancel_builder, contactcenteraiplatform_projects_locations_operations_cancel_task,
    contactcenteraiplatform_projects_locations_operations_delete_builder, contactcenteraiplatform_projects_locations_operations_delete_task,
    contactcenteraiplatform_projects_locations_operations_get_builder, contactcenteraiplatform_projects_locations_operations_get_task,
    contactcenteraiplatform_projects_locations_operations_list_builder, contactcenteraiplatform_projects_locations_operations_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::contactcenteraiplatform::ContactCenter;
use crate::providers::gcp::clients::contactcenteraiplatform::ContactCenterQuota;
use crate::providers::gcp::clients::contactcenteraiplatform::Empty;
use crate::providers::gcp::clients::contactcenteraiplatform::ListContactCentersResponse;
use crate::providers::gcp::clients::contactcenteraiplatform::ListLocationsResponse;
use crate::providers::gcp::clients::contactcenteraiplatform::ListOperationsResponse;
use crate::providers::gcp::clients::contactcenteraiplatform::Location;
use crate::providers::gcp::clients::contactcenteraiplatform::Operation;
use crate::providers::gcp::clients::contactcenteraiplatform::ContactcenteraiplatformProjectsLocationsContactCentersCreateArgs;
use crate::providers::gcp::clients::contactcenteraiplatform::ContactcenteraiplatformProjectsLocationsContactCentersDeleteArgs;
use crate::providers::gcp::clients::contactcenteraiplatform::ContactcenteraiplatformProjectsLocationsContactCentersGetArgs;
use crate::providers::gcp::clients::contactcenteraiplatform::ContactcenteraiplatformProjectsLocationsContactCentersListArgs;
use crate::providers::gcp::clients::contactcenteraiplatform::ContactcenteraiplatformProjectsLocationsContactCentersPatchArgs;
use crate::providers::gcp::clients::contactcenteraiplatform::ContactcenteraiplatformProjectsLocationsGenerateShiftsArgs;
use crate::providers::gcp::clients::contactcenteraiplatform::ContactcenteraiplatformProjectsLocationsGetArgs;
use crate::providers::gcp::clients::contactcenteraiplatform::ContactcenteraiplatformProjectsLocationsListArgs;
use crate::providers::gcp::clients::contactcenteraiplatform::ContactcenteraiplatformProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::contactcenteraiplatform::ContactcenteraiplatformProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::contactcenteraiplatform::ContactcenteraiplatformProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::contactcenteraiplatform::ContactcenteraiplatformProjectsLocationsOperationsListArgs;
use crate::providers::gcp::clients::contactcenteraiplatform::ContactcenteraiplatformProjectsLocationsQueryContactCenterQuotaArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// ContactcenteraiplatformProvider with automatic state tracking.
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
/// let provider = ContactcenteraiplatformProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct ContactcenteraiplatformProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> ContactcenteraiplatformProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new ContactcenteraiplatformProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Contactcenteraiplatform projects locations generate shifts.
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
    pub fn contactcenteraiplatform_projects_locations_generate_shifts(
        &self,
        args: &ContactcenteraiplatformProjectsLocationsGenerateShiftsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contactcenteraiplatform_projects_locations_generate_shifts_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = contactcenteraiplatform_projects_locations_generate_shifts_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Contactcenteraiplatform projects locations get.
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
    pub fn contactcenteraiplatform_projects_locations_get(
        &self,
        args: &ContactcenteraiplatformProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contactcenteraiplatform_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = contactcenteraiplatform_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Contactcenteraiplatform projects locations list.
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
    pub fn contactcenteraiplatform_projects_locations_list(
        &self,
        args: &ContactcenteraiplatformProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contactcenteraiplatform_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = contactcenteraiplatform_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Contactcenteraiplatform projects locations query contact center quota.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ContactCenterQuota result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn contactcenteraiplatform_projects_locations_query_contact_center_quota(
        &self,
        args: &ContactcenteraiplatformProjectsLocationsQueryContactCenterQuotaArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ContactCenterQuota, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contactcenteraiplatform_projects_locations_query_contact_center_quota_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = contactcenteraiplatform_projects_locations_query_contact_center_quota_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Contactcenteraiplatform projects locations contact centers create.
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
    pub fn contactcenteraiplatform_projects_locations_contact_centers_create(
        &self,
        args: &ContactcenteraiplatformProjectsLocationsContactCentersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contactcenteraiplatform_projects_locations_contact_centers_create_builder(
            &self.http_client,
            &args.parent,
            &args.contactCenterId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = contactcenteraiplatform_projects_locations_contact_centers_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Contactcenteraiplatform projects locations contact centers delete.
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
    pub fn contactcenteraiplatform_projects_locations_contact_centers_delete(
        &self,
        args: &ContactcenteraiplatformProjectsLocationsContactCentersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contactcenteraiplatform_projects_locations_contact_centers_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = contactcenteraiplatform_projects_locations_contact_centers_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Contactcenteraiplatform projects locations contact centers get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ContactCenter result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn contactcenteraiplatform_projects_locations_contact_centers_get(
        &self,
        args: &ContactcenteraiplatformProjectsLocationsContactCentersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ContactCenter, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contactcenteraiplatform_projects_locations_contact_centers_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = contactcenteraiplatform_projects_locations_contact_centers_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Contactcenteraiplatform projects locations contact centers list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListContactCentersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn contactcenteraiplatform_projects_locations_contact_centers_list(
        &self,
        args: &ContactcenteraiplatformProjectsLocationsContactCentersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListContactCentersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contactcenteraiplatform_projects_locations_contact_centers_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = contactcenteraiplatform_projects_locations_contact_centers_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Contactcenteraiplatform projects locations contact centers patch.
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
    pub fn contactcenteraiplatform_projects_locations_contact_centers_patch(
        &self,
        args: &ContactcenteraiplatformProjectsLocationsContactCentersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contactcenteraiplatform_projects_locations_contact_centers_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = contactcenteraiplatform_projects_locations_contact_centers_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Contactcenteraiplatform projects locations operations cancel.
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
    pub fn contactcenteraiplatform_projects_locations_operations_cancel(
        &self,
        args: &ContactcenteraiplatformProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contactcenteraiplatform_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = contactcenteraiplatform_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Contactcenteraiplatform projects locations operations delete.
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
    pub fn contactcenteraiplatform_projects_locations_operations_delete(
        &self,
        args: &ContactcenteraiplatformProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contactcenteraiplatform_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = contactcenteraiplatform_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Contactcenteraiplatform projects locations operations get.
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
    pub fn contactcenteraiplatform_projects_locations_operations_get(
        &self,
        args: &ContactcenteraiplatformProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contactcenteraiplatform_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = contactcenteraiplatform_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Contactcenteraiplatform projects locations operations list.
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
    pub fn contactcenteraiplatform_projects_locations_operations_list(
        &self,
        args: &ContactcenteraiplatformProjectsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contactcenteraiplatform_projects_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = contactcenteraiplatform_projects_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
