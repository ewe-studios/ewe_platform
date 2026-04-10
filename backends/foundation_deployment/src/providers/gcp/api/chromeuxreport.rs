//! ChromeuxreportProvider - State-aware chromeuxreport API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       chromeuxreport API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::chromeuxreport::{
    chromeuxreport_records_query_history_record_builder, chromeuxreport_records_query_history_record_task,
    chromeuxreport_records_query_record_builder, chromeuxreport_records_query_record_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::chromeuxreport::QueryHistoryResponse;
use crate::providers::gcp::clients::chromeuxreport::QueryResponse;
use crate::providers::gcp::clients::chromeuxreport::ChromeuxreportRecordsQueryHistoryRecordArgs;
use crate::providers::gcp::clients::chromeuxreport::ChromeuxreportRecordsQueryRecordArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// ChromeuxreportProvider with automatic state tracking.
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
/// let provider = ChromeuxreportProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct ChromeuxreportProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> ChromeuxreportProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new ChromeuxreportProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Chromeuxreport records query history record.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the QueryHistoryResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chromeuxreport_records_query_history_record(
        &self,
        args: &ChromeuxreportRecordsQueryHistoryRecordArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<QueryHistoryResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromeuxreport_records_query_history_record_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = chromeuxreport_records_query_history_record_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromeuxreport records query record.
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
    pub fn chromeuxreport_records_query_record(
        &self,
        args: &ChromeuxreportRecordsQueryRecordArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<QueryResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromeuxreport_records_query_record_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = chromeuxreport_records_query_record_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
