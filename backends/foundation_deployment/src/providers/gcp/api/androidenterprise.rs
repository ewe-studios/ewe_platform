//! AndroidenterpriseProvider - State-aware androidenterprise API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       androidenterprise API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::androidenterprise::{
    androidenterprise_devices_force_report_upload_builder, androidenterprise_devices_force_report_upload_task,
    androidenterprise_devices_get_builder, androidenterprise_devices_get_task,
    androidenterprise_devices_get_state_builder, androidenterprise_devices_get_state_task,
    androidenterprise_devices_list_builder, androidenterprise_devices_list_task,
    androidenterprise_devices_set_state_builder, androidenterprise_devices_set_state_task,
    androidenterprise_devices_update_builder, androidenterprise_devices_update_task,
    androidenterprise_enrollment_tokens_create_builder, androidenterprise_enrollment_tokens_create_task,
    androidenterprise_enterprises_acknowledge_notification_set_builder, androidenterprise_enterprises_acknowledge_notification_set_task,
    androidenterprise_enterprises_complete_signup_builder, androidenterprise_enterprises_complete_signup_task,
    androidenterprise_enterprises_create_web_token_builder, androidenterprise_enterprises_create_web_token_task,
    androidenterprise_enterprises_enroll_builder, androidenterprise_enterprises_enroll_task,
    androidenterprise_enterprises_generate_enterprise_upgrade_url_builder, androidenterprise_enterprises_generate_enterprise_upgrade_url_task,
    androidenterprise_enterprises_generate_signup_url_builder, androidenterprise_enterprises_generate_signup_url_task,
    androidenterprise_enterprises_get_builder, androidenterprise_enterprises_get_task,
    androidenterprise_enterprises_get_service_account_builder, androidenterprise_enterprises_get_service_account_task,
    androidenterprise_enterprises_get_store_layout_builder, androidenterprise_enterprises_get_store_layout_task,
    androidenterprise_enterprises_list_builder, androidenterprise_enterprises_list_task,
    androidenterprise_enterprises_pull_notification_set_builder, androidenterprise_enterprises_pull_notification_set_task,
    androidenterprise_enterprises_send_test_push_notification_builder, androidenterprise_enterprises_send_test_push_notification_task,
    androidenterprise_enterprises_set_account_builder, androidenterprise_enterprises_set_account_task,
    androidenterprise_enterprises_set_store_layout_builder, androidenterprise_enterprises_set_store_layout_task,
    androidenterprise_enterprises_unenroll_builder, androidenterprise_enterprises_unenroll_task,
    androidenterprise_entitlements_delete_builder, androidenterprise_entitlements_delete_task,
    androidenterprise_entitlements_get_builder, androidenterprise_entitlements_get_task,
    androidenterprise_entitlements_list_builder, androidenterprise_entitlements_list_task,
    androidenterprise_entitlements_update_builder, androidenterprise_entitlements_update_task,
    androidenterprise_grouplicenses_get_builder, androidenterprise_grouplicenses_get_task,
    androidenterprise_grouplicenses_list_builder, androidenterprise_grouplicenses_list_task,
    androidenterprise_grouplicenseusers_list_builder, androidenterprise_grouplicenseusers_list_task,
    androidenterprise_installs_delete_builder, androidenterprise_installs_delete_task,
    androidenterprise_installs_get_builder, androidenterprise_installs_get_task,
    androidenterprise_installs_list_builder, androidenterprise_installs_list_task,
    androidenterprise_installs_update_builder, androidenterprise_installs_update_task,
    androidenterprise_managedconfigurationsfordevice_delete_builder, androidenterprise_managedconfigurationsfordevice_delete_task,
    androidenterprise_managedconfigurationsfordevice_get_builder, androidenterprise_managedconfigurationsfordevice_get_task,
    androidenterprise_managedconfigurationsfordevice_list_builder, androidenterprise_managedconfigurationsfordevice_list_task,
    androidenterprise_managedconfigurationsfordevice_update_builder, androidenterprise_managedconfigurationsfordevice_update_task,
    androidenterprise_managedconfigurationsforuser_delete_builder, androidenterprise_managedconfigurationsforuser_delete_task,
    androidenterprise_managedconfigurationsforuser_get_builder, androidenterprise_managedconfigurationsforuser_get_task,
    androidenterprise_managedconfigurationsforuser_list_builder, androidenterprise_managedconfigurationsforuser_list_task,
    androidenterprise_managedconfigurationsforuser_update_builder, androidenterprise_managedconfigurationsforuser_update_task,
    androidenterprise_managedconfigurationssettings_list_builder, androidenterprise_managedconfigurationssettings_list_task,
    androidenterprise_permissions_get_builder, androidenterprise_permissions_get_task,
    androidenterprise_products_approve_builder, androidenterprise_products_approve_task,
    androidenterprise_products_generate_approval_url_builder, androidenterprise_products_generate_approval_url_task,
    androidenterprise_products_get_builder, androidenterprise_products_get_task,
    androidenterprise_products_get_app_restrictions_schema_builder, androidenterprise_products_get_app_restrictions_schema_task,
    androidenterprise_products_get_permissions_builder, androidenterprise_products_get_permissions_task,
    androidenterprise_products_list_builder, androidenterprise_products_list_task,
    androidenterprise_products_unapprove_builder, androidenterprise_products_unapprove_task,
    androidenterprise_serviceaccountkeys_delete_builder, androidenterprise_serviceaccountkeys_delete_task,
    androidenterprise_serviceaccountkeys_insert_builder, androidenterprise_serviceaccountkeys_insert_task,
    androidenterprise_serviceaccountkeys_list_builder, androidenterprise_serviceaccountkeys_list_task,
    androidenterprise_storelayoutclusters_delete_builder, androidenterprise_storelayoutclusters_delete_task,
    androidenterprise_storelayoutclusters_get_builder, androidenterprise_storelayoutclusters_get_task,
    androidenterprise_storelayoutclusters_insert_builder, androidenterprise_storelayoutclusters_insert_task,
    androidenterprise_storelayoutclusters_list_builder, androidenterprise_storelayoutclusters_list_task,
    androidenterprise_storelayoutclusters_update_builder, androidenterprise_storelayoutclusters_update_task,
    androidenterprise_storelayoutpages_delete_builder, androidenterprise_storelayoutpages_delete_task,
    androidenterprise_storelayoutpages_get_builder, androidenterprise_storelayoutpages_get_task,
    androidenterprise_storelayoutpages_insert_builder, androidenterprise_storelayoutpages_insert_task,
    androidenterprise_storelayoutpages_list_builder, androidenterprise_storelayoutpages_list_task,
    androidenterprise_storelayoutpages_update_builder, androidenterprise_storelayoutpages_update_task,
    androidenterprise_users_delete_builder, androidenterprise_users_delete_task,
    androidenterprise_users_generate_authentication_token_builder, androidenterprise_users_generate_authentication_token_task,
    androidenterprise_users_get_builder, androidenterprise_users_get_task,
    androidenterprise_users_get_available_product_set_builder, androidenterprise_users_get_available_product_set_task,
    androidenterprise_users_insert_builder, androidenterprise_users_insert_task,
    androidenterprise_users_list_builder, androidenterprise_users_list_task,
    androidenterprise_users_revoke_device_access_builder, androidenterprise_users_revoke_device_access_task,
    androidenterprise_users_set_available_product_set_builder, androidenterprise_users_set_available_product_set_task,
    androidenterprise_users_update_builder, androidenterprise_users_update_task,
    androidenterprise_webapps_delete_builder, androidenterprise_webapps_delete_task,
    androidenterprise_webapps_get_builder, androidenterprise_webapps_get_task,
    androidenterprise_webapps_insert_builder, androidenterprise_webapps_insert_task,
    androidenterprise_webapps_list_builder, androidenterprise_webapps_list_task,
    androidenterprise_webapps_update_builder, androidenterprise_webapps_update_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::androidenterprise::AdministratorWebToken;
use crate::providers::gcp::clients::androidenterprise::AppRestrictionsSchema;
use crate::providers::gcp::clients::androidenterprise::AuthenticationToken;
use crate::providers::gcp::clients::androidenterprise::Device;
use crate::providers::gcp::clients::androidenterprise::DeviceState;
use crate::providers::gcp::clients::androidenterprise::DevicesListResponse;
use crate::providers::gcp::clients::androidenterprise::EnrollmentToken;
use crate::providers::gcp::clients::androidenterprise::Enterprise;
use crate::providers::gcp::clients::androidenterprise::EnterpriseAccount;
use crate::providers::gcp::clients::androidenterprise::EnterprisesListResponse;
use crate::providers::gcp::clients::androidenterprise::EnterprisesSendTestPushNotificationResponse;
use crate::providers::gcp::clients::androidenterprise::Entitlement;
use crate::providers::gcp::clients::androidenterprise::EntitlementsListResponse;
use crate::providers::gcp::clients::androidenterprise::GenerateEnterpriseUpgradeUrlResponse;
use crate::providers::gcp::clients::androidenterprise::GroupLicense;
use crate::providers::gcp::clients::androidenterprise::GroupLicenseUsersListResponse;
use crate::providers::gcp::clients::androidenterprise::GroupLicensesListResponse;
use crate::providers::gcp::clients::androidenterprise::Install;
use crate::providers::gcp::clients::androidenterprise::InstallsListResponse;
use crate::providers::gcp::clients::androidenterprise::ManagedConfiguration;
use crate::providers::gcp::clients::androidenterprise::ManagedConfigurationsForDeviceListResponse;
use crate::providers::gcp::clients::androidenterprise::ManagedConfigurationsForUserListResponse;
use crate::providers::gcp::clients::androidenterprise::ManagedConfigurationsSettingsListResponse;
use crate::providers::gcp::clients::androidenterprise::NotificationSet;
use crate::providers::gcp::clients::androidenterprise::Permission;
use crate::providers::gcp::clients::androidenterprise::Product;
use crate::providers::gcp::clients::androidenterprise::ProductPermissions;
use crate::providers::gcp::clients::androidenterprise::ProductSet;
use crate::providers::gcp::clients::androidenterprise::ProductsGenerateApprovalUrlResponse;
use crate::providers::gcp::clients::androidenterprise::ProductsListResponse;
use crate::providers::gcp::clients::androidenterprise::ServiceAccount;
use crate::providers::gcp::clients::androidenterprise::ServiceAccountKey;
use crate::providers::gcp::clients::androidenterprise::ServiceAccountKeysListResponse;
use crate::providers::gcp::clients::androidenterprise::SignupInfo;
use crate::providers::gcp::clients::androidenterprise::StoreCluster;
use crate::providers::gcp::clients::androidenterprise::StoreLayout;
use crate::providers::gcp::clients::androidenterprise::StoreLayoutClustersListResponse;
use crate::providers::gcp::clients::androidenterprise::StoreLayoutPagesListResponse;
use crate::providers::gcp::clients::androidenterprise::StorePage;
use crate::providers::gcp::clients::androidenterprise::User;
use crate::providers::gcp::clients::androidenterprise::UsersListResponse;
use crate::providers::gcp::clients::androidenterprise::WebApp;
use crate::providers::gcp::clients::androidenterprise::WebAppsListResponse;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseDevicesForceReportUploadArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseDevicesGetArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseDevicesGetStateArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseDevicesListArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseDevicesSetStateArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseDevicesUpdateArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseEnrollmentTokensCreateArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseEnterprisesAcknowledgeNotificationSetArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseEnterprisesCompleteSignupArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseEnterprisesCreateWebTokenArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseEnterprisesEnrollArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseEnterprisesGenerateEnterpriseUpgradeUrlArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseEnterprisesGenerateSignupUrlArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseEnterprisesGetArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseEnterprisesGetServiceAccountArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseEnterprisesGetStoreLayoutArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseEnterprisesListArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseEnterprisesPullNotificationSetArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseEnterprisesSendTestPushNotificationArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseEnterprisesSetAccountArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseEnterprisesSetStoreLayoutArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseEnterprisesUnenrollArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseEntitlementsDeleteArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseEntitlementsGetArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseEntitlementsListArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseEntitlementsUpdateArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseGrouplicensesGetArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseGrouplicensesListArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseGrouplicenseusersListArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseInstallsDeleteArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseInstallsGetArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseInstallsListArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseInstallsUpdateArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseManagedconfigurationsfordeviceDeleteArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseManagedconfigurationsfordeviceGetArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseManagedconfigurationsfordeviceListArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseManagedconfigurationsfordeviceUpdateArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseManagedconfigurationsforuserDeleteArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseManagedconfigurationsforuserGetArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseManagedconfigurationsforuserListArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseManagedconfigurationsforuserUpdateArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseManagedconfigurationssettingsListArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterprisePermissionsGetArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseProductsApproveArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseProductsGenerateApprovalUrlArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseProductsGetAppRestrictionsSchemaArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseProductsGetArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseProductsGetPermissionsArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseProductsListArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseProductsUnapproveArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseServiceaccountkeysDeleteArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseServiceaccountkeysInsertArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseServiceaccountkeysListArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseStorelayoutclustersDeleteArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseStorelayoutclustersGetArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseStorelayoutclustersInsertArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseStorelayoutclustersListArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseStorelayoutclustersUpdateArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseStorelayoutpagesDeleteArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseStorelayoutpagesGetArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseStorelayoutpagesInsertArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseStorelayoutpagesListArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseStorelayoutpagesUpdateArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseUsersDeleteArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseUsersGenerateAuthenticationTokenArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseUsersGetArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseUsersGetAvailableProductSetArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseUsersInsertArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseUsersListArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseUsersRevokeDeviceAccessArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseUsersSetAvailableProductSetArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseUsersUpdateArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseWebappsDeleteArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseWebappsGetArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseWebappsInsertArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseWebappsListArgs;
use crate::providers::gcp::clients::androidenterprise::AndroidenterpriseWebappsUpdateArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// AndroidenterpriseProvider with automatic state tracking.
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
/// let provider = AndroidenterpriseProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct AndroidenterpriseProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> AndroidenterpriseProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new AndroidenterpriseProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new AndroidenterpriseProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Androidenterprise devices force report upload.
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
    pub fn androidenterprise_devices_force_report_upload(
        &self,
        args: &AndroidenterpriseDevicesForceReportUploadArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_devices_force_report_upload_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.userId,
            &args.deviceId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_devices_force_report_upload_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise devices get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Device result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidenterprise_devices_get(
        &self,
        args: &AndroidenterpriseDevicesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Device, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_devices_get_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.userId,
            &args.deviceId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_devices_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise devices get state.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeviceState result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidenterprise_devices_get_state(
        &self,
        args: &AndroidenterpriseDevicesGetStateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeviceState, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_devices_get_state_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.userId,
            &args.deviceId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_devices_get_state_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise devices list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DevicesListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidenterprise_devices_list(
        &self,
        args: &AndroidenterpriseDevicesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DevicesListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_devices_list_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_devices_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise devices set state.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeviceState result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidenterprise_devices_set_state(
        &self,
        args: &AndroidenterpriseDevicesSetStateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeviceState, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_devices_set_state_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.userId,
            &args.deviceId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_devices_set_state_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise devices update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Device result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidenterprise_devices_update(
        &self,
        args: &AndroidenterpriseDevicesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Device, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_devices_update_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.userId,
            &args.deviceId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_devices_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise enrollment tokens create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EnrollmentToken result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidenterprise_enrollment_tokens_create(
        &self,
        args: &AndroidenterpriseEnrollmentTokensCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EnrollmentToken, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_enrollment_tokens_create_builder(
            &self.http_client,
            &args.enterpriseId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_enrollment_tokens_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise enterprises acknowledge notification set.
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
    pub fn androidenterprise_enterprises_acknowledge_notification_set(
        &self,
        args: &AndroidenterpriseEnterprisesAcknowledgeNotificationSetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_enterprises_acknowledge_notification_set_builder(
            &self.http_client,
            &args.notificationSetId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_enterprises_acknowledge_notification_set_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise enterprises complete signup.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Enterprise result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidenterprise_enterprises_complete_signup(
        &self,
        args: &AndroidenterpriseEnterprisesCompleteSignupArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Enterprise, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_enterprises_complete_signup_builder(
            &self.http_client,
            &args.completionToken,
            &args.enterpriseToken,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_enterprises_complete_signup_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise enterprises create web token.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AdministratorWebToken result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidenterprise_enterprises_create_web_token(
        &self,
        args: &AndroidenterpriseEnterprisesCreateWebTokenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AdministratorWebToken, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_enterprises_create_web_token_builder(
            &self.http_client,
            &args.enterpriseId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_enterprises_create_web_token_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise enterprises enroll.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Enterprise result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidenterprise_enterprises_enroll(
        &self,
        args: &AndroidenterpriseEnterprisesEnrollArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Enterprise, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_enterprises_enroll_builder(
            &self.http_client,
            &args.token,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_enterprises_enroll_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise enterprises generate enterprise upgrade url.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GenerateEnterpriseUpgradeUrlResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidenterprise_enterprises_generate_enterprise_upgrade_url(
        &self,
        args: &AndroidenterpriseEnterprisesGenerateEnterpriseUpgradeUrlArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GenerateEnterpriseUpgradeUrlResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_enterprises_generate_enterprise_upgrade_url_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.adminEmail,
            &args.allowedDomains,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_enterprises_generate_enterprise_upgrade_url_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise enterprises generate signup url.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SignupInfo result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidenterprise_enterprises_generate_signup_url(
        &self,
        args: &AndroidenterpriseEnterprisesGenerateSignupUrlArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SignupInfo, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_enterprises_generate_signup_url_builder(
            &self.http_client,
            &args.adminEmail,
            &args.allowedDomains,
            &args.callbackUrl,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_enterprises_generate_signup_url_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise enterprises get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Enterprise result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidenterprise_enterprises_get(
        &self,
        args: &AndroidenterpriseEnterprisesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Enterprise, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_enterprises_get_builder(
            &self.http_client,
            &args.enterpriseId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_enterprises_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise enterprises get service account.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ServiceAccount result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidenterprise_enterprises_get_service_account(
        &self,
        args: &AndroidenterpriseEnterprisesGetServiceAccountArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ServiceAccount, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_enterprises_get_service_account_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.keyType,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_enterprises_get_service_account_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise enterprises get store layout.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the StoreLayout result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidenterprise_enterprises_get_store_layout(
        &self,
        args: &AndroidenterpriseEnterprisesGetStoreLayoutArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<StoreLayout, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_enterprises_get_store_layout_builder(
            &self.http_client,
            &args.enterpriseId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_enterprises_get_store_layout_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise enterprises list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EnterprisesListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidenterprise_enterprises_list(
        &self,
        args: &AndroidenterpriseEnterprisesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EnterprisesListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_enterprises_list_builder(
            &self.http_client,
            &args.domain,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_enterprises_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise enterprises pull notification set.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NotificationSet result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidenterprise_enterprises_pull_notification_set(
        &self,
        args: &AndroidenterpriseEnterprisesPullNotificationSetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NotificationSet, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_enterprises_pull_notification_set_builder(
            &self.http_client,
            &args.requestMode,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_enterprises_pull_notification_set_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise enterprises send test push notification.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EnterprisesSendTestPushNotificationResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidenterprise_enterprises_send_test_push_notification(
        &self,
        args: &AndroidenterpriseEnterprisesSendTestPushNotificationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EnterprisesSendTestPushNotificationResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_enterprises_send_test_push_notification_builder(
            &self.http_client,
            &args.enterpriseId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_enterprises_send_test_push_notification_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise enterprises set account.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EnterpriseAccount result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidenterprise_enterprises_set_account(
        &self,
        args: &AndroidenterpriseEnterprisesSetAccountArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EnterpriseAccount, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_enterprises_set_account_builder(
            &self.http_client,
            &args.enterpriseId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_enterprises_set_account_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise enterprises set store layout.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the StoreLayout result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidenterprise_enterprises_set_store_layout(
        &self,
        args: &AndroidenterpriseEnterprisesSetStoreLayoutArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<StoreLayout, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_enterprises_set_store_layout_builder(
            &self.http_client,
            &args.enterpriseId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_enterprises_set_store_layout_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise enterprises unenroll.
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
    pub fn androidenterprise_enterprises_unenroll(
        &self,
        args: &AndroidenterpriseEnterprisesUnenrollArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_enterprises_unenroll_builder(
            &self.http_client,
            &args.enterpriseId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_enterprises_unenroll_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise entitlements delete.
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
    pub fn androidenterprise_entitlements_delete(
        &self,
        args: &AndroidenterpriseEntitlementsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_entitlements_delete_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.userId,
            &args.entitlementId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_entitlements_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise entitlements get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Entitlement result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidenterprise_entitlements_get(
        &self,
        args: &AndroidenterpriseEntitlementsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Entitlement, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_entitlements_get_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.userId,
            &args.entitlementId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_entitlements_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise entitlements list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EntitlementsListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidenterprise_entitlements_list(
        &self,
        args: &AndroidenterpriseEntitlementsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EntitlementsListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_entitlements_list_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_entitlements_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise entitlements update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Entitlement result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidenterprise_entitlements_update(
        &self,
        args: &AndroidenterpriseEntitlementsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Entitlement, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_entitlements_update_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.userId,
            &args.entitlementId,
            &args.install,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_entitlements_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise grouplicenses get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GroupLicense result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidenterprise_grouplicenses_get(
        &self,
        args: &AndroidenterpriseGrouplicensesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GroupLicense, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_grouplicenses_get_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.groupLicenseId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_grouplicenses_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise grouplicenses list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GroupLicensesListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidenterprise_grouplicenses_list(
        &self,
        args: &AndroidenterpriseGrouplicensesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GroupLicensesListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_grouplicenses_list_builder(
            &self.http_client,
            &args.enterpriseId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_grouplicenses_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise grouplicenseusers list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GroupLicenseUsersListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidenterprise_grouplicenseusers_list(
        &self,
        args: &AndroidenterpriseGrouplicenseusersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GroupLicenseUsersListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_grouplicenseusers_list_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.groupLicenseId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_grouplicenseusers_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise installs delete.
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
    pub fn androidenterprise_installs_delete(
        &self,
        args: &AndroidenterpriseInstallsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_installs_delete_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.userId,
            &args.deviceId,
            &args.installId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_installs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise installs get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Install result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidenterprise_installs_get(
        &self,
        args: &AndroidenterpriseInstallsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Install, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_installs_get_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.userId,
            &args.deviceId,
            &args.installId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_installs_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise installs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the InstallsListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidenterprise_installs_list(
        &self,
        args: &AndroidenterpriseInstallsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<InstallsListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_installs_list_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.userId,
            &args.deviceId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_installs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise installs update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Install result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidenterprise_installs_update(
        &self,
        args: &AndroidenterpriseInstallsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Install, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_installs_update_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.userId,
            &args.deviceId,
            &args.installId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_installs_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise managedconfigurationsfordevice delete.
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
    pub fn androidenterprise_managedconfigurationsfordevice_delete(
        &self,
        args: &AndroidenterpriseManagedconfigurationsfordeviceDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_managedconfigurationsfordevice_delete_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.userId,
            &args.deviceId,
            &args.managedConfigurationForDeviceId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_managedconfigurationsfordevice_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise managedconfigurationsfordevice get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ManagedConfiguration result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidenterprise_managedconfigurationsfordevice_get(
        &self,
        args: &AndroidenterpriseManagedconfigurationsfordeviceGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ManagedConfiguration, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_managedconfigurationsfordevice_get_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.userId,
            &args.deviceId,
            &args.managedConfigurationForDeviceId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_managedconfigurationsfordevice_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise managedconfigurationsfordevice list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ManagedConfigurationsForDeviceListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidenterprise_managedconfigurationsfordevice_list(
        &self,
        args: &AndroidenterpriseManagedconfigurationsfordeviceListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ManagedConfigurationsForDeviceListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_managedconfigurationsfordevice_list_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.userId,
            &args.deviceId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_managedconfigurationsfordevice_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise managedconfigurationsfordevice update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ManagedConfiguration result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidenterprise_managedconfigurationsfordevice_update(
        &self,
        args: &AndroidenterpriseManagedconfigurationsfordeviceUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ManagedConfiguration, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_managedconfigurationsfordevice_update_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.userId,
            &args.deviceId,
            &args.managedConfigurationForDeviceId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_managedconfigurationsfordevice_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise managedconfigurationsforuser delete.
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
    pub fn androidenterprise_managedconfigurationsforuser_delete(
        &self,
        args: &AndroidenterpriseManagedconfigurationsforuserDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_managedconfigurationsforuser_delete_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.userId,
            &args.managedConfigurationForUserId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_managedconfigurationsforuser_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise managedconfigurationsforuser get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ManagedConfiguration result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidenterprise_managedconfigurationsforuser_get(
        &self,
        args: &AndroidenterpriseManagedconfigurationsforuserGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ManagedConfiguration, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_managedconfigurationsforuser_get_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.userId,
            &args.managedConfigurationForUserId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_managedconfigurationsforuser_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise managedconfigurationsforuser list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ManagedConfigurationsForUserListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidenterprise_managedconfigurationsforuser_list(
        &self,
        args: &AndroidenterpriseManagedconfigurationsforuserListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ManagedConfigurationsForUserListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_managedconfigurationsforuser_list_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_managedconfigurationsforuser_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise managedconfigurationsforuser update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ManagedConfiguration result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidenterprise_managedconfigurationsforuser_update(
        &self,
        args: &AndroidenterpriseManagedconfigurationsforuserUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ManagedConfiguration, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_managedconfigurationsforuser_update_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.userId,
            &args.managedConfigurationForUserId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_managedconfigurationsforuser_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise managedconfigurationssettings list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ManagedConfigurationsSettingsListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidenterprise_managedconfigurationssettings_list(
        &self,
        args: &AndroidenterpriseManagedconfigurationssettingsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ManagedConfigurationsSettingsListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_managedconfigurationssettings_list_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.productId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_managedconfigurationssettings_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise permissions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Permission result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidenterprise_permissions_get(
        &self,
        args: &AndroidenterprisePermissionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Permission, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_permissions_get_builder(
            &self.http_client,
            &args.permissionId,
            &args.language,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_permissions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise products approve.
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
    pub fn androidenterprise_products_approve(
        &self,
        args: &AndroidenterpriseProductsApproveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_products_approve_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.productId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_products_approve_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise products generate approval url.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProductsGenerateApprovalUrlResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidenterprise_products_generate_approval_url(
        &self,
        args: &AndroidenterpriseProductsGenerateApprovalUrlArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProductsGenerateApprovalUrlResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_products_generate_approval_url_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.productId,
            &args.languageCode,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_products_generate_approval_url_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise products get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Product result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidenterprise_products_get(
        &self,
        args: &AndroidenterpriseProductsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Product, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_products_get_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.productId,
            &args.language,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_products_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise products get app restrictions schema.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AppRestrictionsSchema result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidenterprise_products_get_app_restrictions_schema(
        &self,
        args: &AndroidenterpriseProductsGetAppRestrictionsSchemaArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AppRestrictionsSchema, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_products_get_app_restrictions_schema_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.productId,
            &args.language,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_products_get_app_restrictions_schema_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise products get permissions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProductPermissions result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidenterprise_products_get_permissions(
        &self,
        args: &AndroidenterpriseProductsGetPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProductPermissions, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_products_get_permissions_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.productId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_products_get_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise products list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProductsListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidenterprise_products_list(
        &self,
        args: &AndroidenterpriseProductsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProductsListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_products_list_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.approved,
            &args.language,
            &args.maxResults,
            &args.query,
            &args.token,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_products_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise products unapprove.
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
    pub fn androidenterprise_products_unapprove(
        &self,
        args: &AndroidenterpriseProductsUnapproveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_products_unapprove_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.productId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_products_unapprove_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise serviceaccountkeys delete.
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
    pub fn androidenterprise_serviceaccountkeys_delete(
        &self,
        args: &AndroidenterpriseServiceaccountkeysDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_serviceaccountkeys_delete_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.keyId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_serviceaccountkeys_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise serviceaccountkeys insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ServiceAccountKey result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidenterprise_serviceaccountkeys_insert(
        &self,
        args: &AndroidenterpriseServiceaccountkeysInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ServiceAccountKey, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_serviceaccountkeys_insert_builder(
            &self.http_client,
            &args.enterpriseId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_serviceaccountkeys_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise serviceaccountkeys list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ServiceAccountKeysListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidenterprise_serviceaccountkeys_list(
        &self,
        args: &AndroidenterpriseServiceaccountkeysListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ServiceAccountKeysListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_serviceaccountkeys_list_builder(
            &self.http_client,
            &args.enterpriseId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_serviceaccountkeys_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise storelayoutclusters delete.
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
    pub fn androidenterprise_storelayoutclusters_delete(
        &self,
        args: &AndroidenterpriseStorelayoutclustersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_storelayoutclusters_delete_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.pageId,
            &args.clusterId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_storelayoutclusters_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise storelayoutclusters get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the StoreCluster result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidenterprise_storelayoutclusters_get(
        &self,
        args: &AndroidenterpriseStorelayoutclustersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<StoreCluster, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_storelayoutclusters_get_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.pageId,
            &args.clusterId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_storelayoutclusters_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise storelayoutclusters insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the StoreCluster result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidenterprise_storelayoutclusters_insert(
        &self,
        args: &AndroidenterpriseStorelayoutclustersInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<StoreCluster, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_storelayoutclusters_insert_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.pageId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_storelayoutclusters_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise storelayoutclusters list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the StoreLayoutClustersListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidenterprise_storelayoutclusters_list(
        &self,
        args: &AndroidenterpriseStorelayoutclustersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<StoreLayoutClustersListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_storelayoutclusters_list_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.pageId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_storelayoutclusters_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise storelayoutclusters update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the StoreCluster result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidenterprise_storelayoutclusters_update(
        &self,
        args: &AndroidenterpriseStorelayoutclustersUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<StoreCluster, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_storelayoutclusters_update_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.pageId,
            &args.clusterId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_storelayoutclusters_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise storelayoutpages delete.
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
    pub fn androidenterprise_storelayoutpages_delete(
        &self,
        args: &AndroidenterpriseStorelayoutpagesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_storelayoutpages_delete_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.pageId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_storelayoutpages_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise storelayoutpages get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the StorePage result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidenterprise_storelayoutpages_get(
        &self,
        args: &AndroidenterpriseStorelayoutpagesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<StorePage, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_storelayoutpages_get_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.pageId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_storelayoutpages_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise storelayoutpages insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the StorePage result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidenterprise_storelayoutpages_insert(
        &self,
        args: &AndroidenterpriseStorelayoutpagesInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<StorePage, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_storelayoutpages_insert_builder(
            &self.http_client,
            &args.enterpriseId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_storelayoutpages_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise storelayoutpages list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the StoreLayoutPagesListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidenterprise_storelayoutpages_list(
        &self,
        args: &AndroidenterpriseStorelayoutpagesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<StoreLayoutPagesListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_storelayoutpages_list_builder(
            &self.http_client,
            &args.enterpriseId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_storelayoutpages_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise storelayoutpages update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the StorePage result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidenterprise_storelayoutpages_update(
        &self,
        args: &AndroidenterpriseStorelayoutpagesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<StorePage, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_storelayoutpages_update_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.pageId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_storelayoutpages_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise users delete.
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
    pub fn androidenterprise_users_delete(
        &self,
        args: &AndroidenterpriseUsersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_users_delete_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_users_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise users generate authentication token.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AuthenticationToken result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidenterprise_users_generate_authentication_token(
        &self,
        args: &AndroidenterpriseUsersGenerateAuthenticationTokenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AuthenticationToken, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_users_generate_authentication_token_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_users_generate_authentication_token_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise users get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the User result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidenterprise_users_get(
        &self,
        args: &AndroidenterpriseUsersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<User, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_users_get_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_users_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise users get available product set.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProductSet result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidenterprise_users_get_available_product_set(
        &self,
        args: &AndroidenterpriseUsersGetAvailableProductSetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProductSet, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_users_get_available_product_set_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_users_get_available_product_set_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise users insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the User result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidenterprise_users_insert(
        &self,
        args: &AndroidenterpriseUsersInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<User, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_users_insert_builder(
            &self.http_client,
            &args.enterpriseId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_users_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise users list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UsersListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidenterprise_users_list(
        &self,
        args: &AndroidenterpriseUsersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UsersListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_users_list_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.email,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_users_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise users revoke device access.
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
    pub fn androidenterprise_users_revoke_device_access(
        &self,
        args: &AndroidenterpriseUsersRevokeDeviceAccessArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_users_revoke_device_access_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_users_revoke_device_access_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise users set available product set.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProductSet result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidenterprise_users_set_available_product_set(
        &self,
        args: &AndroidenterpriseUsersSetAvailableProductSetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProductSet, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_users_set_available_product_set_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_users_set_available_product_set_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise users update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the User result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidenterprise_users_update(
        &self,
        args: &AndroidenterpriseUsersUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<User, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_users_update_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_users_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise webapps delete.
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
    pub fn androidenterprise_webapps_delete(
        &self,
        args: &AndroidenterpriseWebappsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_webapps_delete_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.webAppId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_webapps_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise webapps get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WebApp result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidenterprise_webapps_get(
        &self,
        args: &AndroidenterpriseWebappsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WebApp, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_webapps_get_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.webAppId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_webapps_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise webapps insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WebApp result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidenterprise_webapps_insert(
        &self,
        args: &AndroidenterpriseWebappsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WebApp, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_webapps_insert_builder(
            &self.http_client,
            &args.enterpriseId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_webapps_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise webapps list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WebAppsListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidenterprise_webapps_list(
        &self,
        args: &AndroidenterpriseWebappsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WebAppsListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_webapps_list_builder(
            &self.http_client,
            &args.enterpriseId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_webapps_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidenterprise webapps update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WebApp result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidenterprise_webapps_update(
        &self,
        args: &AndroidenterpriseWebappsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WebApp, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidenterprise_webapps_update_builder(
            &self.http_client,
            &args.enterpriseId,
            &args.webAppId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidenterprise_webapps_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
