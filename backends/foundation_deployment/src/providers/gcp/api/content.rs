//! ContentProvider - State-aware content API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       content API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::content::{
    content_accounts_authinfo_builder, content_accounts_authinfo_task,
    content_accounts_claimwebsite_builder, content_accounts_claimwebsite_task,
    content_accounts_custombatch_builder, content_accounts_custombatch_task,
    content_accounts_delete_builder, content_accounts_delete_task,
    content_accounts_get_builder, content_accounts_get_task,
    content_accounts_insert_builder, content_accounts_insert_task,
    content_accounts_link_builder, content_accounts_link_task,
    content_accounts_list_builder, content_accounts_list_task,
    content_accounts_listlinks_builder, content_accounts_listlinks_task,
    content_accounts_requestphoneverification_builder, content_accounts_requestphoneverification_task,
    content_accounts_update_builder, content_accounts_update_task,
    content_accounts_updatelabels_builder, content_accounts_updatelabels_task,
    content_accounts_verifyphonenumber_builder, content_accounts_verifyphonenumber_task,
    content_accounts_credentials_create_builder, content_accounts_credentials_create_task,
    content_accounts_labels_create_builder, content_accounts_labels_create_task,
    content_accounts_labels_delete_builder, content_accounts_labels_delete_task,
    content_accounts_labels_list_builder, content_accounts_labels_list_task,
    content_accounts_labels_patch_builder, content_accounts_labels_patch_task,
    content_accounts_returncarrier_create_builder, content_accounts_returncarrier_create_task,
    content_accounts_returncarrier_delete_builder, content_accounts_returncarrier_delete_task,
    content_accounts_returncarrier_list_builder, content_accounts_returncarrier_list_task,
    content_accounts_returncarrier_patch_builder, content_accounts_returncarrier_patch_task,
    content_accountstatuses_custombatch_builder, content_accountstatuses_custombatch_task,
    content_accountstatuses_get_builder, content_accountstatuses_get_task,
    content_accountstatuses_list_builder, content_accountstatuses_list_task,
    content_accounttax_custombatch_builder, content_accounttax_custombatch_task,
    content_accounttax_get_builder, content_accounttax_get_task,
    content_accounttax_list_builder, content_accounttax_list_task,
    content_accounttax_update_builder, content_accounttax_update_task,
    content_collections_create_builder, content_collections_create_task,
    content_collections_delete_builder, content_collections_delete_task,
    content_collections_get_builder, content_collections_get_task,
    content_collections_list_builder, content_collections_list_task,
    content_collectionstatuses_get_builder, content_collectionstatuses_get_task,
    content_collectionstatuses_list_builder, content_collectionstatuses_list_task,
    content_conversionsources_create_builder, content_conversionsources_create_task,
    content_conversionsources_delete_builder, content_conversionsources_delete_task,
    content_conversionsources_get_builder, content_conversionsources_get_task,
    content_conversionsources_list_builder, content_conversionsources_list_task,
    content_conversionsources_patch_builder, content_conversionsources_patch_task,
    content_conversionsources_undelete_builder, content_conversionsources_undelete_task,
    content_csses_get_builder, content_csses_get_task,
    content_csses_list_builder, content_csses_list_task,
    content_csses_updatelabels_builder, content_csses_updatelabels_task,
    content_datafeeds_custombatch_builder, content_datafeeds_custombatch_task,
    content_datafeeds_delete_builder, content_datafeeds_delete_task,
    content_datafeeds_fetchnow_builder, content_datafeeds_fetchnow_task,
    content_datafeeds_get_builder, content_datafeeds_get_task,
    content_datafeeds_insert_builder, content_datafeeds_insert_task,
    content_datafeeds_list_builder, content_datafeeds_list_task,
    content_datafeeds_update_builder, content_datafeeds_update_task,
    content_datafeedstatuses_custombatch_builder, content_datafeedstatuses_custombatch_task,
    content_datafeedstatuses_get_builder, content_datafeedstatuses_get_task,
    content_datafeedstatuses_list_builder, content_datafeedstatuses_list_task,
    content_freelistingsprogram_get_builder, content_freelistingsprogram_get_task,
    content_freelistingsprogram_requestreview_builder, content_freelistingsprogram_requestreview_task,
    content_freelistingsprogram_checkoutsettings_delete_builder, content_freelistingsprogram_checkoutsettings_delete_task,
    content_freelistingsprogram_checkoutsettings_get_builder, content_freelistingsprogram_checkoutsettings_get_task,
    content_freelistingsprogram_checkoutsettings_insert_builder, content_freelistingsprogram_checkoutsettings_insert_task,
    content_liasettings_custombatch_builder, content_liasettings_custombatch_task,
    content_liasettings_get_builder, content_liasettings_get_task,
    content_liasettings_getaccessiblegmbaccounts_builder, content_liasettings_getaccessiblegmbaccounts_task,
    content_liasettings_list_builder, content_liasettings_list_task,
    content_liasettings_listposdataproviders_builder, content_liasettings_listposdataproviders_task,
    content_liasettings_requestgmbaccess_builder, content_liasettings_requestgmbaccess_task,
    content_liasettings_requestinventoryverification_builder, content_liasettings_requestinventoryverification_task,
    content_liasettings_setinventoryverificationcontact_builder, content_liasettings_setinventoryverificationcontact_task,
    content_liasettings_setomnichannelexperience_builder, content_liasettings_setomnichannelexperience_task,
    content_liasettings_setposdataprovider_builder, content_liasettings_setposdataprovider_task,
    content_liasettings_update_builder, content_liasettings_update_task,
    content_localinventory_custombatch_builder, content_localinventory_custombatch_task,
    content_localinventory_insert_builder, content_localinventory_insert_task,
    content_merchantsupport_renderaccountissues_builder, content_merchantsupport_renderaccountissues_task,
    content_merchantsupport_renderproductissues_builder, content_merchantsupport_renderproductissues_task,
    content_merchantsupport_triggeraction_builder, content_merchantsupport_triggeraction_task,
    content_ordertrackingsignals_create_builder, content_ordertrackingsignals_create_task,
    content_pos_custombatch_builder, content_pos_custombatch_task,
    content_pos_delete_builder, content_pos_delete_task,
    content_pos_get_builder, content_pos_get_task,
    content_pos_insert_builder, content_pos_insert_task,
    content_pos_inventory_builder, content_pos_inventory_task,
    content_pos_list_builder, content_pos_list_task,
    content_pos_sale_builder, content_pos_sale_task,
    content_productdeliverytime_create_builder, content_productdeliverytime_create_task,
    content_productdeliverytime_delete_builder, content_productdeliverytime_delete_task,
    content_productdeliverytime_get_builder, content_productdeliverytime_get_task,
    content_products_custombatch_builder, content_products_custombatch_task,
    content_products_delete_builder, content_products_delete_task,
    content_products_get_builder, content_products_get_task,
    content_products_insert_builder, content_products_insert_task,
    content_products_list_builder, content_products_list_task,
    content_products_update_builder, content_products_update_task,
    content_productstatuses_custombatch_builder, content_productstatuses_custombatch_task,
    content_productstatuses_get_builder, content_productstatuses_get_task,
    content_productstatuses_list_builder, content_productstatuses_list_task,
    content_promotions_create_builder, content_promotions_create_task,
    content_promotions_get_builder, content_promotions_get_task,
    content_promotions_list_builder, content_promotions_list_task,
    content_pubsubnotificationsettings_get_builder, content_pubsubnotificationsettings_get_task,
    content_pubsubnotificationsettings_update_builder, content_pubsubnotificationsettings_update_task,
    content_quotas_list_builder, content_quotas_list_task,
    content_recommendations_generate_builder, content_recommendations_generate_task,
    content_recommendations_report_interaction_builder, content_recommendations_report_interaction_task,
    content_regionalinventory_custombatch_builder, content_regionalinventory_custombatch_task,
    content_regionalinventory_insert_builder, content_regionalinventory_insert_task,
    content_regions_create_builder, content_regions_create_task,
    content_regions_delete_builder, content_regions_delete_task,
    content_regions_get_builder, content_regions_get_task,
    content_regions_list_builder, content_regions_list_task,
    content_regions_patch_builder, content_regions_patch_task,
    content_reports_search_builder, content_reports_search_task,
    content_returnpolicyonline_create_builder, content_returnpolicyonline_create_task,
    content_returnpolicyonline_delete_builder, content_returnpolicyonline_delete_task,
    content_returnpolicyonline_get_builder, content_returnpolicyonline_get_task,
    content_returnpolicyonline_list_builder, content_returnpolicyonline_list_task,
    content_returnpolicyonline_patch_builder, content_returnpolicyonline_patch_task,
    content_shippingsettings_custombatch_builder, content_shippingsettings_custombatch_task,
    content_shippingsettings_get_builder, content_shippingsettings_get_task,
    content_shippingsettings_getsupportedcarriers_builder, content_shippingsettings_getsupportedcarriers_task,
    content_shippingsettings_getsupportedholidays_builder, content_shippingsettings_getsupportedholidays_task,
    content_shippingsettings_getsupportedpickupservices_builder, content_shippingsettings_getsupportedpickupservices_task,
    content_shippingsettings_list_builder, content_shippingsettings_list_task,
    content_shippingsettings_update_builder, content_shippingsettings_update_task,
    content_shoppingadsprogram_get_builder, content_shoppingadsprogram_get_task,
    content_shoppingadsprogram_requestreview_builder, content_shoppingadsprogram_requestreview_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::content::Account;
use crate::providers::gcp::clients::content::AccountCredentials;
use crate::providers::gcp::clients::content::AccountLabel;
use crate::providers::gcp::clients::content::AccountReturnCarrier;
use crate::providers::gcp::clients::content::AccountStatus;
use crate::providers::gcp::clients::content::AccountTax;
use crate::providers::gcp::clients::content::AccountsAuthInfoResponse;
use crate::providers::gcp::clients::content::AccountsClaimWebsiteResponse;
use crate::providers::gcp::clients::content::AccountsCustomBatchResponse;
use crate::providers::gcp::clients::content::AccountsLinkResponse;
use crate::providers::gcp::clients::content::AccountsListLinksResponse;
use crate::providers::gcp::clients::content::AccountsListResponse;
use crate::providers::gcp::clients::content::AccountsUpdateLabelsResponse;
use crate::providers::gcp::clients::content::AccountstatusesCustomBatchResponse;
use crate::providers::gcp::clients::content::AccountstatusesListResponse;
use crate::providers::gcp::clients::content::AccounttaxCustomBatchResponse;
use crate::providers::gcp::clients::content::AccounttaxListResponse;
use crate::providers::gcp::clients::content::CheckoutSettings;
use crate::providers::gcp::clients::content::Collection;
use crate::providers::gcp::clients::content::CollectionStatus;
use crate::providers::gcp::clients::content::ConversionSource;
use crate::providers::gcp::clients::content::Css;
use crate::providers::gcp::clients::content::Datafeed;
use crate::providers::gcp::clients::content::DatafeedStatus;
use crate::providers::gcp::clients::content::DatafeedsCustomBatchResponse;
use crate::providers::gcp::clients::content::DatafeedsFetchNowResponse;
use crate::providers::gcp::clients::content::DatafeedsListResponse;
use crate::providers::gcp::clients::content::DatafeedstatusesCustomBatchResponse;
use crate::providers::gcp::clients::content::DatafeedstatusesListResponse;
use crate::providers::gcp::clients::content::FreeListingsProgramStatus;
use crate::providers::gcp::clients::content::GenerateRecommendationsResponse;
use crate::providers::gcp::clients::content::LiaOmnichannelExperience;
use crate::providers::gcp::clients::content::LiaSettings;
use crate::providers::gcp::clients::content::LiasettingsCustomBatchResponse;
use crate::providers::gcp::clients::content::LiasettingsGetAccessibleGmbAccountsResponse;
use crate::providers::gcp::clients::content::LiasettingsListPosDataProvidersResponse;
use crate::providers::gcp::clients::content::LiasettingsListResponse;
use crate::providers::gcp::clients::content::LiasettingsRequestGmbAccessResponse;
use crate::providers::gcp::clients::content::LiasettingsRequestInventoryVerificationResponse;
use crate::providers::gcp::clients::content::LiasettingsSetInventoryVerificationContactResponse;
use crate::providers::gcp::clients::content::LiasettingsSetPosDataProviderResponse;
use crate::providers::gcp::clients::content::ListAccountLabelsResponse;
use crate::providers::gcp::clients::content::ListAccountReturnCarrierResponse;
use crate::providers::gcp::clients::content::ListCollectionStatusesResponse;
use crate::providers::gcp::clients::content::ListCollectionsResponse;
use crate::providers::gcp::clients::content::ListConversionSourcesResponse;
use crate::providers::gcp::clients::content::ListCssesResponse;
use crate::providers::gcp::clients::content::ListMethodQuotasResponse;
use crate::providers::gcp::clients::content::ListPromotionResponse;
use crate::providers::gcp::clients::content::ListRegionsResponse;
use crate::providers::gcp::clients::content::ListReturnPolicyOnlineResponse;
use crate::providers::gcp::clients::content::LocalInventory;
use crate::providers::gcp::clients::content::LocalinventoryCustomBatchResponse;
use crate::providers::gcp::clients::content::OrderTrackingSignal;
use crate::providers::gcp::clients::content::PosCustomBatchResponse;
use crate::providers::gcp::clients::content::PosInventoryResponse;
use crate::providers::gcp::clients::content::PosListResponse;
use crate::providers::gcp::clients::content::PosSaleResponse;
use crate::providers::gcp::clients::content::PosStore;
use crate::providers::gcp::clients::content::Product;
use crate::providers::gcp::clients::content::ProductDeliveryTime;
use crate::providers::gcp::clients::content::ProductStatus;
use crate::providers::gcp::clients::content::ProductsCustomBatchResponse;
use crate::providers::gcp::clients::content::ProductsListResponse;
use crate::providers::gcp::clients::content::ProductstatusesCustomBatchResponse;
use crate::providers::gcp::clients::content::ProductstatusesListResponse;
use crate::providers::gcp::clients::content::Promotion;
use crate::providers::gcp::clients::content::PubsubNotificationSettings;
use crate::providers::gcp::clients::content::Region;
use crate::providers::gcp::clients::content::RegionalInventory;
use crate::providers::gcp::clients::content::RegionalinventoryCustomBatchResponse;
use crate::providers::gcp::clients::content::RenderAccountIssuesResponse;
use crate::providers::gcp::clients::content::RenderProductIssuesResponse;
use crate::providers::gcp::clients::content::RequestPhoneVerificationResponse;
use crate::providers::gcp::clients::content::ReturnPolicyOnline;
use crate::providers::gcp::clients::content::SearchResponse;
use crate::providers::gcp::clients::content::ShippingSettings;
use crate::providers::gcp::clients::content::ShippingsettingsCustomBatchResponse;
use crate::providers::gcp::clients::content::ShippingsettingsGetSupportedCarriersResponse;
use crate::providers::gcp::clients::content::ShippingsettingsGetSupportedHolidaysResponse;
use crate::providers::gcp::clients::content::ShippingsettingsGetSupportedPickupServicesResponse;
use crate::providers::gcp::clients::content::ShippingsettingsListResponse;
use crate::providers::gcp::clients::content::ShoppingAdsProgramStatus;
use crate::providers::gcp::clients::content::TriggerActionResponse;
use crate::providers::gcp::clients::content::VerifyPhoneNumberResponse;
use crate::providers::gcp::clients::content::ContentAccountsAuthinfoArgs;
use crate::providers::gcp::clients::content::ContentAccountsClaimwebsiteArgs;
use crate::providers::gcp::clients::content::ContentAccountsCredentialsCreateArgs;
use crate::providers::gcp::clients::content::ContentAccountsCustombatchArgs;
use crate::providers::gcp::clients::content::ContentAccountsDeleteArgs;
use crate::providers::gcp::clients::content::ContentAccountsGetArgs;
use crate::providers::gcp::clients::content::ContentAccountsInsertArgs;
use crate::providers::gcp::clients::content::ContentAccountsLabelsCreateArgs;
use crate::providers::gcp::clients::content::ContentAccountsLabelsDeleteArgs;
use crate::providers::gcp::clients::content::ContentAccountsLabelsListArgs;
use crate::providers::gcp::clients::content::ContentAccountsLabelsPatchArgs;
use crate::providers::gcp::clients::content::ContentAccountsLinkArgs;
use crate::providers::gcp::clients::content::ContentAccountsListArgs;
use crate::providers::gcp::clients::content::ContentAccountsListlinksArgs;
use crate::providers::gcp::clients::content::ContentAccountsRequestphoneverificationArgs;
use crate::providers::gcp::clients::content::ContentAccountsReturncarrierCreateArgs;
use crate::providers::gcp::clients::content::ContentAccountsReturncarrierDeleteArgs;
use crate::providers::gcp::clients::content::ContentAccountsReturncarrierListArgs;
use crate::providers::gcp::clients::content::ContentAccountsReturncarrierPatchArgs;
use crate::providers::gcp::clients::content::ContentAccountsUpdateArgs;
use crate::providers::gcp::clients::content::ContentAccountsUpdatelabelsArgs;
use crate::providers::gcp::clients::content::ContentAccountsVerifyphonenumberArgs;
use crate::providers::gcp::clients::content::ContentAccountstatusesCustombatchArgs;
use crate::providers::gcp::clients::content::ContentAccountstatusesGetArgs;
use crate::providers::gcp::clients::content::ContentAccountstatusesListArgs;
use crate::providers::gcp::clients::content::ContentAccounttaxCustombatchArgs;
use crate::providers::gcp::clients::content::ContentAccounttaxGetArgs;
use crate::providers::gcp::clients::content::ContentAccounttaxListArgs;
use crate::providers::gcp::clients::content::ContentAccounttaxUpdateArgs;
use crate::providers::gcp::clients::content::ContentCollectionsCreateArgs;
use crate::providers::gcp::clients::content::ContentCollectionsDeleteArgs;
use crate::providers::gcp::clients::content::ContentCollectionsGetArgs;
use crate::providers::gcp::clients::content::ContentCollectionsListArgs;
use crate::providers::gcp::clients::content::ContentCollectionstatusesGetArgs;
use crate::providers::gcp::clients::content::ContentCollectionstatusesListArgs;
use crate::providers::gcp::clients::content::ContentConversionsourcesCreateArgs;
use crate::providers::gcp::clients::content::ContentConversionsourcesDeleteArgs;
use crate::providers::gcp::clients::content::ContentConversionsourcesGetArgs;
use crate::providers::gcp::clients::content::ContentConversionsourcesListArgs;
use crate::providers::gcp::clients::content::ContentConversionsourcesPatchArgs;
use crate::providers::gcp::clients::content::ContentConversionsourcesUndeleteArgs;
use crate::providers::gcp::clients::content::ContentCssesGetArgs;
use crate::providers::gcp::clients::content::ContentCssesListArgs;
use crate::providers::gcp::clients::content::ContentCssesUpdatelabelsArgs;
use crate::providers::gcp::clients::content::ContentDatafeedsCustombatchArgs;
use crate::providers::gcp::clients::content::ContentDatafeedsDeleteArgs;
use crate::providers::gcp::clients::content::ContentDatafeedsFetchnowArgs;
use crate::providers::gcp::clients::content::ContentDatafeedsGetArgs;
use crate::providers::gcp::clients::content::ContentDatafeedsInsertArgs;
use crate::providers::gcp::clients::content::ContentDatafeedsListArgs;
use crate::providers::gcp::clients::content::ContentDatafeedsUpdateArgs;
use crate::providers::gcp::clients::content::ContentDatafeedstatusesCustombatchArgs;
use crate::providers::gcp::clients::content::ContentDatafeedstatusesGetArgs;
use crate::providers::gcp::clients::content::ContentDatafeedstatusesListArgs;
use crate::providers::gcp::clients::content::ContentFreelistingsprogramCheckoutsettingsDeleteArgs;
use crate::providers::gcp::clients::content::ContentFreelistingsprogramCheckoutsettingsGetArgs;
use crate::providers::gcp::clients::content::ContentFreelistingsprogramCheckoutsettingsInsertArgs;
use crate::providers::gcp::clients::content::ContentFreelistingsprogramGetArgs;
use crate::providers::gcp::clients::content::ContentFreelistingsprogramRequestreviewArgs;
use crate::providers::gcp::clients::content::ContentLiasettingsCustombatchArgs;
use crate::providers::gcp::clients::content::ContentLiasettingsGetArgs;
use crate::providers::gcp::clients::content::ContentLiasettingsGetaccessiblegmbaccountsArgs;
use crate::providers::gcp::clients::content::ContentLiasettingsListArgs;
use crate::providers::gcp::clients::content::ContentLiasettingsListposdataprovidersArgs;
use crate::providers::gcp::clients::content::ContentLiasettingsRequestgmbaccessArgs;
use crate::providers::gcp::clients::content::ContentLiasettingsRequestinventoryverificationArgs;
use crate::providers::gcp::clients::content::ContentLiasettingsSetinventoryverificationcontactArgs;
use crate::providers::gcp::clients::content::ContentLiasettingsSetomnichannelexperienceArgs;
use crate::providers::gcp::clients::content::ContentLiasettingsSetposdataproviderArgs;
use crate::providers::gcp::clients::content::ContentLiasettingsUpdateArgs;
use crate::providers::gcp::clients::content::ContentLocalinventoryCustombatchArgs;
use crate::providers::gcp::clients::content::ContentLocalinventoryInsertArgs;
use crate::providers::gcp::clients::content::ContentMerchantsupportRenderaccountissuesArgs;
use crate::providers::gcp::clients::content::ContentMerchantsupportRenderproductissuesArgs;
use crate::providers::gcp::clients::content::ContentMerchantsupportTriggeractionArgs;
use crate::providers::gcp::clients::content::ContentOrdertrackingsignalsCreateArgs;
use crate::providers::gcp::clients::content::ContentPosCustombatchArgs;
use crate::providers::gcp::clients::content::ContentPosDeleteArgs;
use crate::providers::gcp::clients::content::ContentPosGetArgs;
use crate::providers::gcp::clients::content::ContentPosInsertArgs;
use crate::providers::gcp::clients::content::ContentPosInventoryArgs;
use crate::providers::gcp::clients::content::ContentPosListArgs;
use crate::providers::gcp::clients::content::ContentPosSaleArgs;
use crate::providers::gcp::clients::content::ContentProductdeliverytimeCreateArgs;
use crate::providers::gcp::clients::content::ContentProductdeliverytimeDeleteArgs;
use crate::providers::gcp::clients::content::ContentProductdeliverytimeGetArgs;
use crate::providers::gcp::clients::content::ContentProductsCustombatchArgs;
use crate::providers::gcp::clients::content::ContentProductsDeleteArgs;
use crate::providers::gcp::clients::content::ContentProductsGetArgs;
use crate::providers::gcp::clients::content::ContentProductsInsertArgs;
use crate::providers::gcp::clients::content::ContentProductsListArgs;
use crate::providers::gcp::clients::content::ContentProductsUpdateArgs;
use crate::providers::gcp::clients::content::ContentProductstatusesCustombatchArgs;
use crate::providers::gcp::clients::content::ContentProductstatusesGetArgs;
use crate::providers::gcp::clients::content::ContentProductstatusesListArgs;
use crate::providers::gcp::clients::content::ContentPromotionsCreateArgs;
use crate::providers::gcp::clients::content::ContentPromotionsGetArgs;
use crate::providers::gcp::clients::content::ContentPromotionsListArgs;
use crate::providers::gcp::clients::content::ContentPubsubnotificationsettingsGetArgs;
use crate::providers::gcp::clients::content::ContentPubsubnotificationsettingsUpdateArgs;
use crate::providers::gcp::clients::content::ContentQuotasListArgs;
use crate::providers::gcp::clients::content::ContentRecommendationsGenerateArgs;
use crate::providers::gcp::clients::content::ContentRecommendationsReportInteractionArgs;
use crate::providers::gcp::clients::content::ContentRegionalinventoryCustombatchArgs;
use crate::providers::gcp::clients::content::ContentRegionalinventoryInsertArgs;
use crate::providers::gcp::clients::content::ContentRegionsCreateArgs;
use crate::providers::gcp::clients::content::ContentRegionsDeleteArgs;
use crate::providers::gcp::clients::content::ContentRegionsGetArgs;
use crate::providers::gcp::clients::content::ContentRegionsListArgs;
use crate::providers::gcp::clients::content::ContentRegionsPatchArgs;
use crate::providers::gcp::clients::content::ContentReportsSearchArgs;
use crate::providers::gcp::clients::content::ContentReturnpolicyonlineCreateArgs;
use crate::providers::gcp::clients::content::ContentReturnpolicyonlineDeleteArgs;
use crate::providers::gcp::clients::content::ContentReturnpolicyonlineGetArgs;
use crate::providers::gcp::clients::content::ContentReturnpolicyonlineListArgs;
use crate::providers::gcp::clients::content::ContentReturnpolicyonlinePatchArgs;
use crate::providers::gcp::clients::content::ContentShippingsettingsCustombatchArgs;
use crate::providers::gcp::clients::content::ContentShippingsettingsGetArgs;
use crate::providers::gcp::clients::content::ContentShippingsettingsGetsupportedcarriersArgs;
use crate::providers::gcp::clients::content::ContentShippingsettingsGetsupportedholidaysArgs;
use crate::providers::gcp::clients::content::ContentShippingsettingsGetsupportedpickupservicesArgs;
use crate::providers::gcp::clients::content::ContentShippingsettingsListArgs;
use crate::providers::gcp::clients::content::ContentShippingsettingsUpdateArgs;
use crate::providers::gcp::clients::content::ContentShoppingadsprogramGetArgs;
use crate::providers::gcp::clients::content::ContentShoppingadsprogramRequestreviewArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// ContentProvider with automatic state tracking.
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
/// let provider = ContentProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct ContentProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> ContentProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new ContentProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Content accounts authinfo.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccountsAuthInfoResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_accounts_authinfo(
        &self,
        args: &ContentAccountsAuthinfoArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccountsAuthInfoResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_accounts_authinfo_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = content_accounts_authinfo_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content accounts claimwebsite.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccountsClaimWebsiteResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_accounts_claimwebsite(
        &self,
        args: &ContentAccountsClaimwebsiteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccountsClaimWebsiteResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_accounts_claimwebsite_builder(
            &self.http_client,
            &args.merchantId,
            &args.accountId,
            &args.overwrite,
        )
        .map_err(ProviderError::Api)?;

        let task = content_accounts_claimwebsite_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content accounts custombatch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccountsCustomBatchResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_accounts_custombatch(
        &self,
        args: &ContentAccountsCustombatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccountsCustomBatchResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_accounts_custombatch_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = content_accounts_custombatch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content accounts delete.
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
    pub fn content_accounts_delete(
        &self,
        args: &ContentAccountsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_accounts_delete_builder(
            &self.http_client,
            &args.merchantId,
            &args.accountId,
            &args.force,
        )
        .map_err(ProviderError::Api)?;

        let task = content_accounts_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content accounts get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Account result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_accounts_get(
        &self,
        args: &ContentAccountsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Account, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_accounts_get_builder(
            &self.http_client,
            &args.merchantId,
            &args.accountId,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = content_accounts_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content accounts insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Account result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_accounts_insert(
        &self,
        args: &ContentAccountsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Account, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_accounts_insert_builder(
            &self.http_client,
            &args.merchantId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_accounts_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content accounts link.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccountsLinkResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_accounts_link(
        &self,
        args: &ContentAccountsLinkArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccountsLinkResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_accounts_link_builder(
            &self.http_client,
            &args.merchantId,
            &args.accountId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_accounts_link_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content accounts list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccountsListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_accounts_list(
        &self,
        args: &ContentAccountsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccountsListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_accounts_list_builder(
            &self.http_client,
            &args.merchantId,
            &args.label,
            &args.maxResults,
            &args.name,
            &args.pageToken,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = content_accounts_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content accounts listlinks.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccountsListLinksResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_accounts_listlinks(
        &self,
        args: &ContentAccountsListlinksArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccountsListLinksResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_accounts_listlinks_builder(
            &self.http_client,
            &args.merchantId,
            &args.accountId,
            &args.maxResults,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = content_accounts_listlinks_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content accounts requestphoneverification.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RequestPhoneVerificationResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_accounts_requestphoneverification(
        &self,
        args: &ContentAccountsRequestphoneverificationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RequestPhoneVerificationResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_accounts_requestphoneverification_builder(
            &self.http_client,
            &args.merchantId,
            &args.accountId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_accounts_requestphoneverification_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content accounts update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Account result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_accounts_update(
        &self,
        args: &ContentAccountsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Account, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_accounts_update_builder(
            &self.http_client,
            &args.merchantId,
            &args.accountId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_accounts_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content accounts updatelabels.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccountsUpdateLabelsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_accounts_updatelabels(
        &self,
        args: &ContentAccountsUpdatelabelsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccountsUpdateLabelsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_accounts_updatelabels_builder(
            &self.http_client,
            &args.merchantId,
            &args.accountId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_accounts_updatelabels_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content accounts verifyphonenumber.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the VerifyPhoneNumberResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_accounts_verifyphonenumber(
        &self,
        args: &ContentAccountsVerifyphonenumberArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<VerifyPhoneNumberResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_accounts_verifyphonenumber_builder(
            &self.http_client,
            &args.merchantId,
            &args.accountId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_accounts_verifyphonenumber_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content accounts credentials create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccountCredentials result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_accounts_credentials_create(
        &self,
        args: &ContentAccountsCredentialsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccountCredentials, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_accounts_credentials_create_builder(
            &self.http_client,
            &args.accountId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_accounts_credentials_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content accounts labels create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccountLabel result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_accounts_labels_create(
        &self,
        args: &ContentAccountsLabelsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccountLabel, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_accounts_labels_create_builder(
            &self.http_client,
            &args.accountId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_accounts_labels_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content accounts labels delete.
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
    pub fn content_accounts_labels_delete(
        &self,
        args: &ContentAccountsLabelsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_accounts_labels_delete_builder(
            &self.http_client,
            &args.accountId,
            &args.labelId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_accounts_labels_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content accounts labels list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAccountLabelsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_accounts_labels_list(
        &self,
        args: &ContentAccountsLabelsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAccountLabelsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_accounts_labels_list_builder(
            &self.http_client,
            &args.accountId,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = content_accounts_labels_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content accounts labels patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccountLabel result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_accounts_labels_patch(
        &self,
        args: &ContentAccountsLabelsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccountLabel, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_accounts_labels_patch_builder(
            &self.http_client,
            &args.accountId,
            &args.labelId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_accounts_labels_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content accounts returncarrier create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccountReturnCarrier result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_accounts_returncarrier_create(
        &self,
        args: &ContentAccountsReturncarrierCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccountReturnCarrier, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_accounts_returncarrier_create_builder(
            &self.http_client,
            &args.accountId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_accounts_returncarrier_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content accounts returncarrier delete.
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
    pub fn content_accounts_returncarrier_delete(
        &self,
        args: &ContentAccountsReturncarrierDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_accounts_returncarrier_delete_builder(
            &self.http_client,
            &args.accountId,
            &args.carrierAccountId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_accounts_returncarrier_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content accounts returncarrier list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAccountReturnCarrierResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_accounts_returncarrier_list(
        &self,
        args: &ContentAccountsReturncarrierListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAccountReturnCarrierResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_accounts_returncarrier_list_builder(
            &self.http_client,
            &args.accountId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_accounts_returncarrier_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content accounts returncarrier patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccountReturnCarrier result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_accounts_returncarrier_patch(
        &self,
        args: &ContentAccountsReturncarrierPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccountReturnCarrier, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_accounts_returncarrier_patch_builder(
            &self.http_client,
            &args.accountId,
            &args.carrierAccountId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_accounts_returncarrier_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content accountstatuses custombatch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccountstatusesCustomBatchResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_accountstatuses_custombatch(
        &self,
        args: &ContentAccountstatusesCustombatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccountstatusesCustomBatchResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_accountstatuses_custombatch_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = content_accountstatuses_custombatch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content accountstatuses get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccountStatus result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_accountstatuses_get(
        &self,
        args: &ContentAccountstatusesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccountStatus, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_accountstatuses_get_builder(
            &self.http_client,
            &args.merchantId,
            &args.accountId,
            &args.destinations,
        )
        .map_err(ProviderError::Api)?;

        let task = content_accountstatuses_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content accountstatuses list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccountstatusesListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_accountstatuses_list(
        &self,
        args: &ContentAccountstatusesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccountstatusesListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_accountstatuses_list_builder(
            &self.http_client,
            &args.merchantId,
            &args.destinations,
            &args.maxResults,
            &args.name,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = content_accountstatuses_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content accounttax custombatch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccounttaxCustomBatchResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_accounttax_custombatch(
        &self,
        args: &ContentAccounttaxCustombatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccounttaxCustomBatchResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_accounttax_custombatch_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = content_accounttax_custombatch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content accounttax get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccountTax result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_accounttax_get(
        &self,
        args: &ContentAccounttaxGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccountTax, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_accounttax_get_builder(
            &self.http_client,
            &args.merchantId,
            &args.accountId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_accounttax_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content accounttax list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccounttaxListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_accounttax_list(
        &self,
        args: &ContentAccounttaxListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccounttaxListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_accounttax_list_builder(
            &self.http_client,
            &args.merchantId,
            &args.maxResults,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = content_accounttax_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content accounttax update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccountTax result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_accounttax_update(
        &self,
        args: &ContentAccounttaxUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccountTax, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_accounttax_update_builder(
            &self.http_client,
            &args.merchantId,
            &args.accountId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_accounttax_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content collections create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Collection result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_collections_create(
        &self,
        args: &ContentCollectionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Collection, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_collections_create_builder(
            &self.http_client,
            &args.merchantId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_collections_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content collections delete.
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
    pub fn content_collections_delete(
        &self,
        args: &ContentCollectionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_collections_delete_builder(
            &self.http_client,
            &args.merchantId,
            &args.collectionId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_collections_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content collections get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Collection result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_collections_get(
        &self,
        args: &ContentCollectionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Collection, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_collections_get_builder(
            &self.http_client,
            &args.merchantId,
            &args.collectionId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_collections_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content collections list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCollectionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_collections_list(
        &self,
        args: &ContentCollectionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCollectionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_collections_list_builder(
            &self.http_client,
            &args.merchantId,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = content_collections_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content collectionstatuses get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CollectionStatus result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_collectionstatuses_get(
        &self,
        args: &ContentCollectionstatusesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CollectionStatus, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_collectionstatuses_get_builder(
            &self.http_client,
            &args.merchantId,
            &args.collectionId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_collectionstatuses_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content collectionstatuses list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCollectionStatusesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_collectionstatuses_list(
        &self,
        args: &ContentCollectionstatusesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCollectionStatusesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_collectionstatuses_list_builder(
            &self.http_client,
            &args.merchantId,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = content_collectionstatuses_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content conversionsources create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ConversionSource result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_conversionsources_create(
        &self,
        args: &ContentConversionsourcesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ConversionSource, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_conversionsources_create_builder(
            &self.http_client,
            &args.merchantId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_conversionsources_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content conversionsources delete.
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
    pub fn content_conversionsources_delete(
        &self,
        args: &ContentConversionsourcesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_conversionsources_delete_builder(
            &self.http_client,
            &args.merchantId,
            &args.conversionSourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_conversionsources_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content conversionsources get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ConversionSource result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_conversionsources_get(
        &self,
        args: &ContentConversionsourcesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ConversionSource, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_conversionsources_get_builder(
            &self.http_client,
            &args.merchantId,
            &args.conversionSourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_conversionsources_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content conversionsources list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListConversionSourcesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_conversionsources_list(
        &self,
        args: &ContentConversionsourcesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListConversionSourcesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_conversionsources_list_builder(
            &self.http_client,
            &args.merchantId,
            &args.pageSize,
            &args.pageToken,
            &args.showDeleted,
        )
        .map_err(ProviderError::Api)?;

        let task = content_conversionsources_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content conversionsources patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ConversionSource result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_conversionsources_patch(
        &self,
        args: &ContentConversionsourcesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ConversionSource, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_conversionsources_patch_builder(
            &self.http_client,
            &args.merchantId,
            &args.conversionSourceId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = content_conversionsources_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content conversionsources undelete.
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
    pub fn content_conversionsources_undelete(
        &self,
        args: &ContentConversionsourcesUndeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_conversionsources_undelete_builder(
            &self.http_client,
            &args.merchantId,
            &args.conversionSourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_conversionsources_undelete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content csses get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Css result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_csses_get(
        &self,
        args: &ContentCssesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Css, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_csses_get_builder(
            &self.http_client,
            &args.cssGroupId,
            &args.cssDomainId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_csses_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content csses list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCssesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_csses_list(
        &self,
        args: &ContentCssesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCssesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_csses_list_builder(
            &self.http_client,
            &args.cssGroupId,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = content_csses_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content csses updatelabels.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Css result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_csses_updatelabels(
        &self,
        args: &ContentCssesUpdatelabelsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Css, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_csses_updatelabels_builder(
            &self.http_client,
            &args.cssGroupId,
            &args.cssDomainId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_csses_updatelabels_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content datafeeds custombatch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DatafeedsCustomBatchResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_datafeeds_custombatch(
        &self,
        args: &ContentDatafeedsCustombatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DatafeedsCustomBatchResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_datafeeds_custombatch_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = content_datafeeds_custombatch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content datafeeds delete.
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
    pub fn content_datafeeds_delete(
        &self,
        args: &ContentDatafeedsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_datafeeds_delete_builder(
            &self.http_client,
            &args.merchantId,
            &args.datafeedId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_datafeeds_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content datafeeds fetchnow.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DatafeedsFetchNowResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_datafeeds_fetchnow(
        &self,
        args: &ContentDatafeedsFetchnowArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DatafeedsFetchNowResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_datafeeds_fetchnow_builder(
            &self.http_client,
            &args.merchantId,
            &args.datafeedId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_datafeeds_fetchnow_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content datafeeds get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Datafeed result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_datafeeds_get(
        &self,
        args: &ContentDatafeedsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Datafeed, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_datafeeds_get_builder(
            &self.http_client,
            &args.merchantId,
            &args.datafeedId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_datafeeds_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content datafeeds insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Datafeed result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_datafeeds_insert(
        &self,
        args: &ContentDatafeedsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Datafeed, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_datafeeds_insert_builder(
            &self.http_client,
            &args.merchantId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_datafeeds_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content datafeeds list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DatafeedsListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_datafeeds_list(
        &self,
        args: &ContentDatafeedsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DatafeedsListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_datafeeds_list_builder(
            &self.http_client,
            &args.merchantId,
            &args.maxResults,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = content_datafeeds_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content datafeeds update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Datafeed result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_datafeeds_update(
        &self,
        args: &ContentDatafeedsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Datafeed, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_datafeeds_update_builder(
            &self.http_client,
            &args.merchantId,
            &args.datafeedId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_datafeeds_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content datafeedstatuses custombatch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DatafeedstatusesCustomBatchResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_datafeedstatuses_custombatch(
        &self,
        args: &ContentDatafeedstatusesCustombatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DatafeedstatusesCustomBatchResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_datafeedstatuses_custombatch_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = content_datafeedstatuses_custombatch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content datafeedstatuses get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DatafeedStatus result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_datafeedstatuses_get(
        &self,
        args: &ContentDatafeedstatusesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DatafeedStatus, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_datafeedstatuses_get_builder(
            &self.http_client,
            &args.merchantId,
            &args.datafeedId,
            &args.country,
            &args.feedLabel,
            &args.language,
        )
        .map_err(ProviderError::Api)?;

        let task = content_datafeedstatuses_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content datafeedstatuses list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DatafeedstatusesListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_datafeedstatuses_list(
        &self,
        args: &ContentDatafeedstatusesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DatafeedstatusesListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_datafeedstatuses_list_builder(
            &self.http_client,
            &args.merchantId,
            &args.maxResults,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = content_datafeedstatuses_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content freelistingsprogram get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FreeListingsProgramStatus result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_freelistingsprogram_get(
        &self,
        args: &ContentFreelistingsprogramGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FreeListingsProgramStatus, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_freelistingsprogram_get_builder(
            &self.http_client,
            &args.merchantId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_freelistingsprogram_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content freelistingsprogram requestreview.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn content_freelistingsprogram_requestreview(
        &self,
        args: &ContentFreelistingsprogramRequestreviewArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_freelistingsprogram_requestreview_builder(
            &self.http_client,
            &args.merchantId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_freelistingsprogram_requestreview_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content freelistingsprogram checkoutsettings delete.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn content_freelistingsprogram_checkoutsettings_delete(
        &self,
        args: &ContentFreelistingsprogramCheckoutsettingsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_freelistingsprogram_checkoutsettings_delete_builder(
            &self.http_client,
            &args.merchantId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_freelistingsprogram_checkoutsettings_delete_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content freelistingsprogram checkoutsettings get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CheckoutSettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_freelistingsprogram_checkoutsettings_get(
        &self,
        args: &ContentFreelistingsprogramCheckoutsettingsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CheckoutSettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_freelistingsprogram_checkoutsettings_get_builder(
            &self.http_client,
            &args.merchantId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_freelistingsprogram_checkoutsettings_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content freelistingsprogram checkoutsettings insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CheckoutSettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_freelistingsprogram_checkoutsettings_insert(
        &self,
        args: &ContentFreelistingsprogramCheckoutsettingsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CheckoutSettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_freelistingsprogram_checkoutsettings_insert_builder(
            &self.http_client,
            &args.merchantId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_freelistingsprogram_checkoutsettings_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content liasettings custombatch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LiasettingsCustomBatchResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_liasettings_custombatch(
        &self,
        args: &ContentLiasettingsCustombatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LiasettingsCustomBatchResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_liasettings_custombatch_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = content_liasettings_custombatch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content liasettings get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LiaSettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_liasettings_get(
        &self,
        args: &ContentLiasettingsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LiaSettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_liasettings_get_builder(
            &self.http_client,
            &args.merchantId,
            &args.accountId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_liasettings_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content liasettings getaccessiblegmbaccounts.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LiasettingsGetAccessibleGmbAccountsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_liasettings_getaccessiblegmbaccounts(
        &self,
        args: &ContentLiasettingsGetaccessiblegmbaccountsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LiasettingsGetAccessibleGmbAccountsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_liasettings_getaccessiblegmbaccounts_builder(
            &self.http_client,
            &args.merchantId,
            &args.accountId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_liasettings_getaccessiblegmbaccounts_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content liasettings list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LiasettingsListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_liasettings_list(
        &self,
        args: &ContentLiasettingsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LiasettingsListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_liasettings_list_builder(
            &self.http_client,
            &args.merchantId,
            &args.maxResults,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = content_liasettings_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content liasettings listposdataproviders.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LiasettingsListPosDataProvidersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_liasettings_listposdataproviders(
        &self,
        args: &ContentLiasettingsListposdataprovidersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LiasettingsListPosDataProvidersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_liasettings_listposdataproviders_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = content_liasettings_listposdataproviders_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content liasettings requestgmbaccess.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LiasettingsRequestGmbAccessResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_liasettings_requestgmbaccess(
        &self,
        args: &ContentLiasettingsRequestgmbaccessArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LiasettingsRequestGmbAccessResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_liasettings_requestgmbaccess_builder(
            &self.http_client,
            &args.merchantId,
            &args.accountId,
            &args.gmbEmail,
        )
        .map_err(ProviderError::Api)?;

        let task = content_liasettings_requestgmbaccess_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content liasettings requestinventoryverification.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LiasettingsRequestInventoryVerificationResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_liasettings_requestinventoryverification(
        &self,
        args: &ContentLiasettingsRequestinventoryverificationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LiasettingsRequestInventoryVerificationResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_liasettings_requestinventoryverification_builder(
            &self.http_client,
            &args.merchantId,
            &args.accountId,
            &args.country,
        )
        .map_err(ProviderError::Api)?;

        let task = content_liasettings_requestinventoryverification_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content liasettings setinventoryverificationcontact.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LiasettingsSetInventoryVerificationContactResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_liasettings_setinventoryverificationcontact(
        &self,
        args: &ContentLiasettingsSetinventoryverificationcontactArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LiasettingsSetInventoryVerificationContactResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_liasettings_setinventoryverificationcontact_builder(
            &self.http_client,
            &args.merchantId,
            &args.accountId,
            &args.contactEmail,
            &args.contactName,
            &args.country,
            &args.language,
        )
        .map_err(ProviderError::Api)?;

        let task = content_liasettings_setinventoryverificationcontact_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content liasettings setomnichannelexperience.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LiaOmnichannelExperience result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_liasettings_setomnichannelexperience(
        &self,
        args: &ContentLiasettingsSetomnichannelexperienceArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LiaOmnichannelExperience, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_liasettings_setomnichannelexperience_builder(
            &self.http_client,
            &args.merchantId,
            &args.accountId,
            &args.country,
            &args.lsfType,
            &args.pickupTypes,
        )
        .map_err(ProviderError::Api)?;

        let task = content_liasettings_setomnichannelexperience_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content liasettings setposdataprovider.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LiasettingsSetPosDataProviderResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_liasettings_setposdataprovider(
        &self,
        args: &ContentLiasettingsSetposdataproviderArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LiasettingsSetPosDataProviderResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_liasettings_setposdataprovider_builder(
            &self.http_client,
            &args.merchantId,
            &args.accountId,
            &args.country,
            &args.posDataProviderId,
            &args.posExternalAccountId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_liasettings_setposdataprovider_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content liasettings update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LiaSettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_liasettings_update(
        &self,
        args: &ContentLiasettingsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LiaSettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_liasettings_update_builder(
            &self.http_client,
            &args.merchantId,
            &args.accountId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_liasettings_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content localinventory custombatch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LocalinventoryCustomBatchResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_localinventory_custombatch(
        &self,
        args: &ContentLocalinventoryCustombatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LocalinventoryCustomBatchResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_localinventory_custombatch_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = content_localinventory_custombatch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content localinventory insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LocalInventory result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_localinventory_insert(
        &self,
        args: &ContentLocalinventoryInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LocalInventory, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_localinventory_insert_builder(
            &self.http_client,
            &args.merchantId,
            &args.productId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_localinventory_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content merchantsupport renderaccountissues.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RenderAccountIssuesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_merchantsupport_renderaccountissues(
        &self,
        args: &ContentMerchantsupportRenderaccountissuesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RenderAccountIssuesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_merchantsupport_renderaccountissues_builder(
            &self.http_client,
            &args.merchantId,
            &args.languageCode,
            &args.timeZone,
        )
        .map_err(ProviderError::Api)?;

        let task = content_merchantsupport_renderaccountissues_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content merchantsupport renderproductissues.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RenderProductIssuesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_merchantsupport_renderproductissues(
        &self,
        args: &ContentMerchantsupportRenderproductissuesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RenderProductIssuesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_merchantsupport_renderproductissues_builder(
            &self.http_client,
            &args.merchantId,
            &args.productId,
            &args.languageCode,
            &args.timeZone,
        )
        .map_err(ProviderError::Api)?;

        let task = content_merchantsupport_renderproductissues_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content merchantsupport triggeraction.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TriggerActionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_merchantsupport_triggeraction(
        &self,
        args: &ContentMerchantsupportTriggeractionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TriggerActionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_merchantsupport_triggeraction_builder(
            &self.http_client,
            &args.merchantId,
            &args.languageCode,
        )
        .map_err(ProviderError::Api)?;

        let task = content_merchantsupport_triggeraction_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content ordertrackingsignals create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the OrderTrackingSignal result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_ordertrackingsignals_create(
        &self,
        args: &ContentOrdertrackingsignalsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<OrderTrackingSignal, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_ordertrackingsignals_create_builder(
            &self.http_client,
            &args.merchantId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_ordertrackingsignals_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content pos custombatch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PosCustomBatchResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_pos_custombatch(
        &self,
        args: &ContentPosCustombatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PosCustomBatchResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_pos_custombatch_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = content_pos_custombatch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content pos delete.
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
    pub fn content_pos_delete(
        &self,
        args: &ContentPosDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_pos_delete_builder(
            &self.http_client,
            &args.merchantId,
            &args.targetMerchantId,
            &args.storeCode,
        )
        .map_err(ProviderError::Api)?;

        let task = content_pos_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content pos get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PosStore result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_pos_get(
        &self,
        args: &ContentPosGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PosStore, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_pos_get_builder(
            &self.http_client,
            &args.merchantId,
            &args.targetMerchantId,
            &args.storeCode,
        )
        .map_err(ProviderError::Api)?;

        let task = content_pos_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content pos insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PosStore result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_pos_insert(
        &self,
        args: &ContentPosInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PosStore, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_pos_insert_builder(
            &self.http_client,
            &args.merchantId,
            &args.targetMerchantId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_pos_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content pos inventory.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PosInventoryResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_pos_inventory(
        &self,
        args: &ContentPosInventoryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PosInventoryResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_pos_inventory_builder(
            &self.http_client,
            &args.merchantId,
            &args.targetMerchantId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_pos_inventory_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content pos list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PosListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_pos_list(
        &self,
        args: &ContentPosListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PosListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_pos_list_builder(
            &self.http_client,
            &args.merchantId,
            &args.targetMerchantId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_pos_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content pos sale.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PosSaleResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_pos_sale(
        &self,
        args: &ContentPosSaleArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PosSaleResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_pos_sale_builder(
            &self.http_client,
            &args.merchantId,
            &args.targetMerchantId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_pos_sale_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content productdeliverytime create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProductDeliveryTime result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_productdeliverytime_create(
        &self,
        args: &ContentProductdeliverytimeCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProductDeliveryTime, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_productdeliverytime_create_builder(
            &self.http_client,
            &args.merchantId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_productdeliverytime_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content productdeliverytime delete.
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
    pub fn content_productdeliverytime_delete(
        &self,
        args: &ContentProductdeliverytimeDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_productdeliverytime_delete_builder(
            &self.http_client,
            &args.merchantId,
            &args.productId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_productdeliverytime_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content productdeliverytime get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProductDeliveryTime result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_productdeliverytime_get(
        &self,
        args: &ContentProductdeliverytimeGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProductDeliveryTime, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_productdeliverytime_get_builder(
            &self.http_client,
            &args.merchantId,
            &args.productId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_productdeliverytime_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content products custombatch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProductsCustomBatchResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_products_custombatch(
        &self,
        args: &ContentProductsCustombatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProductsCustomBatchResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_products_custombatch_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = content_products_custombatch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content products delete.
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
    pub fn content_products_delete(
        &self,
        args: &ContentProductsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_products_delete_builder(
            &self.http_client,
            &args.merchantId,
            &args.productId,
            &args.feedId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_products_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content products get.
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
    pub fn content_products_get(
        &self,
        args: &ContentProductsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Product, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_products_get_builder(
            &self.http_client,
            &args.merchantId,
            &args.productId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_products_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content products insert.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_products_insert(
        &self,
        args: &ContentProductsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Product, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_products_insert_builder(
            &self.http_client,
            &args.merchantId,
            &args.feedId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_products_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content products list.
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
    pub fn content_products_list(
        &self,
        args: &ContentProductsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProductsListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_products_list_builder(
            &self.http_client,
            &args.merchantId,
            &args.maxResults,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = content_products_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content products update.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_products_update(
        &self,
        args: &ContentProductsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Product, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_products_update_builder(
            &self.http_client,
            &args.merchantId,
            &args.productId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = content_products_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content productstatuses custombatch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProductstatusesCustomBatchResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_productstatuses_custombatch(
        &self,
        args: &ContentProductstatusesCustombatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProductstatusesCustomBatchResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_productstatuses_custombatch_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = content_productstatuses_custombatch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content productstatuses get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProductStatus result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_productstatuses_get(
        &self,
        args: &ContentProductstatusesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProductStatus, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_productstatuses_get_builder(
            &self.http_client,
            &args.merchantId,
            &args.productId,
            &args.destinations,
        )
        .map_err(ProviderError::Api)?;

        let task = content_productstatuses_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content productstatuses list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProductstatusesListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_productstatuses_list(
        &self,
        args: &ContentProductstatusesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProductstatusesListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_productstatuses_list_builder(
            &self.http_client,
            &args.merchantId,
            &args.destinations,
            &args.maxResults,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = content_productstatuses_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content promotions create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Promotion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_promotions_create(
        &self,
        args: &ContentPromotionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Promotion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_promotions_create_builder(
            &self.http_client,
            &args.merchantId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_promotions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content promotions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Promotion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_promotions_get(
        &self,
        args: &ContentPromotionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Promotion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_promotions_get_builder(
            &self.http_client,
            &args.merchantId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = content_promotions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content promotions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListPromotionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_promotions_list(
        &self,
        args: &ContentPromotionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListPromotionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_promotions_list_builder(
            &self.http_client,
            &args.merchantId,
            &args.countryCode,
            &args.languageCode,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = content_promotions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content pubsubnotificationsettings get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PubsubNotificationSettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_pubsubnotificationsettings_get(
        &self,
        args: &ContentPubsubnotificationsettingsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PubsubNotificationSettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_pubsubnotificationsettings_get_builder(
            &self.http_client,
            &args.merchantId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_pubsubnotificationsettings_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content pubsubnotificationsettings update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PubsubNotificationSettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_pubsubnotificationsettings_update(
        &self,
        args: &ContentPubsubnotificationsettingsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PubsubNotificationSettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_pubsubnotificationsettings_update_builder(
            &self.http_client,
            &args.merchantId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_pubsubnotificationsettings_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content quotas list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListMethodQuotasResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_quotas_list(
        &self,
        args: &ContentQuotasListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListMethodQuotasResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_quotas_list_builder(
            &self.http_client,
            &args.merchantId,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = content_quotas_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content recommendations generate.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GenerateRecommendationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_recommendations_generate(
        &self,
        args: &ContentRecommendationsGenerateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GenerateRecommendationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_recommendations_generate_builder(
            &self.http_client,
            &args.merchantId,
            &args.allowedTag,
            &args.languageCode,
        )
        .map_err(ProviderError::Api)?;

        let task = content_recommendations_generate_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content recommendations report interaction.
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
    pub fn content_recommendations_report_interaction(
        &self,
        args: &ContentRecommendationsReportInteractionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_recommendations_report_interaction_builder(
            &self.http_client,
            &args.merchantId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_recommendations_report_interaction_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content regionalinventory custombatch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RegionalinventoryCustomBatchResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_regionalinventory_custombatch(
        &self,
        args: &ContentRegionalinventoryCustombatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RegionalinventoryCustomBatchResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_regionalinventory_custombatch_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = content_regionalinventory_custombatch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content regionalinventory insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RegionalInventory result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_regionalinventory_insert(
        &self,
        args: &ContentRegionalinventoryInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RegionalInventory, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_regionalinventory_insert_builder(
            &self.http_client,
            &args.merchantId,
            &args.productId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_regionalinventory_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content regions create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Region result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_regions_create(
        &self,
        args: &ContentRegionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Region, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_regions_create_builder(
            &self.http_client,
            &args.merchantId,
            &args.regionId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_regions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content regions delete.
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
    pub fn content_regions_delete(
        &self,
        args: &ContentRegionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_regions_delete_builder(
            &self.http_client,
            &args.merchantId,
            &args.regionId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_regions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content regions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Region result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_regions_get(
        &self,
        args: &ContentRegionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Region, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_regions_get_builder(
            &self.http_client,
            &args.merchantId,
            &args.regionId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_regions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content regions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListRegionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_regions_list(
        &self,
        args: &ContentRegionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListRegionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_regions_list_builder(
            &self.http_client,
            &args.merchantId,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = content_regions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content regions patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Region result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_regions_patch(
        &self,
        args: &ContentRegionsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Region, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_regions_patch_builder(
            &self.http_client,
            &args.merchantId,
            &args.regionId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = content_regions_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content reports search.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_reports_search(
        &self,
        args: &ContentReportsSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_reports_search_builder(
            &self.http_client,
            &args.merchantId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_reports_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content returnpolicyonline create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ReturnPolicyOnline result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_returnpolicyonline_create(
        &self,
        args: &ContentReturnpolicyonlineCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ReturnPolicyOnline, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_returnpolicyonline_create_builder(
            &self.http_client,
            &args.merchantId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_returnpolicyonline_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content returnpolicyonline delete.
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
    pub fn content_returnpolicyonline_delete(
        &self,
        args: &ContentReturnpolicyonlineDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_returnpolicyonline_delete_builder(
            &self.http_client,
            &args.merchantId,
            &args.returnPolicyId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_returnpolicyonline_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content returnpolicyonline get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ReturnPolicyOnline result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_returnpolicyonline_get(
        &self,
        args: &ContentReturnpolicyonlineGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ReturnPolicyOnline, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_returnpolicyonline_get_builder(
            &self.http_client,
            &args.merchantId,
            &args.returnPolicyId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_returnpolicyonline_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content returnpolicyonline list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListReturnPolicyOnlineResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_returnpolicyonline_list(
        &self,
        args: &ContentReturnpolicyonlineListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListReturnPolicyOnlineResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_returnpolicyonline_list_builder(
            &self.http_client,
            &args.merchantId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_returnpolicyonline_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content returnpolicyonline patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ReturnPolicyOnline result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_returnpolicyonline_patch(
        &self,
        args: &ContentReturnpolicyonlinePatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ReturnPolicyOnline, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_returnpolicyonline_patch_builder(
            &self.http_client,
            &args.merchantId,
            &args.returnPolicyId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_returnpolicyonline_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content shippingsettings custombatch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ShippingsettingsCustomBatchResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_shippingsettings_custombatch(
        &self,
        args: &ContentShippingsettingsCustombatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ShippingsettingsCustomBatchResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_shippingsettings_custombatch_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = content_shippingsettings_custombatch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content shippingsettings get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ShippingSettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_shippingsettings_get(
        &self,
        args: &ContentShippingsettingsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ShippingSettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_shippingsettings_get_builder(
            &self.http_client,
            &args.merchantId,
            &args.accountId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_shippingsettings_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content shippingsettings getsupportedcarriers.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ShippingsettingsGetSupportedCarriersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_shippingsettings_getsupportedcarriers(
        &self,
        args: &ContentShippingsettingsGetsupportedcarriersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ShippingsettingsGetSupportedCarriersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_shippingsettings_getsupportedcarriers_builder(
            &self.http_client,
            &args.merchantId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_shippingsettings_getsupportedcarriers_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content shippingsettings getsupportedholidays.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ShippingsettingsGetSupportedHolidaysResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_shippingsettings_getsupportedholidays(
        &self,
        args: &ContentShippingsettingsGetsupportedholidaysArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ShippingsettingsGetSupportedHolidaysResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_shippingsettings_getsupportedholidays_builder(
            &self.http_client,
            &args.merchantId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_shippingsettings_getsupportedholidays_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content shippingsettings getsupportedpickupservices.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ShippingsettingsGetSupportedPickupServicesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_shippingsettings_getsupportedpickupservices(
        &self,
        args: &ContentShippingsettingsGetsupportedpickupservicesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ShippingsettingsGetSupportedPickupServicesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_shippingsettings_getsupportedpickupservices_builder(
            &self.http_client,
            &args.merchantId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_shippingsettings_getsupportedpickupservices_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content shippingsettings list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ShippingsettingsListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_shippingsettings_list(
        &self,
        args: &ContentShippingsettingsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ShippingsettingsListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_shippingsettings_list_builder(
            &self.http_client,
            &args.merchantId,
            &args.maxResults,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = content_shippingsettings_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content shippingsettings update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ShippingSettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn content_shippingsettings_update(
        &self,
        args: &ContentShippingsettingsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ShippingSettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_shippingsettings_update_builder(
            &self.http_client,
            &args.merchantId,
            &args.accountId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_shippingsettings_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content shoppingadsprogram get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ShoppingAdsProgramStatus result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn content_shoppingadsprogram_get(
        &self,
        args: &ContentShoppingadsprogramGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ShoppingAdsProgramStatus, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_shoppingadsprogram_get_builder(
            &self.http_client,
            &args.merchantId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_shoppingadsprogram_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Content shoppingadsprogram requestreview.
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
    pub fn content_shoppingadsprogram_requestreview(
        &self,
        args: &ContentShoppingadsprogramRequestreviewArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = content_shoppingadsprogram_requestreview_builder(
            &self.http_client,
            &args.merchantId,
        )
        .map_err(ProviderError::Api)?;

        let task = content_shoppingadsprogram_requestreview_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
