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
    contactcenteraiplatform_projects_locations_contact_centers_create_builder, contactcenteraiplatform_projects_locations_contact_centers_create_task,
    contactcenteraiplatform_projects_locations_contact_centers_delete_builder, contactcenteraiplatform_projects_locations_contact_centers_delete_task,
    contactcenteraiplatform_projects_locations_contact_centers_patch_builder, contactcenteraiplatform_projects_locations_contact_centers_patch_task,
    contactcenteraiplatform_projects_locations_operations_cancel_builder, contactcenteraiplatform_projects_locations_operations_cancel_task,
    contactcenteraiplatform_projects_locations_operations_delete_builder, contactcenteraiplatform_projects_locations_operations_delete_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::contactcenteraiplatform::Empty;
use crate::providers::gcp::clients::contactcenteraiplatform::Operation;
use crate::providers::gcp::clients::contactcenteraiplatform::ContactcenteraiplatformProjectsLocationsContactCentersCreateArgs;
use crate::providers::gcp::clients::contactcenteraiplatform::ContactcenteraiplatformProjectsLocationsContactCentersDeleteArgs;
use crate::providers::gcp::clients::contactcenteraiplatform::ContactcenteraiplatformProjectsLocationsContactCentersPatchArgs;
use crate::providers::gcp::clients::contactcenteraiplatform::ContactcenteraiplatformProjectsLocationsGenerateShiftsArgs;
use crate::providers::gcp::clients::contactcenteraiplatform::ContactcenteraiplatformProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::contactcenteraiplatform::ContactcenteraiplatformProjectsLocationsOperationsDeleteArgs;
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

}
