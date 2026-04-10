//! ChromemanagementProvider - State-aware chromemanagement API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       chromemanagement API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::chromemanagement::{
    chromemanagement_customers_apps_count_chrome_app_requests_builder, chromemanagement_customers_apps_count_chrome_app_requests_task,
    chromemanagement_customers_apps_fetch_devices_requesting_extension_builder, chromemanagement_customers_apps_fetch_devices_requesting_extension_task,
    chromemanagement_customers_apps_fetch_users_requesting_extension_builder, chromemanagement_customers_apps_fetch_users_requesting_extension_task,
    chromemanagement_customers_apps_android_get_builder, chromemanagement_customers_apps_android_get_task,
    chromemanagement_customers_apps_chrome_get_builder, chromemanagement_customers_apps_chrome_get_task,
    chromemanagement_customers_apps_web_get_builder, chromemanagement_customers_apps_web_get_task,
    chromemanagement_customers_certificate_provisioning_processes_claim_builder, chromemanagement_customers_certificate_provisioning_processes_claim_task,
    chromemanagement_customers_certificate_provisioning_processes_get_builder, chromemanagement_customers_certificate_provisioning_processes_get_task,
    chromemanagement_customers_certificate_provisioning_processes_set_failure_builder, chromemanagement_customers_certificate_provisioning_processes_set_failure_task,
    chromemanagement_customers_certificate_provisioning_processes_sign_data_builder, chromemanagement_customers_certificate_provisioning_processes_sign_data_task,
    chromemanagement_customers_certificate_provisioning_processes_upload_certificate_builder, chromemanagement_customers_certificate_provisioning_processes_upload_certificate_task,
    chromemanagement_customers_certificate_provisioning_processes_operations_get_builder, chromemanagement_customers_certificate_provisioning_processes_operations_get_task,
    chromemanagement_customers_profiles_delete_builder, chromemanagement_customers_profiles_delete_task,
    chromemanagement_customers_profiles_get_builder, chromemanagement_customers_profiles_get_task,
    chromemanagement_customers_profiles_list_builder, chromemanagement_customers_profiles_list_task,
    chromemanagement_customers_profiles_commands_create_builder, chromemanagement_customers_profiles_commands_create_task,
    chromemanagement_customers_profiles_commands_get_builder, chromemanagement_customers_profiles_commands_get_task,
    chromemanagement_customers_profiles_commands_list_builder, chromemanagement_customers_profiles_commands_list_task,
    chromemanagement_customers_reports_count_active_devices_builder, chromemanagement_customers_reports_count_active_devices_task,
    chromemanagement_customers_reports_count_chrome_browsers_needing_attention_builder, chromemanagement_customers_reports_count_chrome_browsers_needing_attention_task,
    chromemanagement_customers_reports_count_chrome_crash_events_builder, chromemanagement_customers_reports_count_chrome_crash_events_task,
    chromemanagement_customers_reports_count_chrome_devices_reaching_auto_expiration_date_builder, chromemanagement_customers_reports_count_chrome_devices_reaching_auto_expiration_date_task,
    chromemanagement_customers_reports_count_chrome_devices_that_need_attention_builder, chromemanagement_customers_reports_count_chrome_devices_that_need_attention_task,
    chromemanagement_customers_reports_count_chrome_hardware_fleet_devices_builder, chromemanagement_customers_reports_count_chrome_hardware_fleet_devices_task,
    chromemanagement_customers_reports_count_chrome_versions_builder, chromemanagement_customers_reports_count_chrome_versions_task,
    chromemanagement_customers_reports_count_devices_per_boot_type_builder, chromemanagement_customers_reports_count_devices_per_boot_type_task,
    chromemanagement_customers_reports_count_devices_per_release_channel_builder, chromemanagement_customers_reports_count_devices_per_release_channel_task,
    chromemanagement_customers_reports_count_installed_apps_builder, chromemanagement_customers_reports_count_installed_apps_task,
    chromemanagement_customers_reports_count_print_jobs_by_printer_builder, chromemanagement_customers_reports_count_print_jobs_by_printer_task,
    chromemanagement_customers_reports_count_print_jobs_by_user_builder, chromemanagement_customers_reports_count_print_jobs_by_user_task,
    chromemanagement_customers_reports_enumerate_print_jobs_builder, chromemanagement_customers_reports_enumerate_print_jobs_task,
    chromemanagement_customers_reports_find_installed_app_devices_builder, chromemanagement_customers_reports_find_installed_app_devices_task,
    chromemanagement_customers_telemetry_devices_get_builder, chromemanagement_customers_telemetry_devices_get_task,
    chromemanagement_customers_telemetry_devices_list_builder, chromemanagement_customers_telemetry_devices_list_task,
    chromemanagement_customers_telemetry_events_list_builder, chromemanagement_customers_telemetry_events_list_task,
    chromemanagement_customers_telemetry_notification_configs_create_builder, chromemanagement_customers_telemetry_notification_configs_create_task,
    chromemanagement_customers_telemetry_notification_configs_delete_builder, chromemanagement_customers_telemetry_notification_configs_delete_task,
    chromemanagement_customers_telemetry_notification_configs_list_builder, chromemanagement_customers_telemetry_notification_configs_list_task,
    chromemanagement_customers_telemetry_users_get_builder, chromemanagement_customers_telemetry_users_get_task,
    chromemanagement_customers_telemetry_users_list_builder, chromemanagement_customers_telemetry_users_list_task,
    chromemanagement_customers_third_party_profile_users_move_builder, chromemanagement_customers_third_party_profile_users_move_task,
    chromemanagement_operations_cancel_builder, chromemanagement_operations_cancel_task,
    chromemanagement_operations_delete_builder, chromemanagement_operations_delete_task,
    chromemanagement_operations_list_builder, chromemanagement_operations_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::chromemanagement::GoogleChromeManagementV1AppDetails;
use crate::providers::gcp::clients::chromemanagement::GoogleChromeManagementV1CountActiveDevicesResponse;
use crate::providers::gcp::clients::chromemanagement::GoogleChromeManagementV1CountChromeAppRequestsResponse;
use crate::providers::gcp::clients::chromemanagement::GoogleChromeManagementV1CountChromeBrowsersNeedingAttentionResponse;
use crate::providers::gcp::clients::chromemanagement::GoogleChromeManagementV1CountChromeCrashEventsResponse;
use crate::providers::gcp::clients::chromemanagement::GoogleChromeManagementV1CountChromeDevicesReachingAutoExpirationDateResponse;
use crate::providers::gcp::clients::chromemanagement::GoogleChromeManagementV1CountChromeDevicesThatNeedAttentionResponse;
use crate::providers::gcp::clients::chromemanagement::GoogleChromeManagementV1CountChromeHardwareFleetDevicesResponse;
use crate::providers::gcp::clients::chromemanagement::GoogleChromeManagementV1CountChromeVersionsResponse;
use crate::providers::gcp::clients::chromemanagement::GoogleChromeManagementV1CountDevicesPerBootTypeResponse;
use crate::providers::gcp::clients::chromemanagement::GoogleChromeManagementV1CountDevicesPerReleaseChannelResponse;
use crate::providers::gcp::clients::chromemanagement::GoogleChromeManagementV1CountInstalledAppsResponse;
use crate::providers::gcp::clients::chromemanagement::GoogleChromeManagementV1CountPrintJobsByPrinterResponse;
use crate::providers::gcp::clients::chromemanagement::GoogleChromeManagementV1CountPrintJobsByUserResponse;
use crate::providers::gcp::clients::chromemanagement::GoogleChromeManagementV1EnumeratePrintJobsResponse;
use crate::providers::gcp::clients::chromemanagement::GoogleChromeManagementV1FetchDevicesRequestingExtensionResponse;
use crate::providers::gcp::clients::chromemanagement::GoogleChromeManagementV1FetchUsersRequestingExtensionResponse;
use crate::providers::gcp::clients::chromemanagement::GoogleChromeManagementV1FindInstalledAppDevicesResponse;
use crate::providers::gcp::clients::chromemanagement::GoogleChromeManagementV1ListTelemetryDevicesResponse;
use crate::providers::gcp::clients::chromemanagement::GoogleChromeManagementV1ListTelemetryEventsResponse;
use crate::providers::gcp::clients::chromemanagement::GoogleChromeManagementV1ListTelemetryNotificationConfigsResponse;
use crate::providers::gcp::clients::chromemanagement::GoogleChromeManagementV1ListTelemetryUsersResponse;
use crate::providers::gcp::clients::chromemanagement::GoogleChromeManagementV1TelemetryDevice;
use crate::providers::gcp::clients::chromemanagement::GoogleChromeManagementV1TelemetryNotificationConfig;
use crate::providers::gcp::clients::chromemanagement::GoogleChromeManagementV1TelemetryUser;
use crate::providers::gcp::clients::chromemanagement::GoogleChromeManagementVersionsV1CertificateProvisioningProcess;
use crate::providers::gcp::clients::chromemanagement::GoogleChromeManagementVersionsV1ChromeBrowserProfile;
use crate::providers::gcp::clients::chromemanagement::GoogleChromeManagementVersionsV1ChromeBrowserProfileCommand;
use crate::providers::gcp::clients::chromemanagement::GoogleChromeManagementVersionsV1ClaimCertificateProvisioningProcessResponse;
use crate::providers::gcp::clients::chromemanagement::GoogleChromeManagementVersionsV1ListChromeBrowserProfileCommandsResponse;
use crate::providers::gcp::clients::chromemanagement::GoogleChromeManagementVersionsV1ListChromeBrowserProfilesResponse;
use crate::providers::gcp::clients::chromemanagement::GoogleChromeManagementVersionsV1MoveThirdPartyProfileUserResponse;
use crate::providers::gcp::clients::chromemanagement::GoogleChromeManagementVersionsV1SetFailureResponse;
use crate::providers::gcp::clients::chromemanagement::GoogleChromeManagementVersionsV1UploadCertificateResponse;
use crate::providers::gcp::clients::chromemanagement::GoogleLongrunningListOperationsResponse;
use crate::providers::gcp::clients::chromemanagement::GoogleLongrunningOperation;
use crate::providers::gcp::clients::chromemanagement::GoogleProtobufEmpty;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersAppsAndroidGetArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersAppsChromeGetArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersAppsCountChromeAppRequestsArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersAppsFetchDevicesRequestingExtensionArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersAppsFetchUsersRequestingExtensionArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersAppsWebGetArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersCertificateProvisioningProcessesClaimArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersCertificateProvisioningProcessesGetArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersCertificateProvisioningProcessesOperationsGetArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersCertificateProvisioningProcessesSetFailureArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersCertificateProvisioningProcessesSignDataArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersCertificateProvisioningProcessesUploadCertificateArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersProfilesCommandsCreateArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersProfilesCommandsGetArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersProfilesCommandsListArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersProfilesDeleteArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersProfilesGetArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersProfilesListArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersReportsCountActiveDevicesArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersReportsCountChromeBrowsersNeedingAttentionArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersReportsCountChromeCrashEventsArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersReportsCountChromeDevicesReachingAutoExpirationDateArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersReportsCountChromeDevicesThatNeedAttentionArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersReportsCountChromeHardwareFleetDevicesArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersReportsCountChromeVersionsArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersReportsCountDevicesPerBootTypeArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersReportsCountDevicesPerReleaseChannelArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersReportsCountInstalledAppsArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersReportsCountPrintJobsByPrinterArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersReportsCountPrintJobsByUserArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersReportsEnumeratePrintJobsArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersReportsFindInstalledAppDevicesArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersTelemetryDevicesGetArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersTelemetryDevicesListArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersTelemetryEventsListArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersTelemetryNotificationConfigsCreateArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersTelemetryNotificationConfigsDeleteArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersTelemetryNotificationConfigsListArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersTelemetryUsersGetArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersTelemetryUsersListArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersThirdPartyProfileUsersMoveArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementOperationsCancelArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementOperationsDeleteArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementOperationsListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// ChromemanagementProvider with automatic state tracking.
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
/// let provider = ChromemanagementProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct ChromemanagementProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> ChromemanagementProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new ChromemanagementProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Chromemanagement customers apps count chrome app requests.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromeManagementV1CountChromeAppRequestsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chromemanagement_customers_apps_count_chrome_app_requests(
        &self,
        args: &ChromemanagementCustomersAppsCountChromeAppRequestsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromeManagementV1CountChromeAppRequestsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_apps_count_chrome_app_requests_builder(
            &self.http_client,
            &args.customer,
            &args.orderBy,
            &args.orgUnitId,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_apps_count_chrome_app_requests_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement customers apps fetch devices requesting extension.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromeManagementV1FetchDevicesRequestingExtensionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chromemanagement_customers_apps_fetch_devices_requesting_extension(
        &self,
        args: &ChromemanagementCustomersAppsFetchDevicesRequestingExtensionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromeManagementV1FetchDevicesRequestingExtensionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_apps_fetch_devices_requesting_extension_builder(
            &self.http_client,
            &args.customer,
            &args.extensionId,
            &args.orgUnitId,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_apps_fetch_devices_requesting_extension_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement customers apps fetch users requesting extension.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromeManagementV1FetchUsersRequestingExtensionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chromemanagement_customers_apps_fetch_users_requesting_extension(
        &self,
        args: &ChromemanagementCustomersAppsFetchUsersRequestingExtensionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromeManagementV1FetchUsersRequestingExtensionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_apps_fetch_users_requesting_extension_builder(
            &self.http_client,
            &args.customer,
            &args.extensionId,
            &args.orgUnitId,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_apps_fetch_users_requesting_extension_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement customers apps android get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromeManagementV1AppDetails result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chromemanagement_customers_apps_android_get(
        &self,
        args: &ChromemanagementCustomersAppsAndroidGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromeManagementV1AppDetails, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_apps_android_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_apps_android_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement customers apps chrome get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromeManagementV1AppDetails result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chromemanagement_customers_apps_chrome_get(
        &self,
        args: &ChromemanagementCustomersAppsChromeGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromeManagementV1AppDetails, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_apps_chrome_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_apps_chrome_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement customers apps web get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromeManagementV1AppDetails result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chromemanagement_customers_apps_web_get(
        &self,
        args: &ChromemanagementCustomersAppsWebGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromeManagementV1AppDetails, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_apps_web_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_apps_web_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement customers certificate provisioning processes claim.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromeManagementVersionsV1ClaimCertificateProvisioningProcessResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chromemanagement_customers_certificate_provisioning_processes_claim(
        &self,
        args: &ChromemanagementCustomersCertificateProvisioningProcessesClaimArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromeManagementVersionsV1ClaimCertificateProvisioningProcessResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_certificate_provisioning_processes_claim_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_certificate_provisioning_processes_claim_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement customers certificate provisioning processes get.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromeManagementVersionsV1CertificateProvisioningProcess result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chromemanagement_customers_certificate_provisioning_processes_get(
        &self,
        args: &ChromemanagementCustomersCertificateProvisioningProcessesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromeManagementVersionsV1CertificateProvisioningProcess, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_certificate_provisioning_processes_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_certificate_provisioning_processes_get_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement customers certificate provisioning processes set failure.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromeManagementVersionsV1SetFailureResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chromemanagement_customers_certificate_provisioning_processes_set_failure(
        &self,
        args: &ChromemanagementCustomersCertificateProvisioningProcessesSetFailureArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromeManagementVersionsV1SetFailureResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_certificate_provisioning_processes_set_failure_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_certificate_provisioning_processes_set_failure_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement customers certificate provisioning processes sign data.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chromemanagement_customers_certificate_provisioning_processes_sign_data(
        &self,
        args: &ChromemanagementCustomersCertificateProvisioningProcessesSignDataArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_certificate_provisioning_processes_sign_data_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_certificate_provisioning_processes_sign_data_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement customers certificate provisioning processes upload certificate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromeManagementVersionsV1UploadCertificateResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chromemanagement_customers_certificate_provisioning_processes_upload_certificate(
        &self,
        args: &ChromemanagementCustomersCertificateProvisioningProcessesUploadCertificateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromeManagementVersionsV1UploadCertificateResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_certificate_provisioning_processes_upload_certificate_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_certificate_provisioning_processes_upload_certificate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement customers certificate provisioning processes operations get.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chromemanagement_customers_certificate_provisioning_processes_operations_get(
        &self,
        args: &ChromemanagementCustomersCertificateProvisioningProcessesOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_certificate_provisioning_processes_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_certificate_provisioning_processes_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement customers profiles delete.
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
    pub fn chromemanagement_customers_profiles_delete(
        &self,
        args: &ChromemanagementCustomersProfilesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_profiles_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_profiles_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement customers profiles get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromeManagementVersionsV1ChromeBrowserProfile result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chromemanagement_customers_profiles_get(
        &self,
        args: &ChromemanagementCustomersProfilesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromeManagementVersionsV1ChromeBrowserProfile, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_profiles_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_profiles_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement customers profiles list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromeManagementVersionsV1ListChromeBrowserProfilesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chromemanagement_customers_profiles_list(
        &self,
        args: &ChromemanagementCustomersProfilesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromeManagementVersionsV1ListChromeBrowserProfilesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_profiles_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_profiles_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement customers profiles commands create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromeManagementVersionsV1ChromeBrowserProfileCommand result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chromemanagement_customers_profiles_commands_create(
        &self,
        args: &ChromemanagementCustomersProfilesCommandsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromeManagementVersionsV1ChromeBrowserProfileCommand, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_profiles_commands_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_profiles_commands_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement customers profiles commands get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromeManagementVersionsV1ChromeBrowserProfileCommand result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chromemanagement_customers_profiles_commands_get(
        &self,
        args: &ChromemanagementCustomersProfilesCommandsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromeManagementVersionsV1ChromeBrowserProfileCommand, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_profiles_commands_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_profiles_commands_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement customers profiles commands list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromeManagementVersionsV1ListChromeBrowserProfileCommandsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chromemanagement_customers_profiles_commands_list(
        &self,
        args: &ChromemanagementCustomersProfilesCommandsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromeManagementVersionsV1ListChromeBrowserProfileCommandsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_profiles_commands_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_profiles_commands_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement customers reports count active devices.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromeManagementV1CountActiveDevicesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chromemanagement_customers_reports_count_active_devices(
        &self,
        args: &ChromemanagementCustomersReportsCountActiveDevicesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromeManagementV1CountActiveDevicesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_reports_count_active_devices_builder(
            &self.http_client,
            &args.customer,
            &args.date.day,
            &args.date.month,
            &args.date.year,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_reports_count_active_devices_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement customers reports count chrome browsers needing attention.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromeManagementV1CountChromeBrowsersNeedingAttentionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chromemanagement_customers_reports_count_chrome_browsers_needing_attention(
        &self,
        args: &ChromemanagementCustomersReportsCountChromeBrowsersNeedingAttentionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromeManagementV1CountChromeBrowsersNeedingAttentionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_reports_count_chrome_browsers_needing_attention_builder(
            &self.http_client,
            &args.customer,
            &args.orgUnitId,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_reports_count_chrome_browsers_needing_attention_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement customers reports count chrome crash events.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromeManagementV1CountChromeCrashEventsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chromemanagement_customers_reports_count_chrome_crash_events(
        &self,
        args: &ChromemanagementCustomersReportsCountChromeCrashEventsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromeManagementV1CountChromeCrashEventsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_reports_count_chrome_crash_events_builder(
            &self.http_client,
            &args.customer,
            &args.filter,
            &args.orderBy,
            &args.orgUnitId,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_reports_count_chrome_crash_events_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement customers reports count chrome devices reaching auto expiration date.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromeManagementV1CountChromeDevicesReachingAutoExpirationDateResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chromemanagement_customers_reports_count_chrome_devices_reaching_auto_expiration_date(
        &self,
        args: &ChromemanagementCustomersReportsCountChromeDevicesReachingAutoExpirationDateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromeManagementV1CountChromeDevicesReachingAutoExpirationDateResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_reports_count_chrome_devices_reaching_auto_expiration_date_builder(
            &self.http_client,
            &args.customer,
            &args.maxAueDate,
            &args.minAueDate,
            &args.orgUnitId,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_reports_count_chrome_devices_reaching_auto_expiration_date_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement customers reports count chrome devices that need attention.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromeManagementV1CountChromeDevicesThatNeedAttentionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chromemanagement_customers_reports_count_chrome_devices_that_need_attention(
        &self,
        args: &ChromemanagementCustomersReportsCountChromeDevicesThatNeedAttentionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromeManagementV1CountChromeDevicesThatNeedAttentionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_reports_count_chrome_devices_that_need_attention_builder(
            &self.http_client,
            &args.customer,
            &args.orgUnitId,
            &args.readMask,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_reports_count_chrome_devices_that_need_attention_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement customers reports count chrome hardware fleet devices.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromeManagementV1CountChromeHardwareFleetDevicesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chromemanagement_customers_reports_count_chrome_hardware_fleet_devices(
        &self,
        args: &ChromemanagementCustomersReportsCountChromeHardwareFleetDevicesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromeManagementV1CountChromeHardwareFleetDevicesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_reports_count_chrome_hardware_fleet_devices_builder(
            &self.http_client,
            &args.customer,
            &args.orgUnitId,
            &args.readMask,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_reports_count_chrome_hardware_fleet_devices_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement customers reports count chrome versions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromeManagementV1CountChromeVersionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chromemanagement_customers_reports_count_chrome_versions(
        &self,
        args: &ChromemanagementCustomersReportsCountChromeVersionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromeManagementV1CountChromeVersionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_reports_count_chrome_versions_builder(
            &self.http_client,
            &args.customer,
            &args.filter,
            &args.orgUnitId,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_reports_count_chrome_versions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement customers reports count devices per boot type.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromeManagementV1CountDevicesPerBootTypeResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chromemanagement_customers_reports_count_devices_per_boot_type(
        &self,
        args: &ChromemanagementCustomersReportsCountDevicesPerBootTypeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromeManagementV1CountDevicesPerBootTypeResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_reports_count_devices_per_boot_type_builder(
            &self.http_client,
            &args.customer,
            &args.date.day,
            &args.date.month,
            &args.date.year,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_reports_count_devices_per_boot_type_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement customers reports count devices per release channel.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromeManagementV1CountDevicesPerReleaseChannelResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chromemanagement_customers_reports_count_devices_per_release_channel(
        &self,
        args: &ChromemanagementCustomersReportsCountDevicesPerReleaseChannelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromeManagementV1CountDevicesPerReleaseChannelResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_reports_count_devices_per_release_channel_builder(
            &self.http_client,
            &args.customer,
            &args.date.day,
            &args.date.month,
            &args.date.year,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_reports_count_devices_per_release_channel_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement customers reports count installed apps.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromeManagementV1CountInstalledAppsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chromemanagement_customers_reports_count_installed_apps(
        &self,
        args: &ChromemanagementCustomersReportsCountInstalledAppsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromeManagementV1CountInstalledAppsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_reports_count_installed_apps_builder(
            &self.http_client,
            &args.customer,
            &args.filter,
            &args.orderBy,
            &args.orgUnitId,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_reports_count_installed_apps_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement customers reports count print jobs by printer.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromeManagementV1CountPrintJobsByPrinterResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chromemanagement_customers_reports_count_print_jobs_by_printer(
        &self,
        args: &ChromemanagementCustomersReportsCountPrintJobsByPrinterArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromeManagementV1CountPrintJobsByPrinterResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_reports_count_print_jobs_by_printer_builder(
            &self.http_client,
            &args.customer,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.printerOrgUnitId,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_reports_count_print_jobs_by_printer_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement customers reports count print jobs by user.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromeManagementV1CountPrintJobsByUserResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chromemanagement_customers_reports_count_print_jobs_by_user(
        &self,
        args: &ChromemanagementCustomersReportsCountPrintJobsByUserArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromeManagementV1CountPrintJobsByUserResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_reports_count_print_jobs_by_user_builder(
            &self.http_client,
            &args.customer,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.printerOrgUnitId,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_reports_count_print_jobs_by_user_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement customers reports enumerate print jobs.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromeManagementV1EnumeratePrintJobsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chromemanagement_customers_reports_enumerate_print_jobs(
        &self,
        args: &ChromemanagementCustomersReportsEnumeratePrintJobsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromeManagementV1EnumeratePrintJobsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_reports_enumerate_print_jobs_builder(
            &self.http_client,
            &args.customer,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.printerOrgUnitId,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_reports_enumerate_print_jobs_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement customers reports find installed app devices.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromeManagementV1FindInstalledAppDevicesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chromemanagement_customers_reports_find_installed_app_devices(
        &self,
        args: &ChromemanagementCustomersReportsFindInstalledAppDevicesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromeManagementV1FindInstalledAppDevicesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_reports_find_installed_app_devices_builder(
            &self.http_client,
            &args.customer,
            &args.appId,
            &args.appType,
            &args.filter,
            &args.orderBy,
            &args.orgUnitId,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_reports_find_installed_app_devices_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement customers telemetry devices get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromeManagementV1TelemetryDevice result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chromemanagement_customers_telemetry_devices_get(
        &self,
        args: &ChromemanagementCustomersTelemetryDevicesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromeManagementV1TelemetryDevice, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_telemetry_devices_get_builder(
            &self.http_client,
            &args.name,
            &args.readMask,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_telemetry_devices_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement customers telemetry devices list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromeManagementV1ListTelemetryDevicesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chromemanagement_customers_telemetry_devices_list(
        &self,
        args: &ChromemanagementCustomersTelemetryDevicesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromeManagementV1ListTelemetryDevicesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_telemetry_devices_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.readMask,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_telemetry_devices_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement customers telemetry events list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromeManagementV1ListTelemetryEventsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chromemanagement_customers_telemetry_events_list(
        &self,
        args: &ChromemanagementCustomersTelemetryEventsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromeManagementV1ListTelemetryEventsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_telemetry_events_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.readMask,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_telemetry_events_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement customers telemetry notification configs create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromeManagementV1TelemetryNotificationConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chromemanagement_customers_telemetry_notification_configs_create(
        &self,
        args: &ChromemanagementCustomersTelemetryNotificationConfigsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromeManagementV1TelemetryNotificationConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_telemetry_notification_configs_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_telemetry_notification_configs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement customers telemetry notification configs delete.
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
    pub fn chromemanagement_customers_telemetry_notification_configs_delete(
        &self,
        args: &ChromemanagementCustomersTelemetryNotificationConfigsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_telemetry_notification_configs_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_telemetry_notification_configs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement customers telemetry notification configs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromeManagementV1ListTelemetryNotificationConfigsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chromemanagement_customers_telemetry_notification_configs_list(
        &self,
        args: &ChromemanagementCustomersTelemetryNotificationConfigsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromeManagementV1ListTelemetryNotificationConfigsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_telemetry_notification_configs_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_telemetry_notification_configs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement customers telemetry users get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromeManagementV1TelemetryUser result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chromemanagement_customers_telemetry_users_get(
        &self,
        args: &ChromemanagementCustomersTelemetryUsersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromeManagementV1TelemetryUser, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_telemetry_users_get_builder(
            &self.http_client,
            &args.name,
            &args.readMask,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_telemetry_users_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement customers telemetry users list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromeManagementV1ListTelemetryUsersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chromemanagement_customers_telemetry_users_list(
        &self,
        args: &ChromemanagementCustomersTelemetryUsersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromeManagementV1ListTelemetryUsersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_telemetry_users_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.readMask,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_telemetry_users_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement customers third party profile users move.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromeManagementVersionsV1MoveThirdPartyProfileUserResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chromemanagement_customers_third_party_profile_users_move(
        &self,
        args: &ChromemanagementCustomersThirdPartyProfileUsersMoveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromeManagementVersionsV1MoveThirdPartyProfileUserResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_third_party_profile_users_move_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_third_party_profile_users_move_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement operations cancel.
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
    pub fn chromemanagement_operations_cancel(
        &self,
        args: &ChromemanagementOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement operations delete.
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
    pub fn chromemanagement_operations_delete(
        &self,
        args: &ChromemanagementOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement operations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningListOperationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chromemanagement_operations_list(
        &self,
        args: &ChromemanagementOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_operations_list_builder(
            &self.http_client,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
