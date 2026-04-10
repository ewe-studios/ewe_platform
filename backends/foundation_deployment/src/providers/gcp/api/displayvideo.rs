//! DisplayvideoProvider - State-aware displayvideo API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       displayvideo API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::displayvideo::{
    displayvideo_advertisers_create_builder, displayvideo_advertisers_create_task,
    displayvideo_advertisers_delete_builder, displayvideo_advertisers_delete_task,
    displayvideo_advertisers_edit_assigned_targeting_options_builder, displayvideo_advertisers_edit_assigned_targeting_options_task,
    displayvideo_advertisers_patch_builder, displayvideo_advertisers_patch_task,
    displayvideo_advertisers_ad_assets_bulk_create_builder, displayvideo_advertisers_ad_assets_bulk_create_task,
    displayvideo_advertisers_ad_assets_create_builder, displayvideo_advertisers_ad_assets_create_task,
    displayvideo_advertisers_ad_assets_upload_builder, displayvideo_advertisers_ad_assets_upload_task,
    displayvideo_advertisers_ad_group_ads_create_builder, displayvideo_advertisers_ad_group_ads_create_task,
    displayvideo_advertisers_ad_group_ads_delete_builder, displayvideo_advertisers_ad_group_ads_delete_task,
    displayvideo_advertisers_ad_group_ads_patch_builder, displayvideo_advertisers_ad_group_ads_patch_task,
    displayvideo_advertisers_ad_groups_bulk_edit_assigned_targeting_options_builder, displayvideo_advertisers_ad_groups_bulk_edit_assigned_targeting_options_task,
    displayvideo_advertisers_ad_groups_create_builder, displayvideo_advertisers_ad_groups_create_task,
    displayvideo_advertisers_ad_groups_delete_builder, displayvideo_advertisers_ad_groups_delete_task,
    displayvideo_advertisers_ad_groups_patch_builder, displayvideo_advertisers_ad_groups_patch_task,
    displayvideo_advertisers_ad_groups_targeting_types_assigned_targeting_options_create_builder, displayvideo_advertisers_ad_groups_targeting_types_assigned_targeting_options_create_task,
    displayvideo_advertisers_ad_groups_targeting_types_assigned_targeting_options_delete_builder, displayvideo_advertisers_ad_groups_targeting_types_assigned_targeting_options_delete_task,
    displayvideo_advertisers_ad_groups_youtube_asset_types_youtube_asset_associations_create_builder, displayvideo_advertisers_ad_groups_youtube_asset_types_youtube_asset_associations_create_task,
    displayvideo_advertisers_ad_groups_youtube_asset_types_youtube_asset_associations_delete_builder, displayvideo_advertisers_ad_groups_youtube_asset_types_youtube_asset_associations_delete_task,
    displayvideo_advertisers_assets_upload_builder, displayvideo_advertisers_assets_upload_task,
    displayvideo_advertisers_campaigns_create_builder, displayvideo_advertisers_campaigns_create_task,
    displayvideo_advertisers_campaigns_delete_builder, displayvideo_advertisers_campaigns_delete_task,
    displayvideo_advertisers_campaigns_patch_builder, displayvideo_advertisers_campaigns_patch_task,
    displayvideo_advertisers_channels_create_builder, displayvideo_advertisers_channels_create_task,
    displayvideo_advertisers_channels_patch_builder, displayvideo_advertisers_channels_patch_task,
    displayvideo_advertisers_channels_sites_bulk_edit_builder, displayvideo_advertisers_channels_sites_bulk_edit_task,
    displayvideo_advertisers_channels_sites_create_builder, displayvideo_advertisers_channels_sites_create_task,
    displayvideo_advertisers_channels_sites_delete_builder, displayvideo_advertisers_channels_sites_delete_task,
    displayvideo_advertisers_channels_sites_replace_builder, displayvideo_advertisers_channels_sites_replace_task,
    displayvideo_advertisers_creatives_create_builder, displayvideo_advertisers_creatives_create_task,
    displayvideo_advertisers_creatives_delete_builder, displayvideo_advertisers_creatives_delete_task,
    displayvideo_advertisers_creatives_patch_builder, displayvideo_advertisers_creatives_patch_task,
    displayvideo_advertisers_insertion_orders_create_builder, displayvideo_advertisers_insertion_orders_create_task,
    displayvideo_advertisers_insertion_orders_delete_builder, displayvideo_advertisers_insertion_orders_delete_task,
    displayvideo_advertisers_insertion_orders_patch_builder, displayvideo_advertisers_insertion_orders_patch_task,
    displayvideo_advertisers_line_items_bulk_edit_assigned_targeting_options_builder, displayvideo_advertisers_line_items_bulk_edit_assigned_targeting_options_task,
    displayvideo_advertisers_line_items_bulk_update_builder, displayvideo_advertisers_line_items_bulk_update_task,
    displayvideo_advertisers_line_items_create_builder, displayvideo_advertisers_line_items_create_task,
    displayvideo_advertisers_line_items_delete_builder, displayvideo_advertisers_line_items_delete_task,
    displayvideo_advertisers_line_items_duplicate_builder, displayvideo_advertisers_line_items_duplicate_task,
    displayvideo_advertisers_line_items_patch_builder, displayvideo_advertisers_line_items_patch_task,
    displayvideo_advertisers_line_items_targeting_types_assigned_targeting_options_create_builder, displayvideo_advertisers_line_items_targeting_types_assigned_targeting_options_create_task,
    displayvideo_advertisers_line_items_targeting_types_assigned_targeting_options_delete_builder, displayvideo_advertisers_line_items_targeting_types_assigned_targeting_options_delete_task,
    displayvideo_advertisers_line_items_youtube_asset_types_youtube_asset_associations_create_builder, displayvideo_advertisers_line_items_youtube_asset_types_youtube_asset_associations_create_task,
    displayvideo_advertisers_line_items_youtube_asset_types_youtube_asset_associations_delete_builder, displayvideo_advertisers_line_items_youtube_asset_types_youtube_asset_associations_delete_task,
    displayvideo_advertisers_location_lists_create_builder, displayvideo_advertisers_location_lists_create_task,
    displayvideo_advertisers_location_lists_patch_builder, displayvideo_advertisers_location_lists_patch_task,
    displayvideo_advertisers_location_lists_assigned_locations_bulk_edit_builder, displayvideo_advertisers_location_lists_assigned_locations_bulk_edit_task,
    displayvideo_advertisers_location_lists_assigned_locations_create_builder, displayvideo_advertisers_location_lists_assigned_locations_create_task,
    displayvideo_advertisers_location_lists_assigned_locations_delete_builder, displayvideo_advertisers_location_lists_assigned_locations_delete_task,
    displayvideo_advertisers_negative_keyword_lists_create_builder, displayvideo_advertisers_negative_keyword_lists_create_task,
    displayvideo_advertisers_negative_keyword_lists_delete_builder, displayvideo_advertisers_negative_keyword_lists_delete_task,
    displayvideo_advertisers_negative_keyword_lists_patch_builder, displayvideo_advertisers_negative_keyword_lists_patch_task,
    displayvideo_advertisers_negative_keyword_lists_negative_keywords_bulk_edit_builder, displayvideo_advertisers_negative_keyword_lists_negative_keywords_bulk_edit_task,
    displayvideo_advertisers_negative_keyword_lists_negative_keywords_create_builder, displayvideo_advertisers_negative_keyword_lists_negative_keywords_create_task,
    displayvideo_advertisers_negative_keyword_lists_negative_keywords_delete_builder, displayvideo_advertisers_negative_keyword_lists_negative_keywords_delete_task,
    displayvideo_advertisers_negative_keyword_lists_negative_keywords_replace_builder, displayvideo_advertisers_negative_keyword_lists_negative_keywords_replace_task,
    displayvideo_advertisers_targeting_types_assigned_targeting_options_create_builder, displayvideo_advertisers_targeting_types_assigned_targeting_options_create_task,
    displayvideo_advertisers_targeting_types_assigned_targeting_options_delete_builder, displayvideo_advertisers_targeting_types_assigned_targeting_options_delete_task,
    displayvideo_custom_bidding_algorithms_create_builder, displayvideo_custom_bidding_algorithms_create_task,
    displayvideo_custom_bidding_algorithms_patch_builder, displayvideo_custom_bidding_algorithms_patch_task,
    displayvideo_custom_bidding_algorithms_rules_create_builder, displayvideo_custom_bidding_algorithms_rules_create_task,
    displayvideo_custom_bidding_algorithms_scripts_create_builder, displayvideo_custom_bidding_algorithms_scripts_create_task,
    displayvideo_first_party_and_partner_audiences_create_builder, displayvideo_first_party_and_partner_audiences_create_task,
    displayvideo_first_party_and_partner_audiences_edit_customer_match_members_builder, displayvideo_first_party_and_partner_audiences_edit_customer_match_members_task,
    displayvideo_first_party_and_partner_audiences_patch_builder, displayvideo_first_party_and_partner_audiences_patch_task,
    displayvideo_floodlight_groups_patch_builder, displayvideo_floodlight_groups_patch_task,
    displayvideo_guaranteed_orders_create_builder, displayvideo_guaranteed_orders_create_task,
    displayvideo_guaranteed_orders_edit_guaranteed_order_read_accessors_builder, displayvideo_guaranteed_orders_edit_guaranteed_order_read_accessors_task,
    displayvideo_guaranteed_orders_patch_builder, displayvideo_guaranteed_orders_patch_task,
    displayvideo_inventory_source_groups_create_builder, displayvideo_inventory_source_groups_create_task,
    displayvideo_inventory_source_groups_delete_builder, displayvideo_inventory_source_groups_delete_task,
    displayvideo_inventory_source_groups_patch_builder, displayvideo_inventory_source_groups_patch_task,
    displayvideo_inventory_source_groups_assigned_inventory_sources_bulk_edit_builder, displayvideo_inventory_source_groups_assigned_inventory_sources_bulk_edit_task,
    displayvideo_inventory_source_groups_assigned_inventory_sources_create_builder, displayvideo_inventory_source_groups_assigned_inventory_sources_create_task,
    displayvideo_inventory_source_groups_assigned_inventory_sources_delete_builder, displayvideo_inventory_source_groups_assigned_inventory_sources_delete_task,
    displayvideo_inventory_sources_create_builder, displayvideo_inventory_sources_create_task,
    displayvideo_inventory_sources_edit_inventory_source_read_write_accessors_builder, displayvideo_inventory_sources_edit_inventory_source_read_write_accessors_task,
    displayvideo_inventory_sources_patch_builder, displayvideo_inventory_sources_patch_task,
    displayvideo_media_upload_builder, displayvideo_media_upload_task,
    displayvideo_partners_edit_assigned_targeting_options_builder, displayvideo_partners_edit_assigned_targeting_options_task,
    displayvideo_partners_channels_create_builder, displayvideo_partners_channels_create_task,
    displayvideo_partners_channels_patch_builder, displayvideo_partners_channels_patch_task,
    displayvideo_partners_channels_sites_bulk_edit_builder, displayvideo_partners_channels_sites_bulk_edit_task,
    displayvideo_partners_channels_sites_create_builder, displayvideo_partners_channels_sites_create_task,
    displayvideo_partners_channels_sites_delete_builder, displayvideo_partners_channels_sites_delete_task,
    displayvideo_partners_channels_sites_replace_builder, displayvideo_partners_channels_sites_replace_task,
    displayvideo_partners_targeting_types_assigned_targeting_options_create_builder, displayvideo_partners_targeting_types_assigned_targeting_options_create_task,
    displayvideo_partners_targeting_types_assigned_targeting_options_delete_builder, displayvideo_partners_targeting_types_assigned_targeting_options_delete_task,
    displayvideo_sdfdownloadtasks_create_builder, displayvideo_sdfdownloadtasks_create_task,
    displayvideo_targeting_types_targeting_options_search_builder, displayvideo_targeting_types_targeting_options_search_task,
    displayvideo_users_bulk_edit_assigned_user_roles_builder, displayvideo_users_bulk_edit_assigned_user_roles_task,
    displayvideo_users_create_builder, displayvideo_users_create_task,
    displayvideo_users_delete_builder, displayvideo_users_delete_task,
    displayvideo_users_patch_builder, displayvideo_users_patch_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::displayvideo::AdAsset;
use crate::providers::gcp::clients::displayvideo::AdGroup;
use crate::providers::gcp::clients::displayvideo::AdGroupAd;
use crate::providers::gcp::clients::displayvideo::Advertiser;
use crate::providers::gcp::clients::displayvideo::AssignedInventorySource;
use crate::providers::gcp::clients::displayvideo::AssignedLocation;
use crate::providers::gcp::clients::displayvideo::AssignedTargetingOption;
use crate::providers::gcp::clients::displayvideo::BulkCreateAdAssetsResponse;
use crate::providers::gcp::clients::displayvideo::BulkEditAdGroupAssignedTargetingOptionsResponse;
use crate::providers::gcp::clients::displayvideo::BulkEditAdvertiserAssignedTargetingOptionsResponse;
use crate::providers::gcp::clients::displayvideo::BulkEditAssignedInventorySourcesResponse;
use crate::providers::gcp::clients::displayvideo::BulkEditAssignedLocationsResponse;
use crate::providers::gcp::clients::displayvideo::BulkEditAssignedTargetingOptionsResponse;
use crate::providers::gcp::clients::displayvideo::BulkEditAssignedUserRolesResponse;
use crate::providers::gcp::clients::displayvideo::BulkEditNegativeKeywordsResponse;
use crate::providers::gcp::clients::displayvideo::BulkEditPartnerAssignedTargetingOptionsResponse;
use crate::providers::gcp::clients::displayvideo::BulkEditSitesResponse;
use crate::providers::gcp::clients::displayvideo::BulkUpdateLineItemsResponse;
use crate::providers::gcp::clients::displayvideo::Campaign;
use crate::providers::gcp::clients::displayvideo::Channel;
use crate::providers::gcp::clients::displayvideo::CreateAssetResponse;
use crate::providers::gcp::clients::displayvideo::Creative;
use crate::providers::gcp::clients::displayvideo::CustomBiddingAlgorithm;
use crate::providers::gcp::clients::displayvideo::CustomBiddingAlgorithmRules;
use crate::providers::gcp::clients::displayvideo::CustomBiddingScript;
use crate::providers::gcp::clients::displayvideo::DuplicateLineItemResponse;
use crate::providers::gcp::clients::displayvideo::EditCustomerMatchMembersResponse;
use crate::providers::gcp::clients::displayvideo::EditGuaranteedOrderReadAccessorsResponse;
use crate::providers::gcp::clients::displayvideo::Empty;
use crate::providers::gcp::clients::displayvideo::FirstPartyAndPartnerAudience;
use crate::providers::gcp::clients::displayvideo::FloodlightGroup;
use crate::providers::gcp::clients::displayvideo::GoogleBytestreamMedia;
use crate::providers::gcp::clients::displayvideo::GuaranteedOrder;
use crate::providers::gcp::clients::displayvideo::InsertionOrder;
use crate::providers::gcp::clients::displayvideo::InventorySource;
use crate::providers::gcp::clients::displayvideo::InventorySourceAccessors;
use crate::providers::gcp::clients::displayvideo::InventorySourceGroup;
use crate::providers::gcp::clients::displayvideo::LineItem;
use crate::providers::gcp::clients::displayvideo::LocationList;
use crate::providers::gcp::clients::displayvideo::NegativeKeyword;
use crate::providers::gcp::clients::displayvideo::NegativeKeywordList;
use crate::providers::gcp::clients::displayvideo::Operation;
use crate::providers::gcp::clients::displayvideo::ReplaceNegativeKeywordsResponse;
use crate::providers::gcp::clients::displayvideo::ReplaceSitesResponse;
use crate::providers::gcp::clients::displayvideo::SearchTargetingOptionsResponse;
use crate::providers::gcp::clients::displayvideo::Site;
use crate::providers::gcp::clients::displayvideo::UploadAdAssetResponse;
use crate::providers::gcp::clients::displayvideo::User;
use crate::providers::gcp::clients::displayvideo::YoutubeAssetAssociation;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersAdAssetsBulkCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersAdAssetsCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersAdAssetsUploadArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersAdGroupAdsCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersAdGroupAdsDeleteArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersAdGroupAdsPatchArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersAdGroupsBulkEditAssignedTargetingOptionsArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersAdGroupsCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersAdGroupsDeleteArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersAdGroupsPatchArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersAdGroupsTargetingTypesAssignedTargetingOptionsCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersAdGroupsTargetingTypesAssignedTargetingOptionsDeleteArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersAdGroupsYoutubeAssetTypesYoutubeAssetAssociationsCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersAdGroupsYoutubeAssetTypesYoutubeAssetAssociationsDeleteArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersAssetsUploadArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersCampaignsCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersCampaignsDeleteArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersCampaignsPatchArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersChannelsCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersChannelsPatchArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersChannelsSitesBulkEditArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersChannelsSitesCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersChannelsSitesDeleteArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersChannelsSitesReplaceArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersCreativesCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersCreativesDeleteArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersCreativesPatchArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersDeleteArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersEditAssignedTargetingOptionsArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersInsertionOrdersCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersInsertionOrdersDeleteArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersInsertionOrdersPatchArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersLineItemsBulkEditAssignedTargetingOptionsArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersLineItemsBulkUpdateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersLineItemsCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersLineItemsDeleteArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersLineItemsDuplicateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersLineItemsPatchArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersLineItemsTargetingTypesAssignedTargetingOptionsCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersLineItemsTargetingTypesAssignedTargetingOptionsDeleteArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersLineItemsYoutubeAssetTypesYoutubeAssetAssociationsCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersLineItemsYoutubeAssetTypesYoutubeAssetAssociationsDeleteArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersLocationListsAssignedLocationsBulkEditArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersLocationListsAssignedLocationsCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersLocationListsAssignedLocationsDeleteArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersLocationListsCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersLocationListsPatchArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersNegativeKeywordListsCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersNegativeKeywordListsDeleteArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersNegativeKeywordListsNegativeKeywordsBulkEditArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersNegativeKeywordListsNegativeKeywordsCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersNegativeKeywordListsNegativeKeywordsDeleteArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersNegativeKeywordListsNegativeKeywordsReplaceArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersNegativeKeywordListsPatchArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersPatchArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersTargetingTypesAssignedTargetingOptionsCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersTargetingTypesAssignedTargetingOptionsDeleteArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoCustomBiddingAlgorithmsCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoCustomBiddingAlgorithmsPatchArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoCustomBiddingAlgorithmsRulesCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoCustomBiddingAlgorithmsScriptsCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoFirstPartyAndPartnerAudiencesCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoFirstPartyAndPartnerAudiencesEditCustomerMatchMembersArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoFirstPartyAndPartnerAudiencesPatchArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoFloodlightGroupsPatchArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoGuaranteedOrdersCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoGuaranteedOrdersEditGuaranteedOrderReadAccessorsArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoGuaranteedOrdersPatchArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoInventorySourceGroupsAssignedInventorySourcesBulkEditArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoInventorySourceGroupsAssignedInventorySourcesCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoInventorySourceGroupsAssignedInventorySourcesDeleteArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoInventorySourceGroupsCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoInventorySourceGroupsDeleteArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoInventorySourceGroupsPatchArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoInventorySourcesCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoInventorySourcesEditInventorySourceReadWriteAccessorsArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoInventorySourcesPatchArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoMediaUploadArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoPartnersChannelsCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoPartnersChannelsPatchArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoPartnersChannelsSitesBulkEditArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoPartnersChannelsSitesCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoPartnersChannelsSitesDeleteArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoPartnersChannelsSitesReplaceArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoPartnersEditAssignedTargetingOptionsArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoPartnersTargetingTypesAssignedTargetingOptionsCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoPartnersTargetingTypesAssignedTargetingOptionsDeleteArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoSdfdownloadtasksCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoTargetingTypesTargetingOptionsSearchArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoUsersBulkEditAssignedUserRolesArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoUsersCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoUsersDeleteArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoUsersPatchArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// DisplayvideoProvider with automatic state tracking.
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
/// let provider = DisplayvideoProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct DisplayvideoProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> DisplayvideoProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new DisplayvideoProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Displayvideo advertisers create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Advertiser result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_advertisers_create(
        &self,
        args: &DisplayvideoAdvertisersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Advertiser, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_create_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers delete.
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
    pub fn displayvideo_advertisers_delete(
        &self,
        args: &DisplayvideoAdvertisersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_delete_builder(
            &self.http_client,
            &args.advertiserId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers edit assigned targeting options.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BulkEditAdvertiserAssignedTargetingOptionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_advertisers_edit_assigned_targeting_options(
        &self,
        args: &DisplayvideoAdvertisersEditAssignedTargetingOptionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BulkEditAdvertiserAssignedTargetingOptionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_edit_assigned_targeting_options_builder(
            &self.http_client,
            &args.advertiserId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_edit_assigned_targeting_options_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Advertiser result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_advertisers_patch(
        &self,
        args: &DisplayvideoAdvertisersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Advertiser, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_patch_builder(
            &self.http_client,
            &args.advertiserId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers ad assets bulk create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BulkCreateAdAssetsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_advertisers_ad_assets_bulk_create(
        &self,
        args: &DisplayvideoAdvertisersAdAssetsBulkCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BulkCreateAdAssetsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_ad_assets_bulk_create_builder(
            &self.http_client,
            &args.advertiserId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_ad_assets_bulk_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers ad assets create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AdAsset result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_advertisers_ad_assets_create(
        &self,
        args: &DisplayvideoAdvertisersAdAssetsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AdAsset, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_ad_assets_create_builder(
            &self.http_client,
            &args.advertiserId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_ad_assets_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers ad assets upload.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UploadAdAssetResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_advertisers_ad_assets_upload(
        &self,
        args: &DisplayvideoAdvertisersAdAssetsUploadArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UploadAdAssetResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_ad_assets_upload_builder(
            &self.http_client,
            &args.advertiserId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_ad_assets_upload_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers ad group ads create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AdGroupAd result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_advertisers_ad_group_ads_create(
        &self,
        args: &DisplayvideoAdvertisersAdGroupAdsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AdGroupAd, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_ad_group_ads_create_builder(
            &self.http_client,
            &args.advertiserId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_ad_group_ads_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers ad group ads delete.
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
    pub fn displayvideo_advertisers_ad_group_ads_delete(
        &self,
        args: &DisplayvideoAdvertisersAdGroupAdsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_ad_group_ads_delete_builder(
            &self.http_client,
            &args.advertiserId,
            &args.adGroupAdId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_ad_group_ads_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers ad group ads patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AdGroupAd result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_advertisers_ad_group_ads_patch(
        &self,
        args: &DisplayvideoAdvertisersAdGroupAdsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AdGroupAd, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_ad_group_ads_patch_builder(
            &self.http_client,
            &args.advertiserId,
            &args.adGroupAdId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_ad_group_ads_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers ad groups bulk edit assigned targeting options.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BulkEditAdGroupAssignedTargetingOptionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_advertisers_ad_groups_bulk_edit_assigned_targeting_options(
        &self,
        args: &DisplayvideoAdvertisersAdGroupsBulkEditAssignedTargetingOptionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BulkEditAdGroupAssignedTargetingOptionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_ad_groups_bulk_edit_assigned_targeting_options_builder(
            &self.http_client,
            &args.advertiserId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_ad_groups_bulk_edit_assigned_targeting_options_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers ad groups create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AdGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_advertisers_ad_groups_create(
        &self,
        args: &DisplayvideoAdvertisersAdGroupsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AdGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_ad_groups_create_builder(
            &self.http_client,
            &args.advertiserId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_ad_groups_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers ad groups delete.
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
    pub fn displayvideo_advertisers_ad_groups_delete(
        &self,
        args: &DisplayvideoAdvertisersAdGroupsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_ad_groups_delete_builder(
            &self.http_client,
            &args.advertiserId,
            &args.adGroupId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_ad_groups_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers ad groups patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AdGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_advertisers_ad_groups_patch(
        &self,
        args: &DisplayvideoAdvertisersAdGroupsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AdGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_ad_groups_patch_builder(
            &self.http_client,
            &args.advertiserId,
            &args.adGroupId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_ad_groups_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers ad groups targeting types assigned targeting options create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AssignedTargetingOption result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_advertisers_ad_groups_targeting_types_assigned_targeting_options_create(
        &self,
        args: &DisplayvideoAdvertisersAdGroupsTargetingTypesAssignedTargetingOptionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AssignedTargetingOption, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_ad_groups_targeting_types_assigned_targeting_options_create_builder(
            &self.http_client,
            &args.advertiserId,
            &args.adGroupId,
            &args.targetingType,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_ad_groups_targeting_types_assigned_targeting_options_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers ad groups targeting types assigned targeting options delete.
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
    pub fn displayvideo_advertisers_ad_groups_targeting_types_assigned_targeting_options_delete(
        &self,
        args: &DisplayvideoAdvertisersAdGroupsTargetingTypesAssignedTargetingOptionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_ad_groups_targeting_types_assigned_targeting_options_delete_builder(
            &self.http_client,
            &args.advertiserId,
            &args.adGroupId,
            &args.targetingType,
            &args.assignedTargetingOptionId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_ad_groups_targeting_types_assigned_targeting_options_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers ad groups youtube asset types youtube asset associations create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the YoutubeAssetAssociation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_advertisers_ad_groups_youtube_asset_types_youtube_asset_associations_create(
        &self,
        args: &DisplayvideoAdvertisersAdGroupsYoutubeAssetTypesYoutubeAssetAssociationsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<YoutubeAssetAssociation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_ad_groups_youtube_asset_types_youtube_asset_associations_create_builder(
            &self.http_client,
            &args.advertiserId,
            &args.adGroupId,
            &args.youtubeAssetType,
            &args.linkedEntity.lineItemId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_ad_groups_youtube_asset_types_youtube_asset_associations_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers ad groups youtube asset types youtube asset associations delete.
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
    pub fn displayvideo_advertisers_ad_groups_youtube_asset_types_youtube_asset_associations_delete(
        &self,
        args: &DisplayvideoAdvertisersAdGroupsYoutubeAssetTypesYoutubeAssetAssociationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_ad_groups_youtube_asset_types_youtube_asset_associations_delete_builder(
            &self.http_client,
            &args.advertiserId,
            &args.adGroupId,
            &args.youtubeAssetType,
            &args.youtubeAssetAssociationId,
            &args.linkedEntity.lineItemId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_ad_groups_youtube_asset_types_youtube_asset_associations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers assets upload.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CreateAssetResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_advertisers_assets_upload(
        &self,
        args: &DisplayvideoAdvertisersAssetsUploadArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CreateAssetResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_assets_upload_builder(
            &self.http_client,
            &args.advertiserId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_assets_upload_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers campaigns create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Campaign result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_advertisers_campaigns_create(
        &self,
        args: &DisplayvideoAdvertisersCampaignsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Campaign, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_campaigns_create_builder(
            &self.http_client,
            &args.advertiserId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_campaigns_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers campaigns delete.
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
    pub fn displayvideo_advertisers_campaigns_delete(
        &self,
        args: &DisplayvideoAdvertisersCampaignsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_campaigns_delete_builder(
            &self.http_client,
            &args.advertiserId,
            &args.campaignId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_campaigns_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers campaigns patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Campaign result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_advertisers_campaigns_patch(
        &self,
        args: &DisplayvideoAdvertisersCampaignsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Campaign, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_campaigns_patch_builder(
            &self.http_client,
            &args.advertiserId,
            &args.campaignId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_campaigns_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers channels create.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_advertisers_channels_create(
        &self,
        args: &DisplayvideoAdvertisersChannelsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Channel, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_channels_create_builder(
            &self.http_client,
            &args.advertiserId,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_channels_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers channels patch.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_advertisers_channels_patch(
        &self,
        args: &DisplayvideoAdvertisersChannelsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Channel, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_channels_patch_builder(
            &self.http_client,
            &args.advertiserId,
            &args.channelId,
            &args.partnerId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_channels_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers channels sites bulk edit.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BulkEditSitesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_advertisers_channels_sites_bulk_edit(
        &self,
        args: &DisplayvideoAdvertisersChannelsSitesBulkEditArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BulkEditSitesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_channels_sites_bulk_edit_builder(
            &self.http_client,
            &args.advertiserId,
            &args.channelId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_channels_sites_bulk_edit_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers channels sites create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Site result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_advertisers_channels_sites_create(
        &self,
        args: &DisplayvideoAdvertisersChannelsSitesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Site, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_channels_sites_create_builder(
            &self.http_client,
            &args.advertiserId,
            &args.channelId,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_channels_sites_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers channels sites delete.
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
    pub fn displayvideo_advertisers_channels_sites_delete(
        &self,
        args: &DisplayvideoAdvertisersChannelsSitesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_channels_sites_delete_builder(
            &self.http_client,
            &args.advertiserId,
            &args.channelId,
            &args.urlOrAppId,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_channels_sites_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers channels sites replace.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ReplaceSitesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_advertisers_channels_sites_replace(
        &self,
        args: &DisplayvideoAdvertisersChannelsSitesReplaceArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ReplaceSitesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_channels_sites_replace_builder(
            &self.http_client,
            &args.advertiserId,
            &args.channelId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_channels_sites_replace_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers creatives create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Creative result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_advertisers_creatives_create(
        &self,
        args: &DisplayvideoAdvertisersCreativesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Creative, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_creatives_create_builder(
            &self.http_client,
            &args.advertiserId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_creatives_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers creatives delete.
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
    pub fn displayvideo_advertisers_creatives_delete(
        &self,
        args: &DisplayvideoAdvertisersCreativesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_creatives_delete_builder(
            &self.http_client,
            &args.advertiserId,
            &args.creativeId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_creatives_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers creatives patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Creative result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_advertisers_creatives_patch(
        &self,
        args: &DisplayvideoAdvertisersCreativesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Creative, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_creatives_patch_builder(
            &self.http_client,
            &args.advertiserId,
            &args.creativeId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_creatives_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers insertion orders create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the InsertionOrder result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_advertisers_insertion_orders_create(
        &self,
        args: &DisplayvideoAdvertisersInsertionOrdersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<InsertionOrder, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_insertion_orders_create_builder(
            &self.http_client,
            &args.advertiserId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_insertion_orders_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers insertion orders delete.
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
    pub fn displayvideo_advertisers_insertion_orders_delete(
        &self,
        args: &DisplayvideoAdvertisersInsertionOrdersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_insertion_orders_delete_builder(
            &self.http_client,
            &args.advertiserId,
            &args.insertionOrderId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_insertion_orders_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers insertion orders patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the InsertionOrder result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_advertisers_insertion_orders_patch(
        &self,
        args: &DisplayvideoAdvertisersInsertionOrdersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<InsertionOrder, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_insertion_orders_patch_builder(
            &self.http_client,
            &args.advertiserId,
            &args.insertionOrderId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_insertion_orders_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers line items bulk edit assigned targeting options.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BulkEditAssignedTargetingOptionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_advertisers_line_items_bulk_edit_assigned_targeting_options(
        &self,
        args: &DisplayvideoAdvertisersLineItemsBulkEditAssignedTargetingOptionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BulkEditAssignedTargetingOptionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_line_items_bulk_edit_assigned_targeting_options_builder(
            &self.http_client,
            &args.advertiserId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_line_items_bulk_edit_assigned_targeting_options_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers line items bulk update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BulkUpdateLineItemsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_advertisers_line_items_bulk_update(
        &self,
        args: &DisplayvideoAdvertisersLineItemsBulkUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BulkUpdateLineItemsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_line_items_bulk_update_builder(
            &self.http_client,
            &args.advertiserId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_line_items_bulk_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers line items create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LineItem result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_advertisers_line_items_create(
        &self,
        args: &DisplayvideoAdvertisersLineItemsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LineItem, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_line_items_create_builder(
            &self.http_client,
            &args.advertiserId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_line_items_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers line items delete.
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
    pub fn displayvideo_advertisers_line_items_delete(
        &self,
        args: &DisplayvideoAdvertisersLineItemsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_line_items_delete_builder(
            &self.http_client,
            &args.advertiserId,
            &args.lineItemId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_line_items_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers line items duplicate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DuplicateLineItemResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_advertisers_line_items_duplicate(
        &self,
        args: &DisplayvideoAdvertisersLineItemsDuplicateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DuplicateLineItemResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_line_items_duplicate_builder(
            &self.http_client,
            &args.advertiserId,
            &args.lineItemId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_line_items_duplicate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers line items patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LineItem result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_advertisers_line_items_patch(
        &self,
        args: &DisplayvideoAdvertisersLineItemsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LineItem, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_line_items_patch_builder(
            &self.http_client,
            &args.advertiserId,
            &args.lineItemId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_line_items_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers line items targeting types assigned targeting options create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AssignedTargetingOption result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_advertisers_line_items_targeting_types_assigned_targeting_options_create(
        &self,
        args: &DisplayvideoAdvertisersLineItemsTargetingTypesAssignedTargetingOptionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AssignedTargetingOption, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_line_items_targeting_types_assigned_targeting_options_create_builder(
            &self.http_client,
            &args.advertiserId,
            &args.lineItemId,
            &args.targetingType,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_line_items_targeting_types_assigned_targeting_options_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers line items targeting types assigned targeting options delete.
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
    pub fn displayvideo_advertisers_line_items_targeting_types_assigned_targeting_options_delete(
        &self,
        args: &DisplayvideoAdvertisersLineItemsTargetingTypesAssignedTargetingOptionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_line_items_targeting_types_assigned_targeting_options_delete_builder(
            &self.http_client,
            &args.advertiserId,
            &args.lineItemId,
            &args.targetingType,
            &args.assignedTargetingOptionId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_line_items_targeting_types_assigned_targeting_options_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers line items youtube asset types youtube asset associations create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the YoutubeAssetAssociation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_advertisers_line_items_youtube_asset_types_youtube_asset_associations_create(
        &self,
        args: &DisplayvideoAdvertisersLineItemsYoutubeAssetTypesYoutubeAssetAssociationsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<YoutubeAssetAssociation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_line_items_youtube_asset_types_youtube_asset_associations_create_builder(
            &self.http_client,
            &args.advertiserId,
            &args.lineItemId,
            &args.youtubeAssetType,
            &args.linkedEntity.adGroupId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_line_items_youtube_asset_types_youtube_asset_associations_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers line items youtube asset types youtube asset associations delete.
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
    pub fn displayvideo_advertisers_line_items_youtube_asset_types_youtube_asset_associations_delete(
        &self,
        args: &DisplayvideoAdvertisersLineItemsYoutubeAssetTypesYoutubeAssetAssociationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_line_items_youtube_asset_types_youtube_asset_associations_delete_builder(
            &self.http_client,
            &args.advertiserId,
            &args.lineItemId,
            &args.youtubeAssetType,
            &args.youtubeAssetAssociationId,
            &args.linkedEntity.adGroupId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_line_items_youtube_asset_types_youtube_asset_associations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers location lists create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LocationList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_advertisers_location_lists_create(
        &self,
        args: &DisplayvideoAdvertisersLocationListsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LocationList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_location_lists_create_builder(
            &self.http_client,
            &args.advertiserId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_location_lists_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers location lists patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LocationList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_advertisers_location_lists_patch(
        &self,
        args: &DisplayvideoAdvertisersLocationListsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LocationList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_location_lists_patch_builder(
            &self.http_client,
            &args.advertiserId,
            &args.locationListId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_location_lists_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers location lists assigned locations bulk edit.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BulkEditAssignedLocationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_advertisers_location_lists_assigned_locations_bulk_edit(
        &self,
        args: &DisplayvideoAdvertisersLocationListsAssignedLocationsBulkEditArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BulkEditAssignedLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_location_lists_assigned_locations_bulk_edit_builder(
            &self.http_client,
            &args.advertiserId,
            &args.locationListId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_location_lists_assigned_locations_bulk_edit_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers location lists assigned locations create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AssignedLocation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_advertisers_location_lists_assigned_locations_create(
        &self,
        args: &DisplayvideoAdvertisersLocationListsAssignedLocationsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AssignedLocation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_location_lists_assigned_locations_create_builder(
            &self.http_client,
            &args.advertiserId,
            &args.locationListId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_location_lists_assigned_locations_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers location lists assigned locations delete.
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
    pub fn displayvideo_advertisers_location_lists_assigned_locations_delete(
        &self,
        args: &DisplayvideoAdvertisersLocationListsAssignedLocationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_location_lists_assigned_locations_delete_builder(
            &self.http_client,
            &args.advertiserId,
            &args.locationListId,
            &args.assignedLocationId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_location_lists_assigned_locations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers negative keyword lists create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NegativeKeywordList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_advertisers_negative_keyword_lists_create(
        &self,
        args: &DisplayvideoAdvertisersNegativeKeywordListsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NegativeKeywordList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_negative_keyword_lists_create_builder(
            &self.http_client,
            &args.advertiserId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_negative_keyword_lists_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers negative keyword lists delete.
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
    pub fn displayvideo_advertisers_negative_keyword_lists_delete(
        &self,
        args: &DisplayvideoAdvertisersNegativeKeywordListsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_negative_keyword_lists_delete_builder(
            &self.http_client,
            &args.advertiserId,
            &args.negativeKeywordListId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_negative_keyword_lists_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers negative keyword lists patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NegativeKeywordList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_advertisers_negative_keyword_lists_patch(
        &self,
        args: &DisplayvideoAdvertisersNegativeKeywordListsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NegativeKeywordList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_negative_keyword_lists_patch_builder(
            &self.http_client,
            &args.advertiserId,
            &args.negativeKeywordListId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_negative_keyword_lists_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers negative keyword lists negative keywords bulk edit.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BulkEditNegativeKeywordsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_advertisers_negative_keyword_lists_negative_keywords_bulk_edit(
        &self,
        args: &DisplayvideoAdvertisersNegativeKeywordListsNegativeKeywordsBulkEditArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BulkEditNegativeKeywordsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_negative_keyword_lists_negative_keywords_bulk_edit_builder(
            &self.http_client,
            &args.advertiserId,
            &args.negativeKeywordListId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_negative_keyword_lists_negative_keywords_bulk_edit_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers negative keyword lists negative keywords create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NegativeKeyword result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_advertisers_negative_keyword_lists_negative_keywords_create(
        &self,
        args: &DisplayvideoAdvertisersNegativeKeywordListsNegativeKeywordsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NegativeKeyword, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_negative_keyword_lists_negative_keywords_create_builder(
            &self.http_client,
            &args.advertiserId,
            &args.negativeKeywordListId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_negative_keyword_lists_negative_keywords_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers negative keyword lists negative keywords delete.
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
    pub fn displayvideo_advertisers_negative_keyword_lists_negative_keywords_delete(
        &self,
        args: &DisplayvideoAdvertisersNegativeKeywordListsNegativeKeywordsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_negative_keyword_lists_negative_keywords_delete_builder(
            &self.http_client,
            &args.advertiserId,
            &args.negativeKeywordListId,
            &args.keywordValue,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_negative_keyword_lists_negative_keywords_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers negative keyword lists negative keywords replace.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ReplaceNegativeKeywordsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_advertisers_negative_keyword_lists_negative_keywords_replace(
        &self,
        args: &DisplayvideoAdvertisersNegativeKeywordListsNegativeKeywordsReplaceArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ReplaceNegativeKeywordsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_negative_keyword_lists_negative_keywords_replace_builder(
            &self.http_client,
            &args.advertiserId,
            &args.negativeKeywordListId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_negative_keyword_lists_negative_keywords_replace_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers targeting types assigned targeting options create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AssignedTargetingOption result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_advertisers_targeting_types_assigned_targeting_options_create(
        &self,
        args: &DisplayvideoAdvertisersTargetingTypesAssignedTargetingOptionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AssignedTargetingOption, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_targeting_types_assigned_targeting_options_create_builder(
            &self.http_client,
            &args.advertiserId,
            &args.targetingType,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_targeting_types_assigned_targeting_options_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers targeting types assigned targeting options delete.
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
    pub fn displayvideo_advertisers_targeting_types_assigned_targeting_options_delete(
        &self,
        args: &DisplayvideoAdvertisersTargetingTypesAssignedTargetingOptionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_targeting_types_assigned_targeting_options_delete_builder(
            &self.http_client,
            &args.advertiserId,
            &args.targetingType,
            &args.assignedTargetingOptionId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_targeting_types_assigned_targeting_options_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo custom bidding algorithms create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CustomBiddingAlgorithm result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_custom_bidding_algorithms_create(
        &self,
        args: &DisplayvideoCustomBiddingAlgorithmsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CustomBiddingAlgorithm, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_custom_bidding_algorithms_create_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_custom_bidding_algorithms_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo custom bidding algorithms patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CustomBiddingAlgorithm result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_custom_bidding_algorithms_patch(
        &self,
        args: &DisplayvideoCustomBiddingAlgorithmsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CustomBiddingAlgorithm, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_custom_bidding_algorithms_patch_builder(
            &self.http_client,
            &args.customBiddingAlgorithmId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_custom_bidding_algorithms_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo custom bidding algorithms rules create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CustomBiddingAlgorithmRules result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_custom_bidding_algorithms_rules_create(
        &self,
        args: &DisplayvideoCustomBiddingAlgorithmsRulesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CustomBiddingAlgorithmRules, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_custom_bidding_algorithms_rules_create_builder(
            &self.http_client,
            &args.customBiddingAlgorithmId,
            &args.advertiserId,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_custom_bidding_algorithms_rules_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo custom bidding algorithms scripts create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CustomBiddingScript result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_custom_bidding_algorithms_scripts_create(
        &self,
        args: &DisplayvideoCustomBiddingAlgorithmsScriptsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CustomBiddingScript, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_custom_bidding_algorithms_scripts_create_builder(
            &self.http_client,
            &args.customBiddingAlgorithmId,
            &args.advertiserId,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_custom_bidding_algorithms_scripts_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo first party and partner audiences create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FirstPartyAndPartnerAudience result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_first_party_and_partner_audiences_create(
        &self,
        args: &DisplayvideoFirstPartyAndPartnerAudiencesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FirstPartyAndPartnerAudience, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_first_party_and_partner_audiences_create_builder(
            &self.http_client,
            &args.advertiserId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_first_party_and_partner_audiences_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo first party and partner audiences edit customer match members.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EditCustomerMatchMembersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_first_party_and_partner_audiences_edit_customer_match_members(
        &self,
        args: &DisplayvideoFirstPartyAndPartnerAudiencesEditCustomerMatchMembersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EditCustomerMatchMembersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_first_party_and_partner_audiences_edit_customer_match_members_builder(
            &self.http_client,
            &args.firstPartyAndPartnerAudienceId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_first_party_and_partner_audiences_edit_customer_match_members_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo first party and partner audiences patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FirstPartyAndPartnerAudience result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_first_party_and_partner_audiences_patch(
        &self,
        args: &DisplayvideoFirstPartyAndPartnerAudiencesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FirstPartyAndPartnerAudience, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_first_party_and_partner_audiences_patch_builder(
            &self.http_client,
            &args.firstPartyAndPartnerAudienceId,
            &args.advertiserId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_first_party_and_partner_audiences_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo floodlight groups patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FloodlightGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_floodlight_groups_patch(
        &self,
        args: &DisplayvideoFloodlightGroupsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FloodlightGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_floodlight_groups_patch_builder(
            &self.http_client,
            &args.floodlightGroupId,
            &args.partnerId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_floodlight_groups_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo guaranteed orders create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GuaranteedOrder result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_guaranteed_orders_create(
        &self,
        args: &DisplayvideoGuaranteedOrdersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GuaranteedOrder, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_guaranteed_orders_create_builder(
            &self.http_client,
            &args.advertiserId,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_guaranteed_orders_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo guaranteed orders edit guaranteed order read accessors.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EditGuaranteedOrderReadAccessorsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_guaranteed_orders_edit_guaranteed_order_read_accessors(
        &self,
        args: &DisplayvideoGuaranteedOrdersEditGuaranteedOrderReadAccessorsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EditGuaranteedOrderReadAccessorsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_guaranteed_orders_edit_guaranteed_order_read_accessors_builder(
            &self.http_client,
            &args.guaranteedOrderId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_guaranteed_orders_edit_guaranteed_order_read_accessors_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo guaranteed orders patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GuaranteedOrder result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_guaranteed_orders_patch(
        &self,
        args: &DisplayvideoGuaranteedOrdersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GuaranteedOrder, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_guaranteed_orders_patch_builder(
            &self.http_client,
            &args.guaranteedOrderId,
            &args.advertiserId,
            &args.partnerId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_guaranteed_orders_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo inventory source groups create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the InventorySourceGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_inventory_source_groups_create(
        &self,
        args: &DisplayvideoInventorySourceGroupsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<InventorySourceGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_inventory_source_groups_create_builder(
            &self.http_client,
            &args.advertiserId,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_inventory_source_groups_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo inventory source groups delete.
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
    pub fn displayvideo_inventory_source_groups_delete(
        &self,
        args: &DisplayvideoInventorySourceGroupsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_inventory_source_groups_delete_builder(
            &self.http_client,
            &args.inventorySourceGroupId,
            &args.advertiserId,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_inventory_source_groups_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo inventory source groups patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the InventorySourceGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_inventory_source_groups_patch(
        &self,
        args: &DisplayvideoInventorySourceGroupsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<InventorySourceGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_inventory_source_groups_patch_builder(
            &self.http_client,
            &args.inventorySourceGroupId,
            &args.advertiserId,
            &args.partnerId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_inventory_source_groups_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo inventory source groups assigned inventory sources bulk edit.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BulkEditAssignedInventorySourcesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_inventory_source_groups_assigned_inventory_sources_bulk_edit(
        &self,
        args: &DisplayvideoInventorySourceGroupsAssignedInventorySourcesBulkEditArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BulkEditAssignedInventorySourcesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_inventory_source_groups_assigned_inventory_sources_bulk_edit_builder(
            &self.http_client,
            &args.inventorySourceGroupId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_inventory_source_groups_assigned_inventory_sources_bulk_edit_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo inventory source groups assigned inventory sources create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AssignedInventorySource result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_inventory_source_groups_assigned_inventory_sources_create(
        &self,
        args: &DisplayvideoInventorySourceGroupsAssignedInventorySourcesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AssignedInventorySource, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_inventory_source_groups_assigned_inventory_sources_create_builder(
            &self.http_client,
            &args.inventorySourceGroupId,
            &args.advertiserId,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_inventory_source_groups_assigned_inventory_sources_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo inventory source groups assigned inventory sources delete.
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
    pub fn displayvideo_inventory_source_groups_assigned_inventory_sources_delete(
        &self,
        args: &DisplayvideoInventorySourceGroupsAssignedInventorySourcesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_inventory_source_groups_assigned_inventory_sources_delete_builder(
            &self.http_client,
            &args.inventorySourceGroupId,
            &args.assignedInventorySourceId,
            &args.advertiserId,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_inventory_source_groups_assigned_inventory_sources_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo inventory sources create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the InventorySource result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_inventory_sources_create(
        &self,
        args: &DisplayvideoInventorySourcesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<InventorySource, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_inventory_sources_create_builder(
            &self.http_client,
            &args.advertiserId,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_inventory_sources_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo inventory sources edit inventory source read write accessors.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the InventorySourceAccessors result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_inventory_sources_edit_inventory_source_read_write_accessors(
        &self,
        args: &DisplayvideoInventorySourcesEditInventorySourceReadWriteAccessorsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<InventorySourceAccessors, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_inventory_sources_edit_inventory_source_read_write_accessors_builder(
            &self.http_client,
            &args.inventorySourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_inventory_sources_edit_inventory_source_read_write_accessors_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo inventory sources patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the InventorySource result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_inventory_sources_patch(
        &self,
        args: &DisplayvideoInventorySourcesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<InventorySource, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_inventory_sources_patch_builder(
            &self.http_client,
            &args.inventorySourceId,
            &args.advertiserId,
            &args.partnerId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_inventory_sources_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo media upload.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleBytestreamMedia result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_media_upload(
        &self,
        args: &DisplayvideoMediaUploadArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleBytestreamMedia, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_media_upload_builder(
            &self.http_client,
            &args.resourceName,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_media_upload_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo partners edit assigned targeting options.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BulkEditPartnerAssignedTargetingOptionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_partners_edit_assigned_targeting_options(
        &self,
        args: &DisplayvideoPartnersEditAssignedTargetingOptionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BulkEditPartnerAssignedTargetingOptionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_partners_edit_assigned_targeting_options_builder(
            &self.http_client,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_partners_edit_assigned_targeting_options_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo partners channels create.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_partners_channels_create(
        &self,
        args: &DisplayvideoPartnersChannelsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Channel, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_partners_channels_create_builder(
            &self.http_client,
            &args.partnerId,
            &args.advertiserId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_partners_channels_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo partners channels patch.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_partners_channels_patch(
        &self,
        args: &DisplayvideoPartnersChannelsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Channel, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_partners_channels_patch_builder(
            &self.http_client,
            &args.partnerId,
            &args.channelId,
            &args.advertiserId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_partners_channels_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo partners channels sites bulk edit.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BulkEditSitesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_partners_channels_sites_bulk_edit(
        &self,
        args: &DisplayvideoPartnersChannelsSitesBulkEditArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BulkEditSitesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_partners_channels_sites_bulk_edit_builder(
            &self.http_client,
            &args.partnerId,
            &args.channelId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_partners_channels_sites_bulk_edit_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo partners channels sites create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Site result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_partners_channels_sites_create(
        &self,
        args: &DisplayvideoPartnersChannelsSitesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Site, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_partners_channels_sites_create_builder(
            &self.http_client,
            &args.partnerId,
            &args.channelId,
            &args.advertiserId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_partners_channels_sites_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo partners channels sites delete.
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
    pub fn displayvideo_partners_channels_sites_delete(
        &self,
        args: &DisplayvideoPartnersChannelsSitesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_partners_channels_sites_delete_builder(
            &self.http_client,
            &args.partnerId,
            &args.channelId,
            &args.urlOrAppId,
            &args.advertiserId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_partners_channels_sites_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo partners channels sites replace.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ReplaceSitesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_partners_channels_sites_replace(
        &self,
        args: &DisplayvideoPartnersChannelsSitesReplaceArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ReplaceSitesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_partners_channels_sites_replace_builder(
            &self.http_client,
            &args.partnerId,
            &args.channelId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_partners_channels_sites_replace_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo partners targeting types assigned targeting options create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AssignedTargetingOption result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_partners_targeting_types_assigned_targeting_options_create(
        &self,
        args: &DisplayvideoPartnersTargetingTypesAssignedTargetingOptionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AssignedTargetingOption, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_partners_targeting_types_assigned_targeting_options_create_builder(
            &self.http_client,
            &args.partnerId,
            &args.targetingType,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_partners_targeting_types_assigned_targeting_options_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo partners targeting types assigned targeting options delete.
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
    pub fn displayvideo_partners_targeting_types_assigned_targeting_options_delete(
        &self,
        args: &DisplayvideoPartnersTargetingTypesAssignedTargetingOptionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_partners_targeting_types_assigned_targeting_options_delete_builder(
            &self.http_client,
            &args.partnerId,
            &args.targetingType,
            &args.assignedTargetingOptionId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_partners_targeting_types_assigned_targeting_options_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo sdfdownloadtasks create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_sdfdownloadtasks_create(
        &self,
        args: &DisplayvideoSdfdownloadtasksCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_sdfdownloadtasks_create_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_sdfdownloadtasks_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo targeting types targeting options search.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchTargetingOptionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_targeting_types_targeting_options_search(
        &self,
        args: &DisplayvideoTargetingTypesTargetingOptionsSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchTargetingOptionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_targeting_types_targeting_options_search_builder(
            &self.http_client,
            &args.targetingType,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_targeting_types_targeting_options_search_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo users bulk edit assigned user roles.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BulkEditAssignedUserRolesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_users_bulk_edit_assigned_user_roles(
        &self,
        args: &DisplayvideoUsersBulkEditAssignedUserRolesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BulkEditAssignedUserRolesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_users_bulk_edit_assigned_user_roles_builder(
            &self.http_client,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_users_bulk_edit_assigned_user_roles_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo users create.
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
    pub fn displayvideo_users_create(
        &self,
        args: &DisplayvideoUsersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<User, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_users_create_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_users_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo users delete.
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
    pub fn displayvideo_users_delete(
        &self,
        args: &DisplayvideoUsersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_users_delete_builder(
            &self.http_client,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_users_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo users patch.
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
    pub fn displayvideo_users_patch(
        &self,
        args: &DisplayvideoUsersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<User, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_users_patch_builder(
            &self.http_client,
            &args.userId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_users_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
