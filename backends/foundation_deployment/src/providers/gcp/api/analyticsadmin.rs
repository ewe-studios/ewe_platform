//! AnalyticsadminProvider - State-aware analyticsadmin API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       analyticsadmin API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::analyticsadmin::{
    analyticsadmin_account_summaries_list_builder, analyticsadmin_account_summaries_list_task,
    analyticsadmin_accounts_delete_builder, analyticsadmin_accounts_delete_task,
    analyticsadmin_accounts_get_builder, analyticsadmin_accounts_get_task,
    analyticsadmin_accounts_get_data_sharing_settings_builder, analyticsadmin_accounts_get_data_sharing_settings_task,
    analyticsadmin_accounts_list_builder, analyticsadmin_accounts_list_task,
    analyticsadmin_accounts_patch_builder, analyticsadmin_accounts_patch_task,
    analyticsadmin_accounts_provision_account_ticket_builder, analyticsadmin_accounts_provision_account_ticket_task,
    analyticsadmin_accounts_run_access_report_builder, analyticsadmin_accounts_run_access_report_task,
    analyticsadmin_accounts_search_change_history_events_builder, analyticsadmin_accounts_search_change_history_events_task,
    analyticsadmin_properties_acknowledge_user_data_collection_builder, analyticsadmin_properties_acknowledge_user_data_collection_task,
    analyticsadmin_properties_create_builder, analyticsadmin_properties_create_task,
    analyticsadmin_properties_delete_builder, analyticsadmin_properties_delete_task,
    analyticsadmin_properties_get_builder, analyticsadmin_properties_get_task,
    analyticsadmin_properties_get_data_retention_settings_builder, analyticsadmin_properties_get_data_retention_settings_task,
    analyticsadmin_properties_list_builder, analyticsadmin_properties_list_task,
    analyticsadmin_properties_patch_builder, analyticsadmin_properties_patch_task,
    analyticsadmin_properties_run_access_report_builder, analyticsadmin_properties_run_access_report_task,
    analyticsadmin_properties_update_data_retention_settings_builder, analyticsadmin_properties_update_data_retention_settings_task,
    analyticsadmin_properties_conversion_events_create_builder, analyticsadmin_properties_conversion_events_create_task,
    analyticsadmin_properties_conversion_events_delete_builder, analyticsadmin_properties_conversion_events_delete_task,
    analyticsadmin_properties_conversion_events_get_builder, analyticsadmin_properties_conversion_events_get_task,
    analyticsadmin_properties_conversion_events_list_builder, analyticsadmin_properties_conversion_events_list_task,
    analyticsadmin_properties_conversion_events_patch_builder, analyticsadmin_properties_conversion_events_patch_task,
    analyticsadmin_properties_custom_dimensions_archive_builder, analyticsadmin_properties_custom_dimensions_archive_task,
    analyticsadmin_properties_custom_dimensions_create_builder, analyticsadmin_properties_custom_dimensions_create_task,
    analyticsadmin_properties_custom_dimensions_get_builder, analyticsadmin_properties_custom_dimensions_get_task,
    analyticsadmin_properties_custom_dimensions_list_builder, analyticsadmin_properties_custom_dimensions_list_task,
    analyticsadmin_properties_custom_dimensions_patch_builder, analyticsadmin_properties_custom_dimensions_patch_task,
    analyticsadmin_properties_custom_metrics_archive_builder, analyticsadmin_properties_custom_metrics_archive_task,
    analyticsadmin_properties_custom_metrics_create_builder, analyticsadmin_properties_custom_metrics_create_task,
    analyticsadmin_properties_custom_metrics_get_builder, analyticsadmin_properties_custom_metrics_get_task,
    analyticsadmin_properties_custom_metrics_list_builder, analyticsadmin_properties_custom_metrics_list_task,
    analyticsadmin_properties_custom_metrics_patch_builder, analyticsadmin_properties_custom_metrics_patch_task,
    analyticsadmin_properties_data_streams_create_builder, analyticsadmin_properties_data_streams_create_task,
    analyticsadmin_properties_data_streams_delete_builder, analyticsadmin_properties_data_streams_delete_task,
    analyticsadmin_properties_data_streams_get_builder, analyticsadmin_properties_data_streams_get_task,
    analyticsadmin_properties_data_streams_list_builder, analyticsadmin_properties_data_streams_list_task,
    analyticsadmin_properties_data_streams_patch_builder, analyticsadmin_properties_data_streams_patch_task,
    analyticsadmin_properties_data_streams_measurement_protocol_secrets_create_builder, analyticsadmin_properties_data_streams_measurement_protocol_secrets_create_task,
    analyticsadmin_properties_data_streams_measurement_protocol_secrets_delete_builder, analyticsadmin_properties_data_streams_measurement_protocol_secrets_delete_task,
    analyticsadmin_properties_data_streams_measurement_protocol_secrets_get_builder, analyticsadmin_properties_data_streams_measurement_protocol_secrets_get_task,
    analyticsadmin_properties_data_streams_measurement_protocol_secrets_list_builder, analyticsadmin_properties_data_streams_measurement_protocol_secrets_list_task,
    analyticsadmin_properties_data_streams_measurement_protocol_secrets_patch_builder, analyticsadmin_properties_data_streams_measurement_protocol_secrets_patch_task,
    analyticsadmin_properties_firebase_links_create_builder, analyticsadmin_properties_firebase_links_create_task,
    analyticsadmin_properties_firebase_links_delete_builder, analyticsadmin_properties_firebase_links_delete_task,
    analyticsadmin_properties_firebase_links_list_builder, analyticsadmin_properties_firebase_links_list_task,
    analyticsadmin_properties_google_ads_links_create_builder, analyticsadmin_properties_google_ads_links_create_task,
    analyticsadmin_properties_google_ads_links_delete_builder, analyticsadmin_properties_google_ads_links_delete_task,
    analyticsadmin_properties_google_ads_links_list_builder, analyticsadmin_properties_google_ads_links_list_task,
    analyticsadmin_properties_google_ads_links_patch_builder, analyticsadmin_properties_google_ads_links_patch_task,
    analyticsadmin_properties_key_events_create_builder, analyticsadmin_properties_key_events_create_task,
    analyticsadmin_properties_key_events_delete_builder, analyticsadmin_properties_key_events_delete_task,
    analyticsadmin_properties_key_events_get_builder, analyticsadmin_properties_key_events_get_task,
    analyticsadmin_properties_key_events_list_builder, analyticsadmin_properties_key_events_list_task,
    analyticsadmin_properties_key_events_patch_builder, analyticsadmin_properties_key_events_patch_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::analyticsadmin::GoogleAnalyticsAdminV1betaAccount;
use crate::providers::gcp::clients::analyticsadmin::GoogleAnalyticsAdminV1betaAcknowledgeUserDataCollectionResponse;
use crate::providers::gcp::clients::analyticsadmin::GoogleAnalyticsAdminV1betaConversionEvent;
use crate::providers::gcp::clients::analyticsadmin::GoogleAnalyticsAdminV1betaCustomDimension;
use crate::providers::gcp::clients::analyticsadmin::GoogleAnalyticsAdminV1betaCustomMetric;
use crate::providers::gcp::clients::analyticsadmin::GoogleAnalyticsAdminV1betaDataRetentionSettings;
use crate::providers::gcp::clients::analyticsadmin::GoogleAnalyticsAdminV1betaDataSharingSettings;
use crate::providers::gcp::clients::analyticsadmin::GoogleAnalyticsAdminV1betaDataStream;
use crate::providers::gcp::clients::analyticsadmin::GoogleAnalyticsAdminV1betaFirebaseLink;
use crate::providers::gcp::clients::analyticsadmin::GoogleAnalyticsAdminV1betaGoogleAdsLink;
use crate::providers::gcp::clients::analyticsadmin::GoogleAnalyticsAdminV1betaKeyEvent;
use crate::providers::gcp::clients::analyticsadmin::GoogleAnalyticsAdminV1betaListAccountSummariesResponse;
use crate::providers::gcp::clients::analyticsadmin::GoogleAnalyticsAdminV1betaListAccountsResponse;
use crate::providers::gcp::clients::analyticsadmin::GoogleAnalyticsAdminV1betaListConversionEventsResponse;
use crate::providers::gcp::clients::analyticsadmin::GoogleAnalyticsAdminV1betaListCustomDimensionsResponse;
use crate::providers::gcp::clients::analyticsadmin::GoogleAnalyticsAdminV1betaListCustomMetricsResponse;
use crate::providers::gcp::clients::analyticsadmin::GoogleAnalyticsAdminV1betaListDataStreamsResponse;
use crate::providers::gcp::clients::analyticsadmin::GoogleAnalyticsAdminV1betaListFirebaseLinksResponse;
use crate::providers::gcp::clients::analyticsadmin::GoogleAnalyticsAdminV1betaListGoogleAdsLinksResponse;
use crate::providers::gcp::clients::analyticsadmin::GoogleAnalyticsAdminV1betaListKeyEventsResponse;
use crate::providers::gcp::clients::analyticsadmin::GoogleAnalyticsAdminV1betaListMeasurementProtocolSecretsResponse;
use crate::providers::gcp::clients::analyticsadmin::GoogleAnalyticsAdminV1betaListPropertiesResponse;
use crate::providers::gcp::clients::analyticsadmin::GoogleAnalyticsAdminV1betaMeasurementProtocolSecret;
use crate::providers::gcp::clients::analyticsadmin::GoogleAnalyticsAdminV1betaProperty;
use crate::providers::gcp::clients::analyticsadmin::GoogleAnalyticsAdminV1betaProvisionAccountTicketResponse;
use crate::providers::gcp::clients::analyticsadmin::GoogleAnalyticsAdminV1betaRunAccessReportResponse;
use crate::providers::gcp::clients::analyticsadmin::GoogleAnalyticsAdminV1betaSearchChangeHistoryEventsResponse;
use crate::providers::gcp::clients::analyticsadmin::GoogleProtobufEmpty;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminAccountSummariesListArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminAccountsDeleteArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminAccountsGetArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminAccountsGetDataSharingSettingsArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminAccountsListArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminAccountsPatchArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminAccountsProvisionAccountTicketArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminAccountsRunAccessReportArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminAccountsSearchChangeHistoryEventsArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminPropertiesAcknowledgeUserDataCollectionArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminPropertiesConversionEventsCreateArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminPropertiesConversionEventsDeleteArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminPropertiesConversionEventsGetArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminPropertiesConversionEventsListArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminPropertiesConversionEventsPatchArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminPropertiesCreateArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminPropertiesCustomDimensionsArchiveArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminPropertiesCustomDimensionsCreateArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminPropertiesCustomDimensionsGetArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminPropertiesCustomDimensionsListArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminPropertiesCustomDimensionsPatchArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminPropertiesCustomMetricsArchiveArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminPropertiesCustomMetricsCreateArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminPropertiesCustomMetricsGetArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminPropertiesCustomMetricsListArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminPropertiesCustomMetricsPatchArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminPropertiesDataStreamsCreateArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminPropertiesDataStreamsDeleteArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminPropertiesDataStreamsGetArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminPropertiesDataStreamsListArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminPropertiesDataStreamsMeasurementProtocolSecretsCreateArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminPropertiesDataStreamsMeasurementProtocolSecretsDeleteArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminPropertiesDataStreamsMeasurementProtocolSecretsGetArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminPropertiesDataStreamsMeasurementProtocolSecretsListArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminPropertiesDataStreamsMeasurementProtocolSecretsPatchArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminPropertiesDataStreamsPatchArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminPropertiesDeleteArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminPropertiesFirebaseLinksCreateArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminPropertiesFirebaseLinksDeleteArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminPropertiesFirebaseLinksListArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminPropertiesGetArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminPropertiesGetDataRetentionSettingsArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminPropertiesGoogleAdsLinksCreateArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminPropertiesGoogleAdsLinksDeleteArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminPropertiesGoogleAdsLinksListArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminPropertiesGoogleAdsLinksPatchArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminPropertiesKeyEventsCreateArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminPropertiesKeyEventsDeleteArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminPropertiesKeyEventsGetArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminPropertiesKeyEventsListArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminPropertiesKeyEventsPatchArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminPropertiesListArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminPropertiesPatchArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminPropertiesRunAccessReportArgs;
use crate::providers::gcp::clients::analyticsadmin::AnalyticsadminPropertiesUpdateDataRetentionSettingsArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// AnalyticsadminProvider with automatic state tracking.
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
/// let provider = AnalyticsadminProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct AnalyticsadminProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> AnalyticsadminProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new AnalyticsadminProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Analyticsadmin account summaries list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAnalyticsAdminV1betaListAccountSummariesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn analyticsadmin_account_summaries_list(
        &self,
        args: &AnalyticsadminAccountSummariesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAnalyticsAdminV1betaListAccountSummariesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_account_summaries_list_builder(
            &self.http_client,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_account_summaries_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin accounts delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analyticsadmin_accounts_delete(
        &self,
        args: &AnalyticsadminAccountsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_accounts_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_accounts_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin accounts get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAnalyticsAdminV1betaAccount result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn analyticsadmin_accounts_get(
        &self,
        args: &AnalyticsadminAccountsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAnalyticsAdminV1betaAccount, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_accounts_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_accounts_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin accounts get data sharing settings.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAnalyticsAdminV1betaDataSharingSettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn analyticsadmin_accounts_get_data_sharing_settings(
        &self,
        args: &AnalyticsadminAccountsGetDataSharingSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAnalyticsAdminV1betaDataSharingSettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_accounts_get_data_sharing_settings_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_accounts_get_data_sharing_settings_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin accounts list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAnalyticsAdminV1betaListAccountsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn analyticsadmin_accounts_list(
        &self,
        args: &AnalyticsadminAccountsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAnalyticsAdminV1betaListAccountsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_accounts_list_builder(
            &self.http_client,
            &args.pageSize,
            &args.pageToken,
            &args.showDeleted,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_accounts_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin accounts patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAnalyticsAdminV1betaAccount result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analyticsadmin_accounts_patch(
        &self,
        args: &AnalyticsadminAccountsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAnalyticsAdminV1betaAccount, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_accounts_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_accounts_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin accounts provision account ticket.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAnalyticsAdminV1betaProvisionAccountTicketResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analyticsadmin_accounts_provision_account_ticket(
        &self,
        args: &AnalyticsadminAccountsProvisionAccountTicketArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAnalyticsAdminV1betaProvisionAccountTicketResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_accounts_provision_account_ticket_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_accounts_provision_account_ticket_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin accounts run access report.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAnalyticsAdminV1betaRunAccessReportResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analyticsadmin_accounts_run_access_report(
        &self,
        args: &AnalyticsadminAccountsRunAccessReportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAnalyticsAdminV1betaRunAccessReportResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_accounts_run_access_report_builder(
            &self.http_client,
            &args.entity,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_accounts_run_access_report_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin accounts search change history events.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAnalyticsAdminV1betaSearchChangeHistoryEventsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn analyticsadmin_accounts_search_change_history_events(
        &self,
        args: &AnalyticsadminAccountsSearchChangeHistoryEventsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAnalyticsAdminV1betaSearchChangeHistoryEventsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_accounts_search_change_history_events_builder(
            &self.http_client,
            &args.account,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_accounts_search_change_history_events_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin properties acknowledge user data collection.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAnalyticsAdminV1betaAcknowledgeUserDataCollectionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analyticsadmin_properties_acknowledge_user_data_collection(
        &self,
        args: &AnalyticsadminPropertiesAcknowledgeUserDataCollectionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAnalyticsAdminV1betaAcknowledgeUserDataCollectionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_properties_acknowledge_user_data_collection_builder(
            &self.http_client,
            &args.property,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_properties_acknowledge_user_data_collection_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin properties create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAnalyticsAdminV1betaProperty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analyticsadmin_properties_create(
        &self,
        args: &AnalyticsadminPropertiesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAnalyticsAdminV1betaProperty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_properties_create_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_properties_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin properties delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAnalyticsAdminV1betaProperty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analyticsadmin_properties_delete(
        &self,
        args: &AnalyticsadminPropertiesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAnalyticsAdminV1betaProperty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_properties_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_properties_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin properties get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAnalyticsAdminV1betaProperty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn analyticsadmin_properties_get(
        &self,
        args: &AnalyticsadminPropertiesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAnalyticsAdminV1betaProperty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_properties_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_properties_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin properties get data retention settings.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAnalyticsAdminV1betaDataRetentionSettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn analyticsadmin_properties_get_data_retention_settings(
        &self,
        args: &AnalyticsadminPropertiesGetDataRetentionSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAnalyticsAdminV1betaDataRetentionSettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_properties_get_data_retention_settings_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_properties_get_data_retention_settings_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin properties list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAnalyticsAdminV1betaListPropertiesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn analyticsadmin_properties_list(
        &self,
        args: &AnalyticsadminPropertiesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAnalyticsAdminV1betaListPropertiesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_properties_list_builder(
            &self.http_client,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.showDeleted,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_properties_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin properties patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAnalyticsAdminV1betaProperty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analyticsadmin_properties_patch(
        &self,
        args: &AnalyticsadminPropertiesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAnalyticsAdminV1betaProperty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_properties_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_properties_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin properties run access report.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAnalyticsAdminV1betaRunAccessReportResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analyticsadmin_properties_run_access_report(
        &self,
        args: &AnalyticsadminPropertiesRunAccessReportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAnalyticsAdminV1betaRunAccessReportResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_properties_run_access_report_builder(
            &self.http_client,
            &args.entity,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_properties_run_access_report_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin properties update data retention settings.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAnalyticsAdminV1betaDataRetentionSettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analyticsadmin_properties_update_data_retention_settings(
        &self,
        args: &AnalyticsadminPropertiesUpdateDataRetentionSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAnalyticsAdminV1betaDataRetentionSettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_properties_update_data_retention_settings_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_properties_update_data_retention_settings_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin properties conversion events create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAnalyticsAdminV1betaConversionEvent result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analyticsadmin_properties_conversion_events_create(
        &self,
        args: &AnalyticsadminPropertiesConversionEventsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAnalyticsAdminV1betaConversionEvent, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_properties_conversion_events_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_properties_conversion_events_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin properties conversion events delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analyticsadmin_properties_conversion_events_delete(
        &self,
        args: &AnalyticsadminPropertiesConversionEventsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_properties_conversion_events_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_properties_conversion_events_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin properties conversion events get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAnalyticsAdminV1betaConversionEvent result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn analyticsadmin_properties_conversion_events_get(
        &self,
        args: &AnalyticsadminPropertiesConversionEventsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAnalyticsAdminV1betaConversionEvent, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_properties_conversion_events_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_properties_conversion_events_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin properties conversion events list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAnalyticsAdminV1betaListConversionEventsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn analyticsadmin_properties_conversion_events_list(
        &self,
        args: &AnalyticsadminPropertiesConversionEventsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAnalyticsAdminV1betaListConversionEventsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_properties_conversion_events_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_properties_conversion_events_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin properties conversion events patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAnalyticsAdminV1betaConversionEvent result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analyticsadmin_properties_conversion_events_patch(
        &self,
        args: &AnalyticsadminPropertiesConversionEventsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAnalyticsAdminV1betaConversionEvent, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_properties_conversion_events_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_properties_conversion_events_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin properties custom dimensions archive.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analyticsadmin_properties_custom_dimensions_archive(
        &self,
        args: &AnalyticsadminPropertiesCustomDimensionsArchiveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_properties_custom_dimensions_archive_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_properties_custom_dimensions_archive_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin properties custom dimensions create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAnalyticsAdminV1betaCustomDimension result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analyticsadmin_properties_custom_dimensions_create(
        &self,
        args: &AnalyticsadminPropertiesCustomDimensionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAnalyticsAdminV1betaCustomDimension, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_properties_custom_dimensions_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_properties_custom_dimensions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin properties custom dimensions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAnalyticsAdminV1betaCustomDimension result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn analyticsadmin_properties_custom_dimensions_get(
        &self,
        args: &AnalyticsadminPropertiesCustomDimensionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAnalyticsAdminV1betaCustomDimension, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_properties_custom_dimensions_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_properties_custom_dimensions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin properties custom dimensions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAnalyticsAdminV1betaListCustomDimensionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn analyticsadmin_properties_custom_dimensions_list(
        &self,
        args: &AnalyticsadminPropertiesCustomDimensionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAnalyticsAdminV1betaListCustomDimensionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_properties_custom_dimensions_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_properties_custom_dimensions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin properties custom dimensions patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAnalyticsAdminV1betaCustomDimension result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analyticsadmin_properties_custom_dimensions_patch(
        &self,
        args: &AnalyticsadminPropertiesCustomDimensionsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAnalyticsAdminV1betaCustomDimension, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_properties_custom_dimensions_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_properties_custom_dimensions_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin properties custom metrics archive.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analyticsadmin_properties_custom_metrics_archive(
        &self,
        args: &AnalyticsadminPropertiesCustomMetricsArchiveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_properties_custom_metrics_archive_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_properties_custom_metrics_archive_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin properties custom metrics create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAnalyticsAdminV1betaCustomMetric result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analyticsadmin_properties_custom_metrics_create(
        &self,
        args: &AnalyticsadminPropertiesCustomMetricsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAnalyticsAdminV1betaCustomMetric, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_properties_custom_metrics_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_properties_custom_metrics_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin properties custom metrics get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAnalyticsAdminV1betaCustomMetric result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn analyticsadmin_properties_custom_metrics_get(
        &self,
        args: &AnalyticsadminPropertiesCustomMetricsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAnalyticsAdminV1betaCustomMetric, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_properties_custom_metrics_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_properties_custom_metrics_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin properties custom metrics list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAnalyticsAdminV1betaListCustomMetricsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn analyticsadmin_properties_custom_metrics_list(
        &self,
        args: &AnalyticsadminPropertiesCustomMetricsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAnalyticsAdminV1betaListCustomMetricsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_properties_custom_metrics_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_properties_custom_metrics_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin properties custom metrics patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAnalyticsAdminV1betaCustomMetric result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analyticsadmin_properties_custom_metrics_patch(
        &self,
        args: &AnalyticsadminPropertiesCustomMetricsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAnalyticsAdminV1betaCustomMetric, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_properties_custom_metrics_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_properties_custom_metrics_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin properties data streams create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAnalyticsAdminV1betaDataStream result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analyticsadmin_properties_data_streams_create(
        &self,
        args: &AnalyticsadminPropertiesDataStreamsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAnalyticsAdminV1betaDataStream, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_properties_data_streams_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_properties_data_streams_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin properties data streams delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analyticsadmin_properties_data_streams_delete(
        &self,
        args: &AnalyticsadminPropertiesDataStreamsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_properties_data_streams_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_properties_data_streams_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin properties data streams get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAnalyticsAdminV1betaDataStream result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn analyticsadmin_properties_data_streams_get(
        &self,
        args: &AnalyticsadminPropertiesDataStreamsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAnalyticsAdminV1betaDataStream, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_properties_data_streams_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_properties_data_streams_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin properties data streams list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAnalyticsAdminV1betaListDataStreamsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn analyticsadmin_properties_data_streams_list(
        &self,
        args: &AnalyticsadminPropertiesDataStreamsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAnalyticsAdminV1betaListDataStreamsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_properties_data_streams_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_properties_data_streams_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin properties data streams patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAnalyticsAdminV1betaDataStream result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analyticsadmin_properties_data_streams_patch(
        &self,
        args: &AnalyticsadminPropertiesDataStreamsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAnalyticsAdminV1betaDataStream, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_properties_data_streams_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_properties_data_streams_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin properties data streams measurement protocol secrets create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAnalyticsAdminV1betaMeasurementProtocolSecret result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analyticsadmin_properties_data_streams_measurement_protocol_secrets_create(
        &self,
        args: &AnalyticsadminPropertiesDataStreamsMeasurementProtocolSecretsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAnalyticsAdminV1betaMeasurementProtocolSecret, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_properties_data_streams_measurement_protocol_secrets_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_properties_data_streams_measurement_protocol_secrets_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin properties data streams measurement protocol secrets delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analyticsadmin_properties_data_streams_measurement_protocol_secrets_delete(
        &self,
        args: &AnalyticsadminPropertiesDataStreamsMeasurementProtocolSecretsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_properties_data_streams_measurement_protocol_secrets_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_properties_data_streams_measurement_protocol_secrets_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin properties data streams measurement protocol secrets get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAnalyticsAdminV1betaMeasurementProtocolSecret result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn analyticsadmin_properties_data_streams_measurement_protocol_secrets_get(
        &self,
        args: &AnalyticsadminPropertiesDataStreamsMeasurementProtocolSecretsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAnalyticsAdminV1betaMeasurementProtocolSecret, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_properties_data_streams_measurement_protocol_secrets_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_properties_data_streams_measurement_protocol_secrets_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin properties data streams measurement protocol secrets list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAnalyticsAdminV1betaListMeasurementProtocolSecretsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn analyticsadmin_properties_data_streams_measurement_protocol_secrets_list(
        &self,
        args: &AnalyticsadminPropertiesDataStreamsMeasurementProtocolSecretsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAnalyticsAdminV1betaListMeasurementProtocolSecretsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_properties_data_streams_measurement_protocol_secrets_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_properties_data_streams_measurement_protocol_secrets_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin properties data streams measurement protocol secrets patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAnalyticsAdminV1betaMeasurementProtocolSecret result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analyticsadmin_properties_data_streams_measurement_protocol_secrets_patch(
        &self,
        args: &AnalyticsadminPropertiesDataStreamsMeasurementProtocolSecretsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAnalyticsAdminV1betaMeasurementProtocolSecret, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_properties_data_streams_measurement_protocol_secrets_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_properties_data_streams_measurement_protocol_secrets_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin properties firebase links create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAnalyticsAdminV1betaFirebaseLink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analyticsadmin_properties_firebase_links_create(
        &self,
        args: &AnalyticsadminPropertiesFirebaseLinksCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAnalyticsAdminV1betaFirebaseLink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_properties_firebase_links_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_properties_firebase_links_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin properties firebase links delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analyticsadmin_properties_firebase_links_delete(
        &self,
        args: &AnalyticsadminPropertiesFirebaseLinksDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_properties_firebase_links_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_properties_firebase_links_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin properties firebase links list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAnalyticsAdminV1betaListFirebaseLinksResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn analyticsadmin_properties_firebase_links_list(
        &self,
        args: &AnalyticsadminPropertiesFirebaseLinksListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAnalyticsAdminV1betaListFirebaseLinksResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_properties_firebase_links_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_properties_firebase_links_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin properties google ads links create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAnalyticsAdminV1betaGoogleAdsLink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analyticsadmin_properties_google_ads_links_create(
        &self,
        args: &AnalyticsadminPropertiesGoogleAdsLinksCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAnalyticsAdminV1betaGoogleAdsLink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_properties_google_ads_links_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_properties_google_ads_links_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin properties google ads links delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analyticsadmin_properties_google_ads_links_delete(
        &self,
        args: &AnalyticsadminPropertiesGoogleAdsLinksDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_properties_google_ads_links_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_properties_google_ads_links_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin properties google ads links list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAnalyticsAdminV1betaListGoogleAdsLinksResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn analyticsadmin_properties_google_ads_links_list(
        &self,
        args: &AnalyticsadminPropertiesGoogleAdsLinksListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAnalyticsAdminV1betaListGoogleAdsLinksResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_properties_google_ads_links_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_properties_google_ads_links_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin properties google ads links patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAnalyticsAdminV1betaGoogleAdsLink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analyticsadmin_properties_google_ads_links_patch(
        &self,
        args: &AnalyticsadminPropertiesGoogleAdsLinksPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAnalyticsAdminV1betaGoogleAdsLink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_properties_google_ads_links_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_properties_google_ads_links_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin properties key events create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAnalyticsAdminV1betaKeyEvent result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analyticsadmin_properties_key_events_create(
        &self,
        args: &AnalyticsadminPropertiesKeyEventsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAnalyticsAdminV1betaKeyEvent, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_properties_key_events_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_properties_key_events_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin properties key events delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analyticsadmin_properties_key_events_delete(
        &self,
        args: &AnalyticsadminPropertiesKeyEventsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_properties_key_events_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_properties_key_events_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin properties key events get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAnalyticsAdminV1betaKeyEvent result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn analyticsadmin_properties_key_events_get(
        &self,
        args: &AnalyticsadminPropertiesKeyEventsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAnalyticsAdminV1betaKeyEvent, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_properties_key_events_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_properties_key_events_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin properties key events list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAnalyticsAdminV1betaListKeyEventsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn analyticsadmin_properties_key_events_list(
        &self,
        args: &AnalyticsadminPropertiesKeyEventsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAnalyticsAdminV1betaListKeyEventsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_properties_key_events_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_properties_key_events_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsadmin properties key events patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAnalyticsAdminV1betaKeyEvent result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analyticsadmin_properties_key_events_patch(
        &self,
        args: &AnalyticsadminPropertiesKeyEventsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAnalyticsAdminV1betaKeyEvent, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsadmin_properties_key_events_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsadmin_properties_key_events_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
