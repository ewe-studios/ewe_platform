//! AdminProvider - State-aware admin API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       admin API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::admin::{
    reports_activities_list_builder, reports_activities_list_task,
    reports_activities_watch_builder, reports_activities_watch_task,
    admin_channels_stop_builder, admin_channels_stop_task,
    reports_customer_usage_reports_get_builder, reports_customer_usage_reports_get_task,
    reports_entity_usage_reports_get_builder, reports_entity_usage_reports_get_task,
    reports_user_usage_report_get_builder, reports_user_usage_report_get_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::admin::Activities;
use crate::providers::gcp::clients::admin::Channel;
use crate::providers::gcp::clients::admin::UsageReports;
use crate::providers::gcp::clients::admin::AdminChannelsStopArgs;
use crate::providers::gcp::clients::admin::ReportsActivitiesListArgs;
use crate::providers::gcp::clients::admin::ReportsActivitiesWatchArgs;
use crate::providers::gcp::clients::admin::ReportsCustomerUsageReportsGetArgs;
use crate::providers::gcp::clients::admin::ReportsEntityUsageReportsGetArgs;
use crate::providers::gcp::clients::admin::ReportsUserUsageReportGetArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// AdminProvider with automatic state tracking.
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
/// let provider = AdminProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct AdminProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> AdminProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new AdminProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Reports activities list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Activities result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn reports_activities_list(
        &self,
        args: &ReportsActivitiesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Activities, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = reports_activities_list_builder(
            &self.http_client,
            &args.userKey,
            &args.applicationName,
            &args.actorIpAddress,
            &args.applicationInfoFilter,
            &args.customerId,
            &args.endTime,
            &args.eventName,
            &args.filters,
            &args.groupIdFilter,
            &args.includeSensitiveData,
            &args.maxResults,
            &args.networkInfoFilter,
            &args.orgUnitID,
            &args.pageToken,
            &args.resourceDetailsFilter,
            &args.startTime,
            &args.statusFilter,
        )
        .map_err(ProviderError::Api)?;

        let task = reports_activities_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Reports activities watch.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Channel result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn reports_activities_watch(
        &self,
        args: &ReportsActivitiesWatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Channel, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = reports_activities_watch_builder(
            &self.http_client,
            &args.userKey,
            &args.applicationName,
            &args.actorIpAddress,
            &args.customerId,
            &args.endTime,
            &args.eventName,
            &args.filters,
            &args.groupIdFilter,
            &args.maxResults,
            &args.orgUnitID,
            &args.pageToken,
            &args.startTime,
        )
        .map_err(ProviderError::Api)?;

        let task = reports_activities_watch_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Admin channels stop.
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
    pub fn admin_channels_stop(
        &self,
        args: &AdminChannelsStopArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = admin_channels_stop_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = admin_channels_stop_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Reports customer usage reports get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UsageReports result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn reports_customer_usage_reports_get(
        &self,
        args: &ReportsCustomerUsageReportsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UsageReports, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = reports_customer_usage_reports_get_builder(
            &self.http_client,
            &args.date,
            &args.customerId,
            &args.pageToken,
            &args.parameters,
        )
        .map_err(ProviderError::Api)?;

        let task = reports_customer_usage_reports_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Reports entity usage reports get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UsageReports result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn reports_entity_usage_reports_get(
        &self,
        args: &ReportsEntityUsageReportsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UsageReports, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = reports_entity_usage_reports_get_builder(
            &self.http_client,
            &args.entityType,
            &args.entityKey,
            &args.date,
            &args.customerId,
            &args.filters,
            &args.maxResults,
            &args.pageToken,
            &args.parameters,
        )
        .map_err(ProviderError::Api)?;

        let task = reports_entity_usage_reports_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Reports user usage report get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UsageReports result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn reports_user_usage_report_get(
        &self,
        args: &ReportsUserUsageReportGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UsageReports, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = reports_user_usage_report_get_builder(
            &self.http_client,
            &args.userKey,
            &args.date,
            &args.customerId,
            &args.filters,
            &args.groupIdFilter,
            &args.maxResults,
            &args.orgUnitID,
            &args.pageToken,
            &args.parameters,
        )
        .map_err(ProviderError::Api)?;

        let task = reports_user_usage_report_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
