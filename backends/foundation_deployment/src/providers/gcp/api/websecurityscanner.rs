//! WebsecurityscannerProvider - State-aware websecurityscanner API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       websecurityscanner API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::websecurityscanner::{
    websecurityscanner_projects_scan_configs_create_builder, websecurityscanner_projects_scan_configs_create_task,
    websecurityscanner_projects_scan_configs_delete_builder, websecurityscanner_projects_scan_configs_delete_task,
    websecurityscanner_projects_scan_configs_patch_builder, websecurityscanner_projects_scan_configs_patch_task,
    websecurityscanner_projects_scan_configs_start_builder, websecurityscanner_projects_scan_configs_start_task,
    websecurityscanner_projects_scan_configs_scan_runs_stop_builder, websecurityscanner_projects_scan_configs_scan_runs_stop_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::websecurityscanner::Empty;
use crate::providers::gcp::clients::websecurityscanner::ScanConfig;
use crate::providers::gcp::clients::websecurityscanner::ScanRun;
use crate::providers::gcp::clients::websecurityscanner::WebsecurityscannerProjectsScanConfigsCreateArgs;
use crate::providers::gcp::clients::websecurityscanner::WebsecurityscannerProjectsScanConfigsDeleteArgs;
use crate::providers::gcp::clients::websecurityscanner::WebsecurityscannerProjectsScanConfigsPatchArgs;
use crate::providers::gcp::clients::websecurityscanner::WebsecurityscannerProjectsScanConfigsScanRunsStopArgs;
use crate::providers::gcp::clients::websecurityscanner::WebsecurityscannerProjectsScanConfigsStartArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// WebsecurityscannerProvider with automatic state tracking.
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
/// let provider = WebsecurityscannerProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct WebsecurityscannerProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> WebsecurityscannerProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new WebsecurityscannerProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Websecurityscanner projects scan configs create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ScanConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn websecurityscanner_projects_scan_configs_create(
        &self,
        args: &WebsecurityscannerProjectsScanConfigsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ScanConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = websecurityscanner_projects_scan_configs_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = websecurityscanner_projects_scan_configs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Websecurityscanner projects scan configs delete.
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
    pub fn websecurityscanner_projects_scan_configs_delete(
        &self,
        args: &WebsecurityscannerProjectsScanConfigsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = websecurityscanner_projects_scan_configs_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = websecurityscanner_projects_scan_configs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Websecurityscanner projects scan configs patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ScanConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn websecurityscanner_projects_scan_configs_patch(
        &self,
        args: &WebsecurityscannerProjectsScanConfigsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ScanConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = websecurityscanner_projects_scan_configs_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = websecurityscanner_projects_scan_configs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Websecurityscanner projects scan configs start.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ScanRun result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn websecurityscanner_projects_scan_configs_start(
        &self,
        args: &WebsecurityscannerProjectsScanConfigsStartArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ScanRun, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = websecurityscanner_projects_scan_configs_start_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = websecurityscanner_projects_scan_configs_start_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Websecurityscanner projects scan configs scan runs stop.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ScanRun result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn websecurityscanner_projects_scan_configs_scan_runs_stop(
        &self,
        args: &WebsecurityscannerProjectsScanConfigsScanRunsStopArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ScanRun, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = websecurityscanner_projects_scan_configs_scan_runs_stop_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = websecurityscanner_projects_scan_configs_scan_runs_stop_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
