//! LicensingProvider - State-aware licensing API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       licensing API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::licensing::{
    licensing_license_assignments_delete_builder, licensing_license_assignments_delete_task,
    licensing_license_assignments_insert_builder, licensing_license_assignments_insert_task,
    licensing_license_assignments_patch_builder, licensing_license_assignments_patch_task,
    licensing_license_assignments_update_builder, licensing_license_assignments_update_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::licensing::Empty;
use crate::providers::gcp::clients::licensing::LicenseAssignment;
use crate::providers::gcp::clients::licensing::LicensingLicenseAssignmentsDeleteArgs;
use crate::providers::gcp::clients::licensing::LicensingLicenseAssignmentsInsertArgs;
use crate::providers::gcp::clients::licensing::LicensingLicenseAssignmentsPatchArgs;
use crate::providers::gcp::clients::licensing::LicensingLicenseAssignmentsUpdateArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// LicensingProvider with automatic state tracking.
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
/// let provider = LicensingProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct LicensingProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> LicensingProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new LicensingProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Licensing license assignments delete.
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
    pub fn licensing_license_assignments_delete(
        &self,
        args: &LicensingLicenseAssignmentsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = licensing_license_assignments_delete_builder(
            &self.http_client,
            &args.productId,
            &args.skuId,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = licensing_license_assignments_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Licensing license assignments insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LicenseAssignment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn licensing_license_assignments_insert(
        &self,
        args: &LicensingLicenseAssignmentsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LicenseAssignment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = licensing_license_assignments_insert_builder(
            &self.http_client,
            &args.productId,
            &args.skuId,
        )
        .map_err(ProviderError::Api)?;

        let task = licensing_license_assignments_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Licensing license assignments patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LicenseAssignment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn licensing_license_assignments_patch(
        &self,
        args: &LicensingLicenseAssignmentsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LicenseAssignment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = licensing_license_assignments_patch_builder(
            &self.http_client,
            &args.productId,
            &args.skuId,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = licensing_license_assignments_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Licensing license assignments update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LicenseAssignment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn licensing_license_assignments_update(
        &self,
        args: &LicensingLicenseAssignmentsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LicenseAssignment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = licensing_license_assignments_update_builder(
            &self.http_client,
            &args.productId,
            &args.skuId,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = licensing_license_assignments_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
