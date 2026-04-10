//! SmartdevicemanagementProvider - State-aware smartdevicemanagement API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       smartdevicemanagement API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::smartdevicemanagement::{
    smartdevicemanagement_enterprises_devices_execute_command_builder, smartdevicemanagement_enterprises_devices_execute_command_task,
    smartdevicemanagement_enterprises_devices_get_builder, smartdevicemanagement_enterprises_devices_get_task,
    smartdevicemanagement_enterprises_devices_list_builder, smartdevicemanagement_enterprises_devices_list_task,
    smartdevicemanagement_enterprises_structures_get_builder, smartdevicemanagement_enterprises_structures_get_task,
    smartdevicemanagement_enterprises_structures_list_builder, smartdevicemanagement_enterprises_structures_list_task,
    smartdevicemanagement_enterprises_structures_rooms_get_builder, smartdevicemanagement_enterprises_structures_rooms_get_task,
    smartdevicemanagement_enterprises_structures_rooms_list_builder, smartdevicemanagement_enterprises_structures_rooms_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::smartdevicemanagement::GoogleHomeEnterpriseSdmV1Device;
use crate::providers::gcp::clients::smartdevicemanagement::GoogleHomeEnterpriseSdmV1ExecuteDeviceCommandResponse;
use crate::providers::gcp::clients::smartdevicemanagement::GoogleHomeEnterpriseSdmV1ListDevicesResponse;
use crate::providers::gcp::clients::smartdevicemanagement::GoogleHomeEnterpriseSdmV1ListRoomsResponse;
use crate::providers::gcp::clients::smartdevicemanagement::GoogleHomeEnterpriseSdmV1ListStructuresResponse;
use crate::providers::gcp::clients::smartdevicemanagement::GoogleHomeEnterpriseSdmV1Room;
use crate::providers::gcp::clients::smartdevicemanagement::GoogleHomeEnterpriseSdmV1Structure;
use crate::providers::gcp::clients::smartdevicemanagement::SmartdevicemanagementEnterprisesDevicesExecuteCommandArgs;
use crate::providers::gcp::clients::smartdevicemanagement::SmartdevicemanagementEnterprisesDevicesGetArgs;
use crate::providers::gcp::clients::smartdevicemanagement::SmartdevicemanagementEnterprisesDevicesListArgs;
use crate::providers::gcp::clients::smartdevicemanagement::SmartdevicemanagementEnterprisesStructuresGetArgs;
use crate::providers::gcp::clients::smartdevicemanagement::SmartdevicemanagementEnterprisesStructuresListArgs;
use crate::providers::gcp::clients::smartdevicemanagement::SmartdevicemanagementEnterprisesStructuresRoomsGetArgs;
use crate::providers::gcp::clients::smartdevicemanagement::SmartdevicemanagementEnterprisesStructuresRoomsListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// SmartdevicemanagementProvider with automatic state tracking.
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
/// let provider = SmartdevicemanagementProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct SmartdevicemanagementProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> SmartdevicemanagementProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new SmartdevicemanagementProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Smartdevicemanagement enterprises devices execute command.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleHomeEnterpriseSdmV1ExecuteDeviceCommandResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn smartdevicemanagement_enterprises_devices_execute_command(
        &self,
        args: &SmartdevicemanagementEnterprisesDevicesExecuteCommandArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleHomeEnterpriseSdmV1ExecuteDeviceCommandResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = smartdevicemanagement_enterprises_devices_execute_command_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = smartdevicemanagement_enterprises_devices_execute_command_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Smartdevicemanagement enterprises devices get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleHomeEnterpriseSdmV1Device result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn smartdevicemanagement_enterprises_devices_get(
        &self,
        args: &SmartdevicemanagementEnterprisesDevicesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleHomeEnterpriseSdmV1Device, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = smartdevicemanagement_enterprises_devices_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = smartdevicemanagement_enterprises_devices_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Smartdevicemanagement enterprises devices list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleHomeEnterpriseSdmV1ListDevicesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn smartdevicemanagement_enterprises_devices_list(
        &self,
        args: &SmartdevicemanagementEnterprisesDevicesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleHomeEnterpriseSdmV1ListDevicesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = smartdevicemanagement_enterprises_devices_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
        )
        .map_err(ProviderError::Api)?;

        let task = smartdevicemanagement_enterprises_devices_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Smartdevicemanagement enterprises structures get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleHomeEnterpriseSdmV1Structure result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn smartdevicemanagement_enterprises_structures_get(
        &self,
        args: &SmartdevicemanagementEnterprisesStructuresGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleHomeEnterpriseSdmV1Structure, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = smartdevicemanagement_enterprises_structures_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = smartdevicemanagement_enterprises_structures_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Smartdevicemanagement enterprises structures list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleHomeEnterpriseSdmV1ListStructuresResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn smartdevicemanagement_enterprises_structures_list(
        &self,
        args: &SmartdevicemanagementEnterprisesStructuresListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleHomeEnterpriseSdmV1ListStructuresResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = smartdevicemanagement_enterprises_structures_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
        )
        .map_err(ProviderError::Api)?;

        let task = smartdevicemanagement_enterprises_structures_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Smartdevicemanagement enterprises structures rooms get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleHomeEnterpriseSdmV1Room result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn smartdevicemanagement_enterprises_structures_rooms_get(
        &self,
        args: &SmartdevicemanagementEnterprisesStructuresRoomsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleHomeEnterpriseSdmV1Room, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = smartdevicemanagement_enterprises_structures_rooms_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = smartdevicemanagement_enterprises_structures_rooms_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Smartdevicemanagement enterprises structures rooms list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleHomeEnterpriseSdmV1ListRoomsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn smartdevicemanagement_enterprises_structures_rooms_list(
        &self,
        args: &SmartdevicemanagementEnterprisesStructuresRoomsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleHomeEnterpriseSdmV1ListRoomsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = smartdevicemanagement_enterprises_structures_rooms_list_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = smartdevicemanagement_enterprises_structures_rooms_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
