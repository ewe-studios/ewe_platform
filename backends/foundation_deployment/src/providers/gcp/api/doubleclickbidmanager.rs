//! DoubleclickbidmanagerProvider - State-aware doubleclickbidmanager API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       doubleclickbidmanager API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::doubleclickbidmanager::{
    doubleclickbidmanager_queries_create_builder, doubleclickbidmanager_queries_create_task,
    doubleclickbidmanager_queries_delete_builder, doubleclickbidmanager_queries_delete_task,
    doubleclickbidmanager_queries_get_builder, doubleclickbidmanager_queries_get_task,
    doubleclickbidmanager_queries_list_builder, doubleclickbidmanager_queries_list_task,
    doubleclickbidmanager_queries_run_builder, doubleclickbidmanager_queries_run_task,
    doubleclickbidmanager_queries_reports_get_builder, doubleclickbidmanager_queries_reports_get_task,
    doubleclickbidmanager_queries_reports_list_builder, doubleclickbidmanager_queries_reports_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::doubleclickbidmanager::ListQueriesResponse;
use crate::providers::gcp::clients::doubleclickbidmanager::ListReportsResponse;
use crate::providers::gcp::clients::doubleclickbidmanager::Query;
use crate::providers::gcp::clients::doubleclickbidmanager::Report;
use crate::providers::gcp::clients::doubleclickbidmanager::DoubleclickbidmanagerQueriesDeleteArgs;
use crate::providers::gcp::clients::doubleclickbidmanager::DoubleclickbidmanagerQueriesGetArgs;
use crate::providers::gcp::clients::doubleclickbidmanager::DoubleclickbidmanagerQueriesListArgs;
use crate::providers::gcp::clients::doubleclickbidmanager::DoubleclickbidmanagerQueriesReportsGetArgs;
use crate::providers::gcp::clients::doubleclickbidmanager::DoubleclickbidmanagerQueriesReportsListArgs;
use crate::providers::gcp::clients::doubleclickbidmanager::DoubleclickbidmanagerQueriesRunArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// DoubleclickbidmanagerProvider with automatic state tracking.
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
/// let provider = DoubleclickbidmanagerProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct DoubleclickbidmanagerProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> DoubleclickbidmanagerProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new DoubleclickbidmanagerProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new DoubleclickbidmanagerProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Doubleclickbidmanager queries create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Query result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn doubleclickbidmanager_queries_create(
        &self,
        args: &DoubleclickbidmanagerQueriesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Query, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = doubleclickbidmanager_queries_create_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = doubleclickbidmanager_queries_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Doubleclickbidmanager queries delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn doubleclickbidmanager_queries_delete(
        &self,
        args: &DoubleclickbidmanagerQueriesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = doubleclickbidmanager_queries_delete_builder(
            &self.http_client,
            &args.queryId,
        )
        .map_err(ProviderError::Api)?;

        let task = doubleclickbidmanager_queries_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Doubleclickbidmanager queries get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Query result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn doubleclickbidmanager_queries_get(
        &self,
        args: &DoubleclickbidmanagerQueriesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Query, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = doubleclickbidmanager_queries_get_builder(
            &self.http_client,
            &args.queryId,
        )
        .map_err(ProviderError::Api)?;

        let task = doubleclickbidmanager_queries_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Doubleclickbidmanager queries list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListQueriesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn doubleclickbidmanager_queries_list(
        &self,
        args: &DoubleclickbidmanagerQueriesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListQueriesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = doubleclickbidmanager_queries_list_builder(
            &self.http_client,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = doubleclickbidmanager_queries_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Doubleclickbidmanager queries run.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn doubleclickbidmanager_queries_run(
        &self,
        args: &DoubleclickbidmanagerQueriesRunArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Report, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = doubleclickbidmanager_queries_run_builder(
            &self.http_client,
            &args.queryId,
            &args.synchronous,
        )
        .map_err(ProviderError::Api)?;

        let task = doubleclickbidmanager_queries_run_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Doubleclickbidmanager queries reports get.
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
    pub fn doubleclickbidmanager_queries_reports_get(
        &self,
        args: &DoubleclickbidmanagerQueriesReportsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Report, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = doubleclickbidmanager_queries_reports_get_builder(
            &self.http_client,
            &args.queryId,
            &args.reportId,
        )
        .map_err(ProviderError::Api)?;

        let task = doubleclickbidmanager_queries_reports_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Doubleclickbidmanager queries reports list.
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
    pub fn doubleclickbidmanager_queries_reports_list(
        &self,
        args: &DoubleclickbidmanagerQueriesReportsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListReportsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = doubleclickbidmanager_queries_reports_list_builder(
            &self.http_client,
            &args.queryId,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = doubleclickbidmanager_queries_reports_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
