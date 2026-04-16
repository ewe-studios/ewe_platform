//! CloudchannelProvider - State-aware cloudchannel API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       cloudchannel API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::cloudchannel::{
    cloudchannel_accounts_check_cloud_identity_accounts_exist_builder, cloudchannel_accounts_check_cloud_identity_accounts_exist_task,
    cloudchannel_accounts_list_subscribers_builder, cloudchannel_accounts_list_subscribers_task,
    cloudchannel_accounts_list_transferable_offers_builder, cloudchannel_accounts_list_transferable_offers_task,
    cloudchannel_accounts_list_transferable_skus_builder, cloudchannel_accounts_list_transferable_skus_task,
    cloudchannel_accounts_register_builder, cloudchannel_accounts_register_task,
    cloudchannel_accounts_unregister_builder, cloudchannel_accounts_unregister_task,
    cloudchannel_accounts_channel_partner_links_create_builder, cloudchannel_accounts_channel_partner_links_create_task,
    cloudchannel_accounts_channel_partner_links_get_builder, cloudchannel_accounts_channel_partner_links_get_task,
    cloudchannel_accounts_channel_partner_links_list_builder, cloudchannel_accounts_channel_partner_links_list_task,
    cloudchannel_accounts_channel_partner_links_patch_builder, cloudchannel_accounts_channel_partner_links_patch_task,
    cloudchannel_accounts_channel_partner_links_channel_partner_repricing_configs_create_builder, cloudchannel_accounts_channel_partner_links_channel_partner_repricing_configs_create_task,
    cloudchannel_accounts_channel_partner_links_channel_partner_repricing_configs_delete_builder, cloudchannel_accounts_channel_partner_links_channel_partner_repricing_configs_delete_task,
    cloudchannel_accounts_channel_partner_links_channel_partner_repricing_configs_get_builder, cloudchannel_accounts_channel_partner_links_channel_partner_repricing_configs_get_task,
    cloudchannel_accounts_channel_partner_links_channel_partner_repricing_configs_list_builder, cloudchannel_accounts_channel_partner_links_channel_partner_repricing_configs_list_task,
    cloudchannel_accounts_channel_partner_links_channel_partner_repricing_configs_patch_builder, cloudchannel_accounts_channel_partner_links_channel_partner_repricing_configs_patch_task,
    cloudchannel_accounts_channel_partner_links_customers_create_builder, cloudchannel_accounts_channel_partner_links_customers_create_task,
    cloudchannel_accounts_channel_partner_links_customers_delete_builder, cloudchannel_accounts_channel_partner_links_customers_delete_task,
    cloudchannel_accounts_channel_partner_links_customers_get_builder, cloudchannel_accounts_channel_partner_links_customers_get_task,
    cloudchannel_accounts_channel_partner_links_customers_import_builder, cloudchannel_accounts_channel_partner_links_customers_import_task,
    cloudchannel_accounts_channel_partner_links_customers_list_builder, cloudchannel_accounts_channel_partner_links_customers_list_task,
    cloudchannel_accounts_channel_partner_links_customers_patch_builder, cloudchannel_accounts_channel_partner_links_customers_patch_task,
    cloudchannel_accounts_customers_create_builder, cloudchannel_accounts_customers_create_task,
    cloudchannel_accounts_customers_delete_builder, cloudchannel_accounts_customers_delete_task,
    cloudchannel_accounts_customers_get_builder, cloudchannel_accounts_customers_get_task,
    cloudchannel_accounts_customers_import_builder, cloudchannel_accounts_customers_import_task,
    cloudchannel_accounts_customers_list_builder, cloudchannel_accounts_customers_list_task,
    cloudchannel_accounts_customers_list_purchasable_offers_builder, cloudchannel_accounts_customers_list_purchasable_offers_task,
    cloudchannel_accounts_customers_list_purchasable_skus_builder, cloudchannel_accounts_customers_list_purchasable_skus_task,
    cloudchannel_accounts_customers_patch_builder, cloudchannel_accounts_customers_patch_task,
    cloudchannel_accounts_customers_provision_cloud_identity_builder, cloudchannel_accounts_customers_provision_cloud_identity_task,
    cloudchannel_accounts_customers_query_eligible_billing_accounts_builder, cloudchannel_accounts_customers_query_eligible_billing_accounts_task,
    cloudchannel_accounts_customers_transfer_entitlements_builder, cloudchannel_accounts_customers_transfer_entitlements_task,
    cloudchannel_accounts_customers_transfer_entitlements_to_google_builder, cloudchannel_accounts_customers_transfer_entitlements_to_google_task,
    cloudchannel_accounts_customers_customer_repricing_configs_create_builder, cloudchannel_accounts_customers_customer_repricing_configs_create_task,
    cloudchannel_accounts_customers_customer_repricing_configs_delete_builder, cloudchannel_accounts_customers_customer_repricing_configs_delete_task,
    cloudchannel_accounts_customers_customer_repricing_configs_get_builder, cloudchannel_accounts_customers_customer_repricing_configs_get_task,
    cloudchannel_accounts_customers_customer_repricing_configs_list_builder, cloudchannel_accounts_customers_customer_repricing_configs_list_task,
    cloudchannel_accounts_customers_customer_repricing_configs_patch_builder, cloudchannel_accounts_customers_customer_repricing_configs_patch_task,
    cloudchannel_accounts_customers_entitlements_activate_builder, cloudchannel_accounts_customers_entitlements_activate_task,
    cloudchannel_accounts_customers_entitlements_cancel_builder, cloudchannel_accounts_customers_entitlements_cancel_task,
    cloudchannel_accounts_customers_entitlements_change_offer_builder, cloudchannel_accounts_customers_entitlements_change_offer_task,
    cloudchannel_accounts_customers_entitlements_change_parameters_builder, cloudchannel_accounts_customers_entitlements_change_parameters_task,
    cloudchannel_accounts_customers_entitlements_change_renewal_settings_builder, cloudchannel_accounts_customers_entitlements_change_renewal_settings_task,
    cloudchannel_accounts_customers_entitlements_create_builder, cloudchannel_accounts_customers_entitlements_create_task,
    cloudchannel_accounts_customers_entitlements_get_builder, cloudchannel_accounts_customers_entitlements_get_task,
    cloudchannel_accounts_customers_entitlements_list_builder, cloudchannel_accounts_customers_entitlements_list_task,
    cloudchannel_accounts_customers_entitlements_list_entitlement_changes_builder, cloudchannel_accounts_customers_entitlements_list_entitlement_changes_task,
    cloudchannel_accounts_customers_entitlements_lookup_offer_builder, cloudchannel_accounts_customers_entitlements_lookup_offer_task,
    cloudchannel_accounts_customers_entitlements_start_paid_service_builder, cloudchannel_accounts_customers_entitlements_start_paid_service_task,
    cloudchannel_accounts_customers_entitlements_suspend_builder, cloudchannel_accounts_customers_entitlements_suspend_task,
    cloudchannel_accounts_offers_list_builder, cloudchannel_accounts_offers_list_task,
    cloudchannel_accounts_report_jobs_fetch_report_results_builder, cloudchannel_accounts_report_jobs_fetch_report_results_task,
    cloudchannel_accounts_reports_list_builder, cloudchannel_accounts_reports_list_task,
    cloudchannel_accounts_reports_run_builder, cloudchannel_accounts_reports_run_task,
    cloudchannel_accounts_sku_groups_list_builder, cloudchannel_accounts_sku_groups_list_task,
    cloudchannel_accounts_sku_groups_billable_skus_list_builder, cloudchannel_accounts_sku_groups_billable_skus_list_task,
    cloudchannel_integrators_list_subscribers_builder, cloudchannel_integrators_list_subscribers_task,
    cloudchannel_integrators_register_subscriber_builder, cloudchannel_integrators_register_subscriber_task,
    cloudchannel_integrators_unregister_subscriber_builder, cloudchannel_integrators_unregister_subscriber_task,
    cloudchannel_operations_cancel_builder, cloudchannel_operations_cancel_task,
    cloudchannel_operations_delete_builder, cloudchannel_operations_delete_task,
    cloudchannel_operations_get_builder, cloudchannel_operations_get_task,
    cloudchannel_operations_list_builder, cloudchannel_operations_list_task,
    cloudchannel_products_list_builder, cloudchannel_products_list_task,
    cloudchannel_products_skus_list_builder, cloudchannel_products_skus_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::cloudchannel::GoogleCloudChannelV1ChannelPartnerLink;
use crate::providers::gcp::clients::cloudchannel::GoogleCloudChannelV1ChannelPartnerRepricingConfig;
use crate::providers::gcp::clients::cloudchannel::GoogleCloudChannelV1CheckCloudIdentityAccountsExistResponse;
use crate::providers::gcp::clients::cloudchannel::GoogleCloudChannelV1Customer;
use crate::providers::gcp::clients::cloudchannel::GoogleCloudChannelV1CustomerRepricingConfig;
use crate::providers::gcp::clients::cloudchannel::GoogleCloudChannelV1Entitlement;
use crate::providers::gcp::clients::cloudchannel::GoogleCloudChannelV1FetchReportResultsResponse;
use crate::providers::gcp::clients::cloudchannel::GoogleCloudChannelV1ListChannelPartnerLinksResponse;
use crate::providers::gcp::clients::cloudchannel::GoogleCloudChannelV1ListChannelPartnerRepricingConfigsResponse;
use crate::providers::gcp::clients::cloudchannel::GoogleCloudChannelV1ListCustomerRepricingConfigsResponse;
use crate::providers::gcp::clients::cloudchannel::GoogleCloudChannelV1ListCustomersResponse;
use crate::providers::gcp::clients::cloudchannel::GoogleCloudChannelV1ListEntitlementChangesResponse;
use crate::providers::gcp::clients::cloudchannel::GoogleCloudChannelV1ListEntitlementsResponse;
use crate::providers::gcp::clients::cloudchannel::GoogleCloudChannelV1ListOffersResponse;
use crate::providers::gcp::clients::cloudchannel::GoogleCloudChannelV1ListProductsResponse;
use crate::providers::gcp::clients::cloudchannel::GoogleCloudChannelV1ListPurchasableOffersResponse;
use crate::providers::gcp::clients::cloudchannel::GoogleCloudChannelV1ListPurchasableSkusResponse;
use crate::providers::gcp::clients::cloudchannel::GoogleCloudChannelV1ListReportsResponse;
use crate::providers::gcp::clients::cloudchannel::GoogleCloudChannelV1ListSkuGroupBillableSkusResponse;
use crate::providers::gcp::clients::cloudchannel::GoogleCloudChannelV1ListSkuGroupsResponse;
use crate::providers::gcp::clients::cloudchannel::GoogleCloudChannelV1ListSkusResponse;
use crate::providers::gcp::clients::cloudchannel::GoogleCloudChannelV1ListSubscribersResponse;
use crate::providers::gcp::clients::cloudchannel::GoogleCloudChannelV1ListTransferableOffersResponse;
use crate::providers::gcp::clients::cloudchannel::GoogleCloudChannelV1ListTransferableSkusResponse;
use crate::providers::gcp::clients::cloudchannel::GoogleCloudChannelV1Offer;
use crate::providers::gcp::clients::cloudchannel::GoogleCloudChannelV1QueryEligibleBillingAccountsResponse;
use crate::providers::gcp::clients::cloudchannel::GoogleCloudChannelV1RegisterSubscriberResponse;
use crate::providers::gcp::clients::cloudchannel::GoogleCloudChannelV1UnregisterSubscriberResponse;
use crate::providers::gcp::clients::cloudchannel::GoogleLongrunningListOperationsResponse;
use crate::providers::gcp::clients::cloudchannel::GoogleLongrunningOperation;
use crate::providers::gcp::clients::cloudchannel::GoogleProtobufEmpty;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsChannelPartnerLinksChannelPartnerRepricingConfigsCreateArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsChannelPartnerLinksChannelPartnerRepricingConfigsDeleteArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsChannelPartnerLinksChannelPartnerRepricingConfigsGetArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsChannelPartnerLinksChannelPartnerRepricingConfigsListArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsChannelPartnerLinksChannelPartnerRepricingConfigsPatchArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsChannelPartnerLinksCreateArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsChannelPartnerLinksCustomersCreateArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsChannelPartnerLinksCustomersDeleteArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsChannelPartnerLinksCustomersGetArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsChannelPartnerLinksCustomersImportArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsChannelPartnerLinksCustomersListArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsChannelPartnerLinksCustomersPatchArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsChannelPartnerLinksGetArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsChannelPartnerLinksListArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsChannelPartnerLinksPatchArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsCheckCloudIdentityAccountsExistArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsCustomersCreateArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsCustomersCustomerRepricingConfigsCreateArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsCustomersCustomerRepricingConfigsDeleteArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsCustomersCustomerRepricingConfigsGetArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsCustomersCustomerRepricingConfigsListArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsCustomersCustomerRepricingConfigsPatchArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsCustomersDeleteArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsCustomersEntitlementsActivateArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsCustomersEntitlementsCancelArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsCustomersEntitlementsChangeOfferArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsCustomersEntitlementsChangeParametersArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsCustomersEntitlementsChangeRenewalSettingsArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsCustomersEntitlementsCreateArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsCustomersEntitlementsGetArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsCustomersEntitlementsListArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsCustomersEntitlementsListEntitlementChangesArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsCustomersEntitlementsLookupOfferArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsCustomersEntitlementsStartPaidServiceArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsCustomersEntitlementsSuspendArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsCustomersGetArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsCustomersImportArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsCustomersListArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsCustomersListPurchasableOffersArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsCustomersListPurchasableSkusArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsCustomersPatchArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsCustomersProvisionCloudIdentityArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsCustomersQueryEligibleBillingAccountsArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsCustomersTransferEntitlementsArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsCustomersTransferEntitlementsToGoogleArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsListSubscribersArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsListTransferableOffersArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsListTransferableSkusArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsOffersListArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsRegisterArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsReportJobsFetchReportResultsArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsReportsListArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsReportsRunArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsSkuGroupsBillableSkusListArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsSkuGroupsListArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelAccountsUnregisterArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelIntegratorsListSubscribersArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelIntegratorsRegisterSubscriberArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelIntegratorsUnregisterSubscriberArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelOperationsCancelArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelOperationsDeleteArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelOperationsGetArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelOperationsListArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelProductsListArgs;
use crate::providers::gcp::clients::cloudchannel::CloudchannelProductsSkusListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// CloudchannelProvider with automatic state tracking.
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
/// let provider = CloudchannelProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct CloudchannelProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> CloudchannelProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new CloudchannelProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new CloudchannelProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Cloudchannel accounts check cloud identity accounts exist.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudChannelV1CheckCloudIdentityAccountsExistResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudchannel_accounts_check_cloud_identity_accounts_exist(
        &self,
        args: &CloudchannelAccountsCheckCloudIdentityAccountsExistArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudChannelV1CheckCloudIdentityAccountsExistResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_check_cloud_identity_accounts_exist_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_check_cloud_identity_accounts_exist_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts list subscribers.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudChannelV1ListSubscribersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudchannel_accounts_list_subscribers(
        &self,
        args: &CloudchannelAccountsListSubscribersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudChannelV1ListSubscribersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_list_subscribers_builder(
            &self.http_client,
            &args.account,
            &args.integrator,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_list_subscribers_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts list transferable offers.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudChannelV1ListTransferableOffersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudchannel_accounts_list_transferable_offers(
        &self,
        args: &CloudchannelAccountsListTransferableOffersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudChannelV1ListTransferableOffersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_list_transferable_offers_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_list_transferable_offers_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts list transferable skus.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudChannelV1ListTransferableSkusResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudchannel_accounts_list_transferable_skus(
        &self,
        args: &CloudchannelAccountsListTransferableSkusArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudChannelV1ListTransferableSkusResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_list_transferable_skus_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_list_transferable_skus_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts register.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudChannelV1RegisterSubscriberResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudchannel_accounts_register(
        &self,
        args: &CloudchannelAccountsRegisterArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudChannelV1RegisterSubscriberResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_register_builder(
            &self.http_client,
            &args.account,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_register_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts unregister.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudChannelV1UnregisterSubscriberResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudchannel_accounts_unregister(
        &self,
        args: &CloudchannelAccountsUnregisterArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudChannelV1UnregisterSubscriberResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_unregister_builder(
            &self.http_client,
            &args.account,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_unregister_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts channel partner links create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudChannelV1ChannelPartnerLink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudchannel_accounts_channel_partner_links_create(
        &self,
        args: &CloudchannelAccountsChannelPartnerLinksCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudChannelV1ChannelPartnerLink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_channel_partner_links_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_channel_partner_links_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts channel partner links get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudChannelV1ChannelPartnerLink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudchannel_accounts_channel_partner_links_get(
        &self,
        args: &CloudchannelAccountsChannelPartnerLinksGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudChannelV1ChannelPartnerLink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_channel_partner_links_get_builder(
            &self.http_client,
            &args.name,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_channel_partner_links_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts channel partner links list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudChannelV1ListChannelPartnerLinksResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudchannel_accounts_channel_partner_links_list(
        &self,
        args: &CloudchannelAccountsChannelPartnerLinksListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudChannelV1ListChannelPartnerLinksResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_channel_partner_links_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_channel_partner_links_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts channel partner links patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudChannelV1ChannelPartnerLink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudchannel_accounts_channel_partner_links_patch(
        &self,
        args: &CloudchannelAccountsChannelPartnerLinksPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudChannelV1ChannelPartnerLink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_channel_partner_links_patch_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_channel_partner_links_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts channel partner links channel partner repricing configs create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudChannelV1ChannelPartnerRepricingConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudchannel_accounts_channel_partner_links_channel_partner_repricing_configs_create(
        &self,
        args: &CloudchannelAccountsChannelPartnerLinksChannelPartnerRepricingConfigsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudChannelV1ChannelPartnerRepricingConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_channel_partner_links_channel_partner_repricing_configs_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_channel_partner_links_channel_partner_repricing_configs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts channel partner links channel partner repricing configs delete.
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
    pub fn cloudchannel_accounts_channel_partner_links_channel_partner_repricing_configs_delete(
        &self,
        args: &CloudchannelAccountsChannelPartnerLinksChannelPartnerRepricingConfigsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_channel_partner_links_channel_partner_repricing_configs_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_channel_partner_links_channel_partner_repricing_configs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts channel partner links channel partner repricing configs get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudChannelV1ChannelPartnerRepricingConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudchannel_accounts_channel_partner_links_channel_partner_repricing_configs_get(
        &self,
        args: &CloudchannelAccountsChannelPartnerLinksChannelPartnerRepricingConfigsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudChannelV1ChannelPartnerRepricingConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_channel_partner_links_channel_partner_repricing_configs_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_channel_partner_links_channel_partner_repricing_configs_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts channel partner links channel partner repricing configs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudChannelV1ListChannelPartnerRepricingConfigsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudchannel_accounts_channel_partner_links_channel_partner_repricing_configs_list(
        &self,
        args: &CloudchannelAccountsChannelPartnerLinksChannelPartnerRepricingConfigsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudChannelV1ListChannelPartnerRepricingConfigsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_channel_partner_links_channel_partner_repricing_configs_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_channel_partner_links_channel_partner_repricing_configs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts channel partner links channel partner repricing configs patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudChannelV1ChannelPartnerRepricingConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudchannel_accounts_channel_partner_links_channel_partner_repricing_configs_patch(
        &self,
        args: &CloudchannelAccountsChannelPartnerLinksChannelPartnerRepricingConfigsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudChannelV1ChannelPartnerRepricingConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_channel_partner_links_channel_partner_repricing_configs_patch_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_channel_partner_links_channel_partner_repricing_configs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts channel partner links customers create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudChannelV1Customer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudchannel_accounts_channel_partner_links_customers_create(
        &self,
        args: &CloudchannelAccountsChannelPartnerLinksCustomersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudChannelV1Customer, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_channel_partner_links_customers_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_channel_partner_links_customers_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts channel partner links customers delete.
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
    pub fn cloudchannel_accounts_channel_partner_links_customers_delete(
        &self,
        args: &CloudchannelAccountsChannelPartnerLinksCustomersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_channel_partner_links_customers_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_channel_partner_links_customers_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts channel partner links customers get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudChannelV1Customer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudchannel_accounts_channel_partner_links_customers_get(
        &self,
        args: &CloudchannelAccountsChannelPartnerLinksCustomersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudChannelV1Customer, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_channel_partner_links_customers_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_channel_partner_links_customers_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts channel partner links customers import.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudChannelV1Customer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudchannel_accounts_channel_partner_links_customers_import(
        &self,
        args: &CloudchannelAccountsChannelPartnerLinksCustomersImportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudChannelV1Customer, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_channel_partner_links_customers_import_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_channel_partner_links_customers_import_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts channel partner links customers list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudChannelV1ListCustomersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudchannel_accounts_channel_partner_links_customers_list(
        &self,
        args: &CloudchannelAccountsChannelPartnerLinksCustomersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudChannelV1ListCustomersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_channel_partner_links_customers_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_channel_partner_links_customers_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts channel partner links customers patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudChannelV1Customer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudchannel_accounts_channel_partner_links_customers_patch(
        &self,
        args: &CloudchannelAccountsChannelPartnerLinksCustomersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudChannelV1Customer, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_channel_partner_links_customers_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_channel_partner_links_customers_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts customers create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudChannelV1Customer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudchannel_accounts_customers_create(
        &self,
        args: &CloudchannelAccountsCustomersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudChannelV1Customer, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_customers_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_customers_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts customers delete.
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
    pub fn cloudchannel_accounts_customers_delete(
        &self,
        args: &CloudchannelAccountsCustomersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_customers_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_customers_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts customers get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudChannelV1Customer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudchannel_accounts_customers_get(
        &self,
        args: &CloudchannelAccountsCustomersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudChannelV1Customer, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_customers_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_customers_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts customers import.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudChannelV1Customer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudchannel_accounts_customers_import(
        &self,
        args: &CloudchannelAccountsCustomersImportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudChannelV1Customer, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_customers_import_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_customers_import_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts customers list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudChannelV1ListCustomersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudchannel_accounts_customers_list(
        &self,
        args: &CloudchannelAccountsCustomersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudChannelV1ListCustomersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_customers_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_customers_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts customers list purchasable offers.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudChannelV1ListPurchasableOffersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudchannel_accounts_customers_list_purchasable_offers(
        &self,
        args: &CloudchannelAccountsCustomersListPurchasableOffersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudChannelV1ListPurchasableOffersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_customers_list_purchasable_offers_builder(
            &self.http_client,
            &args.customer,
            &args.changeOfferPurchase_billingAccount,
            &args.changeOfferPurchase_entitlement,
            &args.changeOfferPurchase_newSku,
            &args.createEntitlementPurchase_billingAccount,
            &args.createEntitlementPurchase_sku,
            &args.languageCode,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_customers_list_purchasable_offers_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts customers list purchasable skus.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudChannelV1ListPurchasableSkusResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudchannel_accounts_customers_list_purchasable_skus(
        &self,
        args: &CloudchannelAccountsCustomersListPurchasableSkusArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudChannelV1ListPurchasableSkusResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_customers_list_purchasable_skus_builder(
            &self.http_client,
            &args.customer,
            &args.changeOfferPurchase_changeType,
            &args.changeOfferPurchase_entitlement,
            &args.createEntitlementPurchase_product,
            &args.languageCode,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_customers_list_purchasable_skus_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts customers patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudChannelV1Customer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudchannel_accounts_customers_patch(
        &self,
        args: &CloudchannelAccountsCustomersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudChannelV1Customer, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_customers_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_customers_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts customers provision cloud identity.
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
    pub fn cloudchannel_accounts_customers_provision_cloud_identity(
        &self,
        args: &CloudchannelAccountsCustomersProvisionCloudIdentityArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_customers_provision_cloud_identity_builder(
            &self.http_client,
            &args.customer,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_customers_provision_cloud_identity_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts customers query eligible billing accounts.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudChannelV1QueryEligibleBillingAccountsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudchannel_accounts_customers_query_eligible_billing_accounts(
        &self,
        args: &CloudchannelAccountsCustomersQueryEligibleBillingAccountsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudChannelV1QueryEligibleBillingAccountsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_customers_query_eligible_billing_accounts_builder(
            &self.http_client,
            &args.customer,
            &args.skus,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_customers_query_eligible_billing_accounts_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts customers transfer entitlements.
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
    pub fn cloudchannel_accounts_customers_transfer_entitlements(
        &self,
        args: &CloudchannelAccountsCustomersTransferEntitlementsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_customers_transfer_entitlements_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_customers_transfer_entitlements_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts customers transfer entitlements to google.
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
    pub fn cloudchannel_accounts_customers_transfer_entitlements_to_google(
        &self,
        args: &CloudchannelAccountsCustomersTransferEntitlementsToGoogleArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_customers_transfer_entitlements_to_google_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_customers_transfer_entitlements_to_google_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts customers customer repricing configs create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudChannelV1CustomerRepricingConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudchannel_accounts_customers_customer_repricing_configs_create(
        &self,
        args: &CloudchannelAccountsCustomersCustomerRepricingConfigsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudChannelV1CustomerRepricingConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_customers_customer_repricing_configs_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_customers_customer_repricing_configs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts customers customer repricing configs delete.
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
    pub fn cloudchannel_accounts_customers_customer_repricing_configs_delete(
        &self,
        args: &CloudchannelAccountsCustomersCustomerRepricingConfigsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_customers_customer_repricing_configs_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_customers_customer_repricing_configs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts customers customer repricing configs get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudChannelV1CustomerRepricingConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudchannel_accounts_customers_customer_repricing_configs_get(
        &self,
        args: &CloudchannelAccountsCustomersCustomerRepricingConfigsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudChannelV1CustomerRepricingConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_customers_customer_repricing_configs_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_customers_customer_repricing_configs_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts customers customer repricing configs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudChannelV1ListCustomerRepricingConfigsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudchannel_accounts_customers_customer_repricing_configs_list(
        &self,
        args: &CloudchannelAccountsCustomersCustomerRepricingConfigsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudChannelV1ListCustomerRepricingConfigsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_customers_customer_repricing_configs_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_customers_customer_repricing_configs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts customers customer repricing configs patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudChannelV1CustomerRepricingConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudchannel_accounts_customers_customer_repricing_configs_patch(
        &self,
        args: &CloudchannelAccountsCustomersCustomerRepricingConfigsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudChannelV1CustomerRepricingConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_customers_customer_repricing_configs_patch_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_customers_customer_repricing_configs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts customers entitlements activate.
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
    pub fn cloudchannel_accounts_customers_entitlements_activate(
        &self,
        args: &CloudchannelAccountsCustomersEntitlementsActivateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_customers_entitlements_activate_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_customers_entitlements_activate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts customers entitlements cancel.
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
    pub fn cloudchannel_accounts_customers_entitlements_cancel(
        &self,
        args: &CloudchannelAccountsCustomersEntitlementsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_customers_entitlements_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_customers_entitlements_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts customers entitlements change offer.
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
    pub fn cloudchannel_accounts_customers_entitlements_change_offer(
        &self,
        args: &CloudchannelAccountsCustomersEntitlementsChangeOfferArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_customers_entitlements_change_offer_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_customers_entitlements_change_offer_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts customers entitlements change parameters.
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
    pub fn cloudchannel_accounts_customers_entitlements_change_parameters(
        &self,
        args: &CloudchannelAccountsCustomersEntitlementsChangeParametersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_customers_entitlements_change_parameters_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_customers_entitlements_change_parameters_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts customers entitlements change renewal settings.
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
    pub fn cloudchannel_accounts_customers_entitlements_change_renewal_settings(
        &self,
        args: &CloudchannelAccountsCustomersEntitlementsChangeRenewalSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_customers_entitlements_change_renewal_settings_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_customers_entitlements_change_renewal_settings_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts customers entitlements create.
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
    pub fn cloudchannel_accounts_customers_entitlements_create(
        &self,
        args: &CloudchannelAccountsCustomersEntitlementsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_customers_entitlements_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_customers_entitlements_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts customers entitlements get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudChannelV1Entitlement result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudchannel_accounts_customers_entitlements_get(
        &self,
        args: &CloudchannelAccountsCustomersEntitlementsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudChannelV1Entitlement, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_customers_entitlements_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_customers_entitlements_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts customers entitlements list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudChannelV1ListEntitlementsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudchannel_accounts_customers_entitlements_list(
        &self,
        args: &CloudchannelAccountsCustomersEntitlementsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudChannelV1ListEntitlementsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_customers_entitlements_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_customers_entitlements_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts customers entitlements list entitlement changes.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudChannelV1ListEntitlementChangesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudchannel_accounts_customers_entitlements_list_entitlement_changes(
        &self,
        args: &CloudchannelAccountsCustomersEntitlementsListEntitlementChangesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudChannelV1ListEntitlementChangesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_customers_entitlements_list_entitlement_changes_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_customers_entitlements_list_entitlement_changes_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts customers entitlements lookup offer.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudChannelV1Offer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudchannel_accounts_customers_entitlements_lookup_offer(
        &self,
        args: &CloudchannelAccountsCustomersEntitlementsLookupOfferArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudChannelV1Offer, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_customers_entitlements_lookup_offer_builder(
            &self.http_client,
            &args.entitlement,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_customers_entitlements_lookup_offer_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts customers entitlements start paid service.
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
    pub fn cloudchannel_accounts_customers_entitlements_start_paid_service(
        &self,
        args: &CloudchannelAccountsCustomersEntitlementsStartPaidServiceArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_customers_entitlements_start_paid_service_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_customers_entitlements_start_paid_service_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts customers entitlements suspend.
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
    pub fn cloudchannel_accounts_customers_entitlements_suspend(
        &self,
        args: &CloudchannelAccountsCustomersEntitlementsSuspendArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_customers_entitlements_suspend_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_customers_entitlements_suspend_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts offers list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudChannelV1ListOffersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudchannel_accounts_offers_list(
        &self,
        args: &CloudchannelAccountsOffersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudChannelV1ListOffersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_offers_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.languageCode,
            &args.pageSize,
            &args.pageToken,
            &args.showFutureOffers,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_offers_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts report jobs fetch report results.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudChannelV1FetchReportResultsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudchannel_accounts_report_jobs_fetch_report_results(
        &self,
        args: &CloudchannelAccountsReportJobsFetchReportResultsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudChannelV1FetchReportResultsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_report_jobs_fetch_report_results_builder(
            &self.http_client,
            &args.reportJob,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_report_jobs_fetch_report_results_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts reports list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudChannelV1ListReportsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudchannel_accounts_reports_list(
        &self,
        args: &CloudchannelAccountsReportsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudChannelV1ListReportsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_reports_list_builder(
            &self.http_client,
            &args.parent,
            &args.languageCode,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_reports_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts reports run.
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
    pub fn cloudchannel_accounts_reports_run(
        &self,
        args: &CloudchannelAccountsReportsRunArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_reports_run_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_reports_run_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts sku groups list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudChannelV1ListSkuGroupsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudchannel_accounts_sku_groups_list(
        &self,
        args: &CloudchannelAccountsSkuGroupsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudChannelV1ListSkuGroupsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_sku_groups_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_sku_groups_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel accounts sku groups billable skus list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudChannelV1ListSkuGroupBillableSkusResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudchannel_accounts_sku_groups_billable_skus_list(
        &self,
        args: &CloudchannelAccountsSkuGroupsBillableSkusListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudChannelV1ListSkuGroupBillableSkusResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_accounts_sku_groups_billable_skus_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_accounts_sku_groups_billable_skus_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel integrators list subscribers.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudChannelV1ListSubscribersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudchannel_integrators_list_subscribers(
        &self,
        args: &CloudchannelIntegratorsListSubscribersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudChannelV1ListSubscribersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_integrators_list_subscribers_builder(
            &self.http_client,
            &args.integrator,
            &args.account,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_integrators_list_subscribers_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel integrators register subscriber.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudChannelV1RegisterSubscriberResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudchannel_integrators_register_subscriber(
        &self,
        args: &CloudchannelIntegratorsRegisterSubscriberArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudChannelV1RegisterSubscriberResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_integrators_register_subscriber_builder(
            &self.http_client,
            &args.integrator,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_integrators_register_subscriber_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel integrators unregister subscriber.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudChannelV1UnregisterSubscriberResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudchannel_integrators_unregister_subscriber(
        &self,
        args: &CloudchannelIntegratorsUnregisterSubscriberArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudChannelV1UnregisterSubscriberResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_integrators_unregister_subscriber_builder(
            &self.http_client,
            &args.integrator,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_integrators_unregister_subscriber_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel operations cancel.
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
    pub fn cloudchannel_operations_cancel(
        &self,
        args: &CloudchannelOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel operations delete.
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
    pub fn cloudchannel_operations_delete(
        &self,
        args: &CloudchannelOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel operations get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn cloudchannel_operations_get(
        &self,
        args: &CloudchannelOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel operations list.
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
    pub fn cloudchannel_operations_list(
        &self,
        args: &CloudchannelOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_operations_list_builder(
            &self.http_client,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel products list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudChannelV1ListProductsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudchannel_products_list(
        &self,
        args: &CloudchannelProductsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudChannelV1ListProductsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_products_list_builder(
            &self.http_client,
            &args.account,
            &args.languageCode,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_products_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudchannel products skus list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudChannelV1ListSkusResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudchannel_products_skus_list(
        &self,
        args: &CloudchannelProductsSkusListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudChannelV1ListSkusResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudchannel_products_skus_list_builder(
            &self.http_client,
            &args.parent,
            &args.account,
            &args.languageCode,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudchannel_products_skus_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
