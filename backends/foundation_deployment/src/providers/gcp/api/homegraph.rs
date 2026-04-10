//! HomegraphProvider - State-aware homegraph API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       homegraph API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::homegraph::{
    homegraph_agent_users_delete_builder, homegraph_agent_users_delete_task,
    homegraph_devices_query_builder, homegraph_devices_query_task,
    homegraph_devices_report_state_and_notification_builder, homegraph_devices_report_state_and_notification_task,
    homegraph_devices_request_sync_builder, homegraph_devices_request_sync_task,
    homegraph_devices_sync_builder, homegraph_devices_sync_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::homegraph::Empty;
use crate::providers::gcp::clients::homegraph::QueryResponse;
use crate::providers::gcp::clients::homegraph::ReportStateAndNotificationResponse;
use crate::providers::gcp::clients::homegraph::RequestSyncDevicesResponse;
use crate::providers::gcp::clients::homegraph::SyncResponse;
use crate::providers::gcp::clients::homegraph::HomegraphAgentUsersDeleteArgs;
use crate::providers::gcp::clients::homegraph::HomegraphDevicesQueryArgs;
use crate::providers::gcp::clients::homegraph::HomegraphDevicesReportStateAndNotificationArgs;
use crate::providers::gcp::clients::homegraph::HomegraphDevicesRequestSyncArgs;
use crate::providers::gcp::clients::homegraph::HomegraphDevicesSyncArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// HomegraphProvider with automatic state tracking.
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
/// let provider = HomegraphProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct HomegraphProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> HomegraphProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new HomegraphProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Homegraph agent users delete.
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
    pub fn homegraph_agent_users_delete(
        &self,
        args: &HomegraphAgentUsersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = homegraph_agent_users_delete_builder(
            &self.http_client,
            &args.agentUserId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = homegraph_agent_users_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Homegraph devices query.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the QueryResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn homegraph_devices_query(
        &self,
        args: &HomegraphDevicesQueryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<QueryResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = homegraph_devices_query_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = homegraph_devices_query_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Homegraph devices report state and notification.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ReportStateAndNotificationResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn homegraph_devices_report_state_and_notification(
        &self,
        args: &HomegraphDevicesReportStateAndNotificationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ReportStateAndNotificationResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = homegraph_devices_report_state_and_notification_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = homegraph_devices_report_state_and_notification_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Homegraph devices request sync.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RequestSyncDevicesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn homegraph_devices_request_sync(
        &self,
        args: &HomegraphDevicesRequestSyncArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RequestSyncDevicesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = homegraph_devices_request_sync_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = homegraph_devices_request_sync_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Homegraph devices sync.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SyncResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn homegraph_devices_sync(
        &self,
        args: &HomegraphDevicesSyncArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SyncResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = homegraph_devices_sync_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = homegraph_devices_sync_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
