//! MonitoringProvider - State-aware monitoring API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       monitoring API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::monitoring::{
    monitoring_folders_time_series_list_builder, monitoring_folders_time_series_list_task,
    monitoring_organizations_time_series_list_builder, monitoring_organizations_time_series_list_task,
    monitoring_projects_alert_policies_create_builder, monitoring_projects_alert_policies_create_task,
    monitoring_projects_alert_policies_delete_builder, monitoring_projects_alert_policies_delete_task,
    monitoring_projects_alert_policies_get_builder, monitoring_projects_alert_policies_get_task,
    monitoring_projects_alert_policies_list_builder, monitoring_projects_alert_policies_list_task,
    monitoring_projects_alert_policies_patch_builder, monitoring_projects_alert_policies_patch_task,
    monitoring_projects_alerts_get_builder, monitoring_projects_alerts_get_task,
    monitoring_projects_alerts_list_builder, monitoring_projects_alerts_list_task,
    monitoring_projects_collectd_time_series_create_builder, monitoring_projects_collectd_time_series_create_task,
    monitoring_projects_groups_create_builder, monitoring_projects_groups_create_task,
    monitoring_projects_groups_delete_builder, monitoring_projects_groups_delete_task,
    monitoring_projects_groups_get_builder, monitoring_projects_groups_get_task,
    monitoring_projects_groups_list_builder, monitoring_projects_groups_list_task,
    monitoring_projects_groups_update_builder, monitoring_projects_groups_update_task,
    monitoring_projects_groups_members_list_builder, monitoring_projects_groups_members_list_task,
    monitoring_projects_metric_descriptors_create_builder, monitoring_projects_metric_descriptors_create_task,
    monitoring_projects_metric_descriptors_delete_builder, monitoring_projects_metric_descriptors_delete_task,
    monitoring_projects_metric_descriptors_get_builder, monitoring_projects_metric_descriptors_get_task,
    monitoring_projects_metric_descriptors_list_builder, monitoring_projects_metric_descriptors_list_task,
    monitoring_projects_monitored_resource_descriptors_get_builder, monitoring_projects_monitored_resource_descriptors_get_task,
    monitoring_projects_monitored_resource_descriptors_list_builder, monitoring_projects_monitored_resource_descriptors_list_task,
    monitoring_projects_notification_channel_descriptors_get_builder, monitoring_projects_notification_channel_descriptors_get_task,
    monitoring_projects_notification_channel_descriptors_list_builder, monitoring_projects_notification_channel_descriptors_list_task,
    monitoring_projects_notification_channels_create_builder, monitoring_projects_notification_channels_create_task,
    monitoring_projects_notification_channels_delete_builder, monitoring_projects_notification_channels_delete_task,
    monitoring_projects_notification_channels_get_builder, monitoring_projects_notification_channels_get_task,
    monitoring_projects_notification_channels_get_verification_code_builder, monitoring_projects_notification_channels_get_verification_code_task,
    monitoring_projects_notification_channels_list_builder, monitoring_projects_notification_channels_list_task,
    monitoring_projects_notification_channels_patch_builder, monitoring_projects_notification_channels_patch_task,
    monitoring_projects_notification_channels_send_verification_code_builder, monitoring_projects_notification_channels_send_verification_code_task,
    monitoring_projects_notification_channels_verify_builder, monitoring_projects_notification_channels_verify_task,
    monitoring_projects_snoozes_create_builder, monitoring_projects_snoozes_create_task,
    monitoring_projects_snoozes_get_builder, monitoring_projects_snoozes_get_task,
    monitoring_projects_snoozes_list_builder, monitoring_projects_snoozes_list_task,
    monitoring_projects_snoozes_patch_builder, monitoring_projects_snoozes_patch_task,
    monitoring_projects_time_series_create_builder, monitoring_projects_time_series_create_task,
    monitoring_projects_time_series_create_service_builder, monitoring_projects_time_series_create_service_task,
    monitoring_projects_time_series_list_builder, monitoring_projects_time_series_list_task,
    monitoring_projects_time_series_query_builder, monitoring_projects_time_series_query_task,
    monitoring_projects_uptime_check_configs_create_builder, monitoring_projects_uptime_check_configs_create_task,
    monitoring_projects_uptime_check_configs_delete_builder, monitoring_projects_uptime_check_configs_delete_task,
    monitoring_projects_uptime_check_configs_get_builder, monitoring_projects_uptime_check_configs_get_task,
    monitoring_projects_uptime_check_configs_list_builder, monitoring_projects_uptime_check_configs_list_task,
    monitoring_projects_uptime_check_configs_patch_builder, monitoring_projects_uptime_check_configs_patch_task,
    monitoring_services_create_builder, monitoring_services_create_task,
    monitoring_services_delete_builder, monitoring_services_delete_task,
    monitoring_services_get_builder, monitoring_services_get_task,
    monitoring_services_list_builder, monitoring_services_list_task,
    monitoring_services_patch_builder, monitoring_services_patch_task,
    monitoring_services_service_level_objectives_create_builder, monitoring_services_service_level_objectives_create_task,
    monitoring_services_service_level_objectives_delete_builder, monitoring_services_service_level_objectives_delete_task,
    monitoring_services_service_level_objectives_get_builder, monitoring_services_service_level_objectives_get_task,
    monitoring_services_service_level_objectives_list_builder, monitoring_services_service_level_objectives_list_task,
    monitoring_services_service_level_objectives_patch_builder, monitoring_services_service_level_objectives_patch_task,
    monitoring_uptime_check_ips_list_builder, monitoring_uptime_check_ips_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::monitoring::Alert;
use crate::providers::gcp::clients::monitoring::AlertPolicy;
use crate::providers::gcp::clients::monitoring::CreateCollectdTimeSeriesResponse;
use crate::providers::gcp::clients::monitoring::Empty;
use crate::providers::gcp::clients::monitoring::GetNotificationChannelVerificationCodeResponse;
use crate::providers::gcp::clients::monitoring::Group;
use crate::providers::gcp::clients::monitoring::ListAlertPoliciesResponse;
use crate::providers::gcp::clients::monitoring::ListAlertsResponse;
use crate::providers::gcp::clients::monitoring::ListGroupMembersResponse;
use crate::providers::gcp::clients::monitoring::ListGroupsResponse;
use crate::providers::gcp::clients::monitoring::ListMetricDescriptorsResponse;
use crate::providers::gcp::clients::monitoring::ListMonitoredResourceDescriptorsResponse;
use crate::providers::gcp::clients::monitoring::ListNotificationChannelDescriptorsResponse;
use crate::providers::gcp::clients::monitoring::ListNotificationChannelsResponse;
use crate::providers::gcp::clients::monitoring::ListServiceLevelObjectivesResponse;
use crate::providers::gcp::clients::monitoring::ListServicesResponse;
use crate::providers::gcp::clients::monitoring::ListSnoozesResponse;
use crate::providers::gcp::clients::monitoring::ListTimeSeriesResponse;
use crate::providers::gcp::clients::monitoring::ListUptimeCheckConfigsResponse;
use crate::providers::gcp::clients::monitoring::ListUptimeCheckIpsResponse;
use crate::providers::gcp::clients::monitoring::MetricDescriptor;
use crate::providers::gcp::clients::monitoring::MonitoredResourceDescriptor;
use crate::providers::gcp::clients::monitoring::NotificationChannel;
use crate::providers::gcp::clients::monitoring::NotificationChannelDescriptor;
use crate::providers::gcp::clients::monitoring::QueryTimeSeriesResponse;
use crate::providers::gcp::clients::monitoring::Service;
use crate::providers::gcp::clients::monitoring::ServiceLevelObjective;
use crate::providers::gcp::clients::monitoring::Snooze;
use crate::providers::gcp::clients::monitoring::UptimeCheckConfig;
use crate::providers::gcp::clients::monitoring::MonitoringFoldersTimeSeriesListArgs;
use crate::providers::gcp::clients::monitoring::MonitoringOrganizationsTimeSeriesListArgs;
use crate::providers::gcp::clients::monitoring::MonitoringProjectsAlertPoliciesCreateArgs;
use crate::providers::gcp::clients::monitoring::MonitoringProjectsAlertPoliciesDeleteArgs;
use crate::providers::gcp::clients::monitoring::MonitoringProjectsAlertPoliciesGetArgs;
use crate::providers::gcp::clients::monitoring::MonitoringProjectsAlertPoliciesListArgs;
use crate::providers::gcp::clients::monitoring::MonitoringProjectsAlertPoliciesPatchArgs;
use crate::providers::gcp::clients::monitoring::MonitoringProjectsAlertsGetArgs;
use crate::providers::gcp::clients::monitoring::MonitoringProjectsAlertsListArgs;
use crate::providers::gcp::clients::monitoring::MonitoringProjectsCollectdTimeSeriesCreateArgs;
use crate::providers::gcp::clients::monitoring::MonitoringProjectsGroupsCreateArgs;
use crate::providers::gcp::clients::monitoring::MonitoringProjectsGroupsDeleteArgs;
use crate::providers::gcp::clients::monitoring::MonitoringProjectsGroupsGetArgs;
use crate::providers::gcp::clients::monitoring::MonitoringProjectsGroupsListArgs;
use crate::providers::gcp::clients::monitoring::MonitoringProjectsGroupsMembersListArgs;
use crate::providers::gcp::clients::monitoring::MonitoringProjectsGroupsUpdateArgs;
use crate::providers::gcp::clients::monitoring::MonitoringProjectsMetricDescriptorsCreateArgs;
use crate::providers::gcp::clients::monitoring::MonitoringProjectsMetricDescriptorsDeleteArgs;
use crate::providers::gcp::clients::monitoring::MonitoringProjectsMetricDescriptorsGetArgs;
use crate::providers::gcp::clients::monitoring::MonitoringProjectsMetricDescriptorsListArgs;
use crate::providers::gcp::clients::monitoring::MonitoringProjectsMonitoredResourceDescriptorsGetArgs;
use crate::providers::gcp::clients::monitoring::MonitoringProjectsMonitoredResourceDescriptorsListArgs;
use crate::providers::gcp::clients::monitoring::MonitoringProjectsNotificationChannelDescriptorsGetArgs;
use crate::providers::gcp::clients::monitoring::MonitoringProjectsNotificationChannelDescriptorsListArgs;
use crate::providers::gcp::clients::monitoring::MonitoringProjectsNotificationChannelsCreateArgs;
use crate::providers::gcp::clients::monitoring::MonitoringProjectsNotificationChannelsDeleteArgs;
use crate::providers::gcp::clients::monitoring::MonitoringProjectsNotificationChannelsGetArgs;
use crate::providers::gcp::clients::monitoring::MonitoringProjectsNotificationChannelsGetVerificationCodeArgs;
use crate::providers::gcp::clients::monitoring::MonitoringProjectsNotificationChannelsListArgs;
use crate::providers::gcp::clients::monitoring::MonitoringProjectsNotificationChannelsPatchArgs;
use crate::providers::gcp::clients::monitoring::MonitoringProjectsNotificationChannelsSendVerificationCodeArgs;
use crate::providers::gcp::clients::monitoring::MonitoringProjectsNotificationChannelsVerifyArgs;
use crate::providers::gcp::clients::monitoring::MonitoringProjectsSnoozesCreateArgs;
use crate::providers::gcp::clients::monitoring::MonitoringProjectsSnoozesGetArgs;
use crate::providers::gcp::clients::monitoring::MonitoringProjectsSnoozesListArgs;
use crate::providers::gcp::clients::monitoring::MonitoringProjectsSnoozesPatchArgs;
use crate::providers::gcp::clients::monitoring::MonitoringProjectsTimeSeriesCreateArgs;
use crate::providers::gcp::clients::monitoring::MonitoringProjectsTimeSeriesCreateServiceArgs;
use crate::providers::gcp::clients::monitoring::MonitoringProjectsTimeSeriesListArgs;
use crate::providers::gcp::clients::monitoring::MonitoringProjectsTimeSeriesQueryArgs;
use crate::providers::gcp::clients::monitoring::MonitoringProjectsUptimeCheckConfigsCreateArgs;
use crate::providers::gcp::clients::monitoring::MonitoringProjectsUptimeCheckConfigsDeleteArgs;
use crate::providers::gcp::clients::monitoring::MonitoringProjectsUptimeCheckConfigsGetArgs;
use crate::providers::gcp::clients::monitoring::MonitoringProjectsUptimeCheckConfigsListArgs;
use crate::providers::gcp::clients::monitoring::MonitoringProjectsUptimeCheckConfigsPatchArgs;
use crate::providers::gcp::clients::monitoring::MonitoringServicesCreateArgs;
use crate::providers::gcp::clients::monitoring::MonitoringServicesDeleteArgs;
use crate::providers::gcp::clients::monitoring::MonitoringServicesGetArgs;
use crate::providers::gcp::clients::monitoring::MonitoringServicesListArgs;
use crate::providers::gcp::clients::monitoring::MonitoringServicesPatchArgs;
use crate::providers::gcp::clients::monitoring::MonitoringServicesServiceLevelObjectivesCreateArgs;
use crate::providers::gcp::clients::monitoring::MonitoringServicesServiceLevelObjectivesDeleteArgs;
use crate::providers::gcp::clients::monitoring::MonitoringServicesServiceLevelObjectivesGetArgs;
use crate::providers::gcp::clients::monitoring::MonitoringServicesServiceLevelObjectivesListArgs;
use crate::providers::gcp::clients::monitoring::MonitoringServicesServiceLevelObjectivesPatchArgs;
use crate::providers::gcp::clients::monitoring::MonitoringUptimeCheckIpsListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// MonitoringProvider with automatic state tracking.
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
/// let provider = MonitoringProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct MonitoringProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> MonitoringProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new MonitoringProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Monitoring folders time series list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListTimeSeriesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn monitoring_folders_time_series_list(
        &self,
        args: &MonitoringFoldersTimeSeriesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListTimeSeriesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_folders_time_series_list_builder(
            &self.http_client,
            &args.name,
            &args.aggregation.alignmentPeriod,
            &args.aggregation.crossSeriesReducer,
            &args.aggregation.groupByFields,
            &args.aggregation.perSeriesAligner,
            &args.filter,
            &args.interval.endTime,
            &args.interval.startTime,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.secondaryAggregation.alignmentPeriod,
            &args.secondaryAggregation.crossSeriesReducer,
            &args.secondaryAggregation.groupByFields,
            &args.secondaryAggregation.perSeriesAligner,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_folders_time_series_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring organizations time series list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListTimeSeriesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn monitoring_organizations_time_series_list(
        &self,
        args: &MonitoringOrganizationsTimeSeriesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListTimeSeriesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_organizations_time_series_list_builder(
            &self.http_client,
            &args.name,
            &args.aggregation.alignmentPeriod,
            &args.aggregation.crossSeriesReducer,
            &args.aggregation.groupByFields,
            &args.aggregation.perSeriesAligner,
            &args.filter,
            &args.interval.endTime,
            &args.interval.startTime,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.secondaryAggregation.alignmentPeriod,
            &args.secondaryAggregation.crossSeriesReducer,
            &args.secondaryAggregation.groupByFields,
            &args.secondaryAggregation.perSeriesAligner,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_organizations_time_series_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring projects alert policies create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AlertPolicy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn monitoring_projects_alert_policies_create(
        &self,
        args: &MonitoringProjectsAlertPoliciesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AlertPolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_projects_alert_policies_create_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_projects_alert_policies_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring projects alert policies delete.
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
    pub fn monitoring_projects_alert_policies_delete(
        &self,
        args: &MonitoringProjectsAlertPoliciesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_projects_alert_policies_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_projects_alert_policies_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring projects alert policies get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AlertPolicy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn monitoring_projects_alert_policies_get(
        &self,
        args: &MonitoringProjectsAlertPoliciesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AlertPolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_projects_alert_policies_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_projects_alert_policies_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring projects alert policies list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAlertPoliciesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn monitoring_projects_alert_policies_list(
        &self,
        args: &MonitoringProjectsAlertPoliciesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAlertPoliciesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_projects_alert_policies_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_projects_alert_policies_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring projects alert policies patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AlertPolicy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn monitoring_projects_alert_policies_patch(
        &self,
        args: &MonitoringProjectsAlertPoliciesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AlertPolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_projects_alert_policies_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_projects_alert_policies_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring projects alerts get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Alert result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn monitoring_projects_alerts_get(
        &self,
        args: &MonitoringProjectsAlertsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Alert, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_projects_alerts_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_projects_alerts_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring projects alerts list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAlertsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn monitoring_projects_alerts_list(
        &self,
        args: &MonitoringProjectsAlertsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAlertsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_projects_alerts_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_projects_alerts_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring projects collectd time series create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CreateCollectdTimeSeriesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn monitoring_projects_collectd_time_series_create(
        &self,
        args: &MonitoringProjectsCollectdTimeSeriesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CreateCollectdTimeSeriesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_projects_collectd_time_series_create_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_projects_collectd_time_series_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring projects groups create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Group result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn monitoring_projects_groups_create(
        &self,
        args: &MonitoringProjectsGroupsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Group, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_projects_groups_create_builder(
            &self.http_client,
            &args.name,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_projects_groups_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring projects groups delete.
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
    pub fn monitoring_projects_groups_delete(
        &self,
        args: &MonitoringProjectsGroupsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_projects_groups_delete_builder(
            &self.http_client,
            &args.name,
            &args.recursive,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_projects_groups_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring projects groups get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Group result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn monitoring_projects_groups_get(
        &self,
        args: &MonitoringProjectsGroupsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Group, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_projects_groups_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_projects_groups_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring projects groups list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListGroupsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn monitoring_projects_groups_list(
        &self,
        args: &MonitoringProjectsGroupsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListGroupsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_projects_groups_list_builder(
            &self.http_client,
            &args.name,
            &args.ancestorsOfGroup,
            &args.childrenOfGroup,
            &args.descendantsOfGroup,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_projects_groups_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring projects groups update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Group result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn monitoring_projects_groups_update(
        &self,
        args: &MonitoringProjectsGroupsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Group, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_projects_groups_update_builder(
            &self.http_client,
            &args.name,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_projects_groups_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring projects groups members list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListGroupMembersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn monitoring_projects_groups_members_list(
        &self,
        args: &MonitoringProjectsGroupsMembersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListGroupMembersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_projects_groups_members_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.interval.endTime,
            &args.interval.startTime,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_projects_groups_members_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring projects metric descriptors create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the MetricDescriptor result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn monitoring_projects_metric_descriptors_create(
        &self,
        args: &MonitoringProjectsMetricDescriptorsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<MetricDescriptor, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_projects_metric_descriptors_create_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_projects_metric_descriptors_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring projects metric descriptors delete.
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
    pub fn monitoring_projects_metric_descriptors_delete(
        &self,
        args: &MonitoringProjectsMetricDescriptorsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_projects_metric_descriptors_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_projects_metric_descriptors_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring projects metric descriptors get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the MetricDescriptor result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn monitoring_projects_metric_descriptors_get(
        &self,
        args: &MonitoringProjectsMetricDescriptorsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<MetricDescriptor, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_projects_metric_descriptors_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_projects_metric_descriptors_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring projects metric descriptors list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListMetricDescriptorsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn monitoring_projects_metric_descriptors_list(
        &self,
        args: &MonitoringProjectsMetricDescriptorsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListMetricDescriptorsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_projects_metric_descriptors_list_builder(
            &self.http_client,
            &args.name,
            &args.activeOnly,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_projects_metric_descriptors_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring projects monitored resource descriptors get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the MonitoredResourceDescriptor result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn monitoring_projects_monitored_resource_descriptors_get(
        &self,
        args: &MonitoringProjectsMonitoredResourceDescriptorsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<MonitoredResourceDescriptor, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_projects_monitored_resource_descriptors_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_projects_monitored_resource_descriptors_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring projects monitored resource descriptors list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListMonitoredResourceDescriptorsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn monitoring_projects_monitored_resource_descriptors_list(
        &self,
        args: &MonitoringProjectsMonitoredResourceDescriptorsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListMonitoredResourceDescriptorsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_projects_monitored_resource_descriptors_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_projects_monitored_resource_descriptors_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring projects notification channel descriptors get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NotificationChannelDescriptor result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn monitoring_projects_notification_channel_descriptors_get(
        &self,
        args: &MonitoringProjectsNotificationChannelDescriptorsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NotificationChannelDescriptor, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_projects_notification_channel_descriptors_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_projects_notification_channel_descriptors_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring projects notification channel descriptors list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListNotificationChannelDescriptorsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn monitoring_projects_notification_channel_descriptors_list(
        &self,
        args: &MonitoringProjectsNotificationChannelDescriptorsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListNotificationChannelDescriptorsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_projects_notification_channel_descriptors_list_builder(
            &self.http_client,
            &args.name,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_projects_notification_channel_descriptors_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring projects notification channels create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NotificationChannel result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn monitoring_projects_notification_channels_create(
        &self,
        args: &MonitoringProjectsNotificationChannelsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NotificationChannel, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_projects_notification_channels_create_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_projects_notification_channels_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring projects notification channels delete.
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
    pub fn monitoring_projects_notification_channels_delete(
        &self,
        args: &MonitoringProjectsNotificationChannelsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_projects_notification_channels_delete_builder(
            &self.http_client,
            &args.name,
            &args.force,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_projects_notification_channels_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring projects notification channels get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NotificationChannel result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn monitoring_projects_notification_channels_get(
        &self,
        args: &MonitoringProjectsNotificationChannelsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NotificationChannel, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_projects_notification_channels_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_projects_notification_channels_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring projects notification channels get verification code.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GetNotificationChannelVerificationCodeResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn monitoring_projects_notification_channels_get_verification_code(
        &self,
        args: &MonitoringProjectsNotificationChannelsGetVerificationCodeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GetNotificationChannelVerificationCodeResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_projects_notification_channels_get_verification_code_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_projects_notification_channels_get_verification_code_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring projects notification channels list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListNotificationChannelsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn monitoring_projects_notification_channels_list(
        &self,
        args: &MonitoringProjectsNotificationChannelsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListNotificationChannelsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_projects_notification_channels_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_projects_notification_channels_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring projects notification channels patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NotificationChannel result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn monitoring_projects_notification_channels_patch(
        &self,
        args: &MonitoringProjectsNotificationChannelsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NotificationChannel, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_projects_notification_channels_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_projects_notification_channels_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring projects notification channels send verification code.
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
    pub fn monitoring_projects_notification_channels_send_verification_code(
        &self,
        args: &MonitoringProjectsNotificationChannelsSendVerificationCodeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_projects_notification_channels_send_verification_code_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_projects_notification_channels_send_verification_code_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring projects notification channels verify.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NotificationChannel result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn monitoring_projects_notification_channels_verify(
        &self,
        args: &MonitoringProjectsNotificationChannelsVerifyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NotificationChannel, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_projects_notification_channels_verify_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_projects_notification_channels_verify_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring projects snoozes create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Snooze result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn monitoring_projects_snoozes_create(
        &self,
        args: &MonitoringProjectsSnoozesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Snooze, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_projects_snoozes_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_projects_snoozes_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring projects snoozes get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Snooze result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn monitoring_projects_snoozes_get(
        &self,
        args: &MonitoringProjectsSnoozesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Snooze, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_projects_snoozes_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_projects_snoozes_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring projects snoozes list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListSnoozesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn monitoring_projects_snoozes_list(
        &self,
        args: &MonitoringProjectsSnoozesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSnoozesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_projects_snoozes_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_projects_snoozes_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring projects snoozes patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Snooze result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn monitoring_projects_snoozes_patch(
        &self,
        args: &MonitoringProjectsSnoozesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Snooze, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_projects_snoozes_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_projects_snoozes_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring projects time series create.
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
    pub fn monitoring_projects_time_series_create(
        &self,
        args: &MonitoringProjectsTimeSeriesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_projects_time_series_create_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_projects_time_series_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring projects time series create service.
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
    pub fn monitoring_projects_time_series_create_service(
        &self,
        args: &MonitoringProjectsTimeSeriesCreateServiceArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_projects_time_series_create_service_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_projects_time_series_create_service_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring projects time series list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListTimeSeriesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn monitoring_projects_time_series_list(
        &self,
        args: &MonitoringProjectsTimeSeriesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListTimeSeriesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_projects_time_series_list_builder(
            &self.http_client,
            &args.name,
            &args.aggregation.alignmentPeriod,
            &args.aggregation.crossSeriesReducer,
            &args.aggregation.groupByFields,
            &args.aggregation.perSeriesAligner,
            &args.filter,
            &args.interval.endTime,
            &args.interval.startTime,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.secondaryAggregation.alignmentPeriod,
            &args.secondaryAggregation.crossSeriesReducer,
            &args.secondaryAggregation.groupByFields,
            &args.secondaryAggregation.perSeriesAligner,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_projects_time_series_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring projects time series query.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the QueryTimeSeriesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn monitoring_projects_time_series_query(
        &self,
        args: &MonitoringProjectsTimeSeriesQueryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<QueryTimeSeriesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_projects_time_series_query_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_projects_time_series_query_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring projects uptime check configs create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UptimeCheckConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn monitoring_projects_uptime_check_configs_create(
        &self,
        args: &MonitoringProjectsUptimeCheckConfigsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UptimeCheckConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_projects_uptime_check_configs_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_projects_uptime_check_configs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring projects uptime check configs delete.
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
    pub fn monitoring_projects_uptime_check_configs_delete(
        &self,
        args: &MonitoringProjectsUptimeCheckConfigsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_projects_uptime_check_configs_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_projects_uptime_check_configs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring projects uptime check configs get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UptimeCheckConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn monitoring_projects_uptime_check_configs_get(
        &self,
        args: &MonitoringProjectsUptimeCheckConfigsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UptimeCheckConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_projects_uptime_check_configs_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_projects_uptime_check_configs_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring projects uptime check configs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListUptimeCheckConfigsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn monitoring_projects_uptime_check_configs_list(
        &self,
        args: &MonitoringProjectsUptimeCheckConfigsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListUptimeCheckConfigsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_projects_uptime_check_configs_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_projects_uptime_check_configs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring projects uptime check configs patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UptimeCheckConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn monitoring_projects_uptime_check_configs_patch(
        &self,
        args: &MonitoringProjectsUptimeCheckConfigsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UptimeCheckConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_projects_uptime_check_configs_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_projects_uptime_check_configs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring services create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Service result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn monitoring_services_create(
        &self,
        args: &MonitoringServicesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Service, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_services_create_builder(
            &self.http_client,
            &args.parent,
            &args.serviceId,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_services_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring services delete.
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
    pub fn monitoring_services_delete(
        &self,
        args: &MonitoringServicesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_services_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_services_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring services get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Service result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn monitoring_services_get(
        &self,
        args: &MonitoringServicesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Service, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_services_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_services_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring services list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListServicesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn monitoring_services_list(
        &self,
        args: &MonitoringServicesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListServicesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_services_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_services_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring services patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Service result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn monitoring_services_patch(
        &self,
        args: &MonitoringServicesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Service, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_services_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_services_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring services service level objectives create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ServiceLevelObjective result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn monitoring_services_service_level_objectives_create(
        &self,
        args: &MonitoringServicesServiceLevelObjectivesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ServiceLevelObjective, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_services_service_level_objectives_create_builder(
            &self.http_client,
            &args.parent,
            &args.serviceLevelObjectiveId,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_services_service_level_objectives_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring services service level objectives delete.
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
    pub fn monitoring_services_service_level_objectives_delete(
        &self,
        args: &MonitoringServicesServiceLevelObjectivesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_services_service_level_objectives_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_services_service_level_objectives_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring services service level objectives get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ServiceLevelObjective result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn monitoring_services_service_level_objectives_get(
        &self,
        args: &MonitoringServicesServiceLevelObjectivesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ServiceLevelObjective, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_services_service_level_objectives_get_builder(
            &self.http_client,
            &args.name,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_services_service_level_objectives_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring services service level objectives list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListServiceLevelObjectivesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn monitoring_services_service_level_objectives_list(
        &self,
        args: &MonitoringServicesServiceLevelObjectivesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListServiceLevelObjectivesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_services_service_level_objectives_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_services_service_level_objectives_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring services service level objectives patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ServiceLevelObjective result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn monitoring_services_service_level_objectives_patch(
        &self,
        args: &MonitoringServicesServiceLevelObjectivesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ServiceLevelObjective, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_services_service_level_objectives_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_services_service_level_objectives_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Monitoring uptime check ips list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListUptimeCheckIpsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn monitoring_uptime_check_ips_list(
        &self,
        args: &MonitoringUptimeCheckIpsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListUptimeCheckIpsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = monitoring_uptime_check_ips_list_builder(
            &self.http_client,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = monitoring_uptime_check_ips_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
