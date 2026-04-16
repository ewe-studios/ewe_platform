//! ClouderrorreportingProvider - State-aware clouderrorreporting API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       clouderrorreporting API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::clouderrorreporting::{
    clouderrorreporting_projects_delete_events_builder, clouderrorreporting_projects_delete_events_task,
    clouderrorreporting_projects_events_list_builder, clouderrorreporting_projects_events_list_task,
    clouderrorreporting_projects_events_report_builder, clouderrorreporting_projects_events_report_task,
    clouderrorreporting_projects_group_stats_list_builder, clouderrorreporting_projects_group_stats_list_task,
    clouderrorreporting_projects_groups_get_builder, clouderrorreporting_projects_groups_get_task,
    clouderrorreporting_projects_groups_update_builder, clouderrorreporting_projects_groups_update_task,
    clouderrorreporting_projects_locations_delete_events_builder, clouderrorreporting_projects_locations_delete_events_task,
    clouderrorreporting_projects_locations_events_list_builder, clouderrorreporting_projects_locations_events_list_task,
    clouderrorreporting_projects_locations_group_stats_list_builder, clouderrorreporting_projects_locations_group_stats_list_task,
    clouderrorreporting_projects_locations_groups_get_builder, clouderrorreporting_projects_locations_groups_get_task,
    clouderrorreporting_projects_locations_groups_update_builder, clouderrorreporting_projects_locations_groups_update_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::clouderrorreporting::DeleteEventsResponse;
use crate::providers::gcp::clients::clouderrorreporting::ErrorGroup;
use crate::providers::gcp::clients::clouderrorreporting::ListEventsResponse;
use crate::providers::gcp::clients::clouderrorreporting::ListGroupStatsResponse;
use crate::providers::gcp::clients::clouderrorreporting::ReportErrorEventResponse;
use crate::providers::gcp::clients::clouderrorreporting::ClouderrorreportingProjectsDeleteEventsArgs;
use crate::providers::gcp::clients::clouderrorreporting::ClouderrorreportingProjectsEventsListArgs;
use crate::providers::gcp::clients::clouderrorreporting::ClouderrorreportingProjectsEventsReportArgs;
use crate::providers::gcp::clients::clouderrorreporting::ClouderrorreportingProjectsGroupStatsListArgs;
use crate::providers::gcp::clients::clouderrorreporting::ClouderrorreportingProjectsGroupsGetArgs;
use crate::providers::gcp::clients::clouderrorreporting::ClouderrorreportingProjectsGroupsUpdateArgs;
use crate::providers::gcp::clients::clouderrorreporting::ClouderrorreportingProjectsLocationsDeleteEventsArgs;
use crate::providers::gcp::clients::clouderrorreporting::ClouderrorreportingProjectsLocationsEventsListArgs;
use crate::providers::gcp::clients::clouderrorreporting::ClouderrorreportingProjectsLocationsGroupStatsListArgs;
use crate::providers::gcp::clients::clouderrorreporting::ClouderrorreportingProjectsLocationsGroupsGetArgs;
use crate::providers::gcp::clients::clouderrorreporting::ClouderrorreportingProjectsLocationsGroupsUpdateArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// ClouderrorreportingProvider with automatic state tracking.
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
/// let provider = ClouderrorreportingProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct ClouderrorreportingProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> ClouderrorreportingProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new ClouderrorreportingProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new ClouderrorreportingProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Clouderrorreporting projects delete events.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeleteEventsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn clouderrorreporting_projects_delete_events(
        &self,
        args: &ClouderrorreportingProjectsDeleteEventsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeleteEventsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouderrorreporting_projects_delete_events_builder(
            &self.http_client,
            &args.projectName,
        )
        .map_err(ProviderError::Api)?;

        let task = clouderrorreporting_projects_delete_events_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouderrorreporting projects events list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListEventsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn clouderrorreporting_projects_events_list(
        &self,
        args: &ClouderrorreportingProjectsEventsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListEventsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouderrorreporting_projects_events_list_builder(
            &self.http_client,
            &args.projectName,
            &args.groupId,
            &args.pageSize,
            &args.pageToken,
            &args.serviceFilter_resourceType,
            &args.serviceFilter_service,
            &args.serviceFilter_version,
            &args.timeRange_period,
        )
        .map_err(ProviderError::Api)?;

        let task = clouderrorreporting_projects_events_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouderrorreporting projects events report.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ReportErrorEventResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn clouderrorreporting_projects_events_report(
        &self,
        args: &ClouderrorreportingProjectsEventsReportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ReportErrorEventResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouderrorreporting_projects_events_report_builder(
            &self.http_client,
            &args.projectName,
        )
        .map_err(ProviderError::Api)?;

        let task = clouderrorreporting_projects_events_report_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouderrorreporting projects group stats list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListGroupStatsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn clouderrorreporting_projects_group_stats_list(
        &self,
        args: &ClouderrorreportingProjectsGroupStatsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListGroupStatsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouderrorreporting_projects_group_stats_list_builder(
            &self.http_client,
            &args.projectName,
            &args.alignment,
            &args.alignmentTime,
            &args.groupId,
            &args.order,
            &args.pageSize,
            &args.pageToken,
            &args.serviceFilter_resourceType,
            &args.serviceFilter_service,
            &args.serviceFilter_version,
            &args.timeRange_period,
            &args.timedCountDuration,
        )
        .map_err(ProviderError::Api)?;

        let task = clouderrorreporting_projects_group_stats_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouderrorreporting projects groups get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ErrorGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn clouderrorreporting_projects_groups_get(
        &self,
        args: &ClouderrorreportingProjectsGroupsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ErrorGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouderrorreporting_projects_groups_get_builder(
            &self.http_client,
            &args.groupName,
        )
        .map_err(ProviderError::Api)?;

        let task = clouderrorreporting_projects_groups_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouderrorreporting projects groups update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ErrorGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn clouderrorreporting_projects_groups_update(
        &self,
        args: &ClouderrorreportingProjectsGroupsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ErrorGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouderrorreporting_projects_groups_update_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = clouderrorreporting_projects_groups_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouderrorreporting projects locations delete events.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeleteEventsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn clouderrorreporting_projects_locations_delete_events(
        &self,
        args: &ClouderrorreportingProjectsLocationsDeleteEventsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeleteEventsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouderrorreporting_projects_locations_delete_events_builder(
            &self.http_client,
            &args.projectName,
        )
        .map_err(ProviderError::Api)?;

        let task = clouderrorreporting_projects_locations_delete_events_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouderrorreporting projects locations events list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListEventsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn clouderrorreporting_projects_locations_events_list(
        &self,
        args: &ClouderrorreportingProjectsLocationsEventsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListEventsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouderrorreporting_projects_locations_events_list_builder(
            &self.http_client,
            &args.projectName,
            &args.groupId,
            &args.pageSize,
            &args.pageToken,
            &args.serviceFilter_resourceType,
            &args.serviceFilter_service,
            &args.serviceFilter_version,
            &args.timeRange_period,
        )
        .map_err(ProviderError::Api)?;

        let task = clouderrorreporting_projects_locations_events_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouderrorreporting projects locations group stats list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListGroupStatsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn clouderrorreporting_projects_locations_group_stats_list(
        &self,
        args: &ClouderrorreportingProjectsLocationsGroupStatsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListGroupStatsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouderrorreporting_projects_locations_group_stats_list_builder(
            &self.http_client,
            &args.projectName,
            &args.alignment,
            &args.alignmentTime,
            &args.groupId,
            &args.order,
            &args.pageSize,
            &args.pageToken,
            &args.serviceFilter_resourceType,
            &args.serviceFilter_service,
            &args.serviceFilter_version,
            &args.timeRange_period,
            &args.timedCountDuration,
        )
        .map_err(ProviderError::Api)?;

        let task = clouderrorreporting_projects_locations_group_stats_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouderrorreporting projects locations groups get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ErrorGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn clouderrorreporting_projects_locations_groups_get(
        &self,
        args: &ClouderrorreportingProjectsLocationsGroupsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ErrorGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouderrorreporting_projects_locations_groups_get_builder(
            &self.http_client,
            &args.groupName,
        )
        .map_err(ProviderError::Api)?;

        let task = clouderrorreporting_projects_locations_groups_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouderrorreporting projects locations groups update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ErrorGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn clouderrorreporting_projects_locations_groups_update(
        &self,
        args: &ClouderrorreportingProjectsLocationsGroupsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ErrorGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouderrorreporting_projects_locations_groups_update_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = clouderrorreporting_projects_locations_groups_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
