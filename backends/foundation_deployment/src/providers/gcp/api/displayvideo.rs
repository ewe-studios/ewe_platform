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
    displayvideo_advertisers_audit_builder, displayvideo_advertisers_audit_task,
    displayvideo_advertisers_create_builder, displayvideo_advertisers_create_task,
    displayvideo_advertisers_delete_builder, displayvideo_advertisers_delete_task,
    displayvideo_advertisers_edit_assigned_targeting_options_builder, displayvideo_advertisers_edit_assigned_targeting_options_task,
    displayvideo_advertisers_get_builder, displayvideo_advertisers_get_task,
    displayvideo_advertisers_list_builder, displayvideo_advertisers_list_task,
    displayvideo_advertisers_list_assigned_targeting_options_builder, displayvideo_advertisers_list_assigned_targeting_options_task,
    displayvideo_advertisers_patch_builder, displayvideo_advertisers_patch_task,
    displayvideo_advertisers_ad_assets_bulk_create_builder, displayvideo_advertisers_ad_assets_bulk_create_task,
    displayvideo_advertisers_ad_assets_create_builder, displayvideo_advertisers_ad_assets_create_task,
    displayvideo_advertisers_ad_assets_get_builder, displayvideo_advertisers_ad_assets_get_task,
    displayvideo_advertisers_ad_assets_list_builder, displayvideo_advertisers_ad_assets_list_task,
    displayvideo_advertisers_ad_assets_upload_builder, displayvideo_advertisers_ad_assets_upload_task,
    displayvideo_advertisers_ad_group_ads_create_builder, displayvideo_advertisers_ad_group_ads_create_task,
    displayvideo_advertisers_ad_group_ads_delete_builder, displayvideo_advertisers_ad_group_ads_delete_task,
    displayvideo_advertisers_ad_group_ads_get_builder, displayvideo_advertisers_ad_group_ads_get_task,
    displayvideo_advertisers_ad_group_ads_list_builder, displayvideo_advertisers_ad_group_ads_list_task,
    displayvideo_advertisers_ad_group_ads_patch_builder, displayvideo_advertisers_ad_group_ads_patch_task,
    displayvideo_advertisers_ad_groups_bulk_edit_assigned_targeting_options_builder, displayvideo_advertisers_ad_groups_bulk_edit_assigned_targeting_options_task,
    displayvideo_advertisers_ad_groups_bulk_list_assigned_targeting_options_builder, displayvideo_advertisers_ad_groups_bulk_list_assigned_targeting_options_task,
    displayvideo_advertisers_ad_groups_create_builder, displayvideo_advertisers_ad_groups_create_task,
    displayvideo_advertisers_ad_groups_delete_builder, displayvideo_advertisers_ad_groups_delete_task,
    displayvideo_advertisers_ad_groups_get_builder, displayvideo_advertisers_ad_groups_get_task,
    displayvideo_advertisers_ad_groups_list_builder, displayvideo_advertisers_ad_groups_list_task,
    displayvideo_advertisers_ad_groups_patch_builder, displayvideo_advertisers_ad_groups_patch_task,
    displayvideo_advertisers_ad_groups_targeting_types_assigned_targeting_options_create_builder, displayvideo_advertisers_ad_groups_targeting_types_assigned_targeting_options_create_task,
    displayvideo_advertisers_ad_groups_targeting_types_assigned_targeting_options_delete_builder, displayvideo_advertisers_ad_groups_targeting_types_assigned_targeting_options_delete_task,
    displayvideo_advertisers_ad_groups_targeting_types_assigned_targeting_options_get_builder, displayvideo_advertisers_ad_groups_targeting_types_assigned_targeting_options_get_task,
    displayvideo_advertisers_ad_groups_targeting_types_assigned_targeting_options_list_builder, displayvideo_advertisers_ad_groups_targeting_types_assigned_targeting_options_list_task,
    displayvideo_advertisers_ad_groups_youtube_asset_types_youtube_asset_associations_create_builder, displayvideo_advertisers_ad_groups_youtube_asset_types_youtube_asset_associations_create_task,
    displayvideo_advertisers_ad_groups_youtube_asset_types_youtube_asset_associations_delete_builder, displayvideo_advertisers_ad_groups_youtube_asset_types_youtube_asset_associations_delete_task,
    displayvideo_advertisers_ad_groups_youtube_asset_types_youtube_asset_associations_list_builder, displayvideo_advertisers_ad_groups_youtube_asset_types_youtube_asset_associations_list_task,
    displayvideo_advertisers_assets_upload_builder, displayvideo_advertisers_assets_upload_task,
    displayvideo_advertisers_campaigns_create_builder, displayvideo_advertisers_campaigns_create_task,
    displayvideo_advertisers_campaigns_delete_builder, displayvideo_advertisers_campaigns_delete_task,
    displayvideo_advertisers_campaigns_get_builder, displayvideo_advertisers_campaigns_get_task,
    displayvideo_advertisers_campaigns_list_builder, displayvideo_advertisers_campaigns_list_task,
    displayvideo_advertisers_campaigns_patch_builder, displayvideo_advertisers_campaigns_patch_task,
    displayvideo_advertisers_channels_create_builder, displayvideo_advertisers_channels_create_task,
    displayvideo_advertisers_channels_get_builder, displayvideo_advertisers_channels_get_task,
    displayvideo_advertisers_channels_list_builder, displayvideo_advertisers_channels_list_task,
    displayvideo_advertisers_channels_patch_builder, displayvideo_advertisers_channels_patch_task,
    displayvideo_advertisers_channels_sites_bulk_edit_builder, displayvideo_advertisers_channels_sites_bulk_edit_task,
    displayvideo_advertisers_channels_sites_create_builder, displayvideo_advertisers_channels_sites_create_task,
    displayvideo_advertisers_channels_sites_delete_builder, displayvideo_advertisers_channels_sites_delete_task,
    displayvideo_advertisers_channels_sites_list_builder, displayvideo_advertisers_channels_sites_list_task,
    displayvideo_advertisers_channels_sites_replace_builder, displayvideo_advertisers_channels_sites_replace_task,
    displayvideo_advertisers_creatives_create_builder, displayvideo_advertisers_creatives_create_task,
    displayvideo_advertisers_creatives_delete_builder, displayvideo_advertisers_creatives_delete_task,
    displayvideo_advertisers_creatives_get_builder, displayvideo_advertisers_creatives_get_task,
    displayvideo_advertisers_creatives_list_builder, displayvideo_advertisers_creatives_list_task,
    displayvideo_advertisers_creatives_patch_builder, displayvideo_advertisers_creatives_patch_task,
    displayvideo_advertisers_insertion_orders_create_builder, displayvideo_advertisers_insertion_orders_create_task,
    displayvideo_advertisers_insertion_orders_delete_builder, displayvideo_advertisers_insertion_orders_delete_task,
    displayvideo_advertisers_insertion_orders_get_builder, displayvideo_advertisers_insertion_orders_get_task,
    displayvideo_advertisers_insertion_orders_list_builder, displayvideo_advertisers_insertion_orders_list_task,
    displayvideo_advertisers_insertion_orders_patch_builder, displayvideo_advertisers_insertion_orders_patch_task,
    displayvideo_advertisers_invoices_list_builder, displayvideo_advertisers_invoices_list_task,
    displayvideo_advertisers_invoices_lookup_invoice_currency_builder, displayvideo_advertisers_invoices_lookup_invoice_currency_task,
    displayvideo_advertisers_line_items_bulk_edit_assigned_targeting_options_builder, displayvideo_advertisers_line_items_bulk_edit_assigned_targeting_options_task,
    displayvideo_advertisers_line_items_bulk_list_assigned_targeting_options_builder, displayvideo_advertisers_line_items_bulk_list_assigned_targeting_options_task,
    displayvideo_advertisers_line_items_bulk_update_builder, displayvideo_advertisers_line_items_bulk_update_task,
    displayvideo_advertisers_line_items_create_builder, displayvideo_advertisers_line_items_create_task,
    displayvideo_advertisers_line_items_delete_builder, displayvideo_advertisers_line_items_delete_task,
    displayvideo_advertisers_line_items_duplicate_builder, displayvideo_advertisers_line_items_duplicate_task,
    displayvideo_advertisers_line_items_get_builder, displayvideo_advertisers_line_items_get_task,
    displayvideo_advertisers_line_items_list_builder, displayvideo_advertisers_line_items_list_task,
    displayvideo_advertisers_line_items_patch_builder, displayvideo_advertisers_line_items_patch_task,
    displayvideo_advertisers_line_items_targeting_types_assigned_targeting_options_create_builder, displayvideo_advertisers_line_items_targeting_types_assigned_targeting_options_create_task,
    displayvideo_advertisers_line_items_targeting_types_assigned_targeting_options_delete_builder, displayvideo_advertisers_line_items_targeting_types_assigned_targeting_options_delete_task,
    displayvideo_advertisers_line_items_targeting_types_assigned_targeting_options_get_builder, displayvideo_advertisers_line_items_targeting_types_assigned_targeting_options_get_task,
    displayvideo_advertisers_line_items_targeting_types_assigned_targeting_options_list_builder, displayvideo_advertisers_line_items_targeting_types_assigned_targeting_options_list_task,
    displayvideo_advertisers_line_items_youtube_asset_types_youtube_asset_associations_create_builder, displayvideo_advertisers_line_items_youtube_asset_types_youtube_asset_associations_create_task,
    displayvideo_advertisers_line_items_youtube_asset_types_youtube_asset_associations_delete_builder, displayvideo_advertisers_line_items_youtube_asset_types_youtube_asset_associations_delete_task,
    displayvideo_advertisers_line_items_youtube_asset_types_youtube_asset_associations_list_builder, displayvideo_advertisers_line_items_youtube_asset_types_youtube_asset_associations_list_task,
    displayvideo_advertisers_location_lists_create_builder, displayvideo_advertisers_location_lists_create_task,
    displayvideo_advertisers_location_lists_get_builder, displayvideo_advertisers_location_lists_get_task,
    displayvideo_advertisers_location_lists_list_builder, displayvideo_advertisers_location_lists_list_task,
    displayvideo_advertisers_location_lists_patch_builder, displayvideo_advertisers_location_lists_patch_task,
    displayvideo_advertisers_location_lists_assigned_locations_bulk_edit_builder, displayvideo_advertisers_location_lists_assigned_locations_bulk_edit_task,
    displayvideo_advertisers_location_lists_assigned_locations_create_builder, displayvideo_advertisers_location_lists_assigned_locations_create_task,
    displayvideo_advertisers_location_lists_assigned_locations_delete_builder, displayvideo_advertisers_location_lists_assigned_locations_delete_task,
    displayvideo_advertisers_location_lists_assigned_locations_list_builder, displayvideo_advertisers_location_lists_assigned_locations_list_task,
    displayvideo_advertisers_negative_keyword_lists_create_builder, displayvideo_advertisers_negative_keyword_lists_create_task,
    displayvideo_advertisers_negative_keyword_lists_delete_builder, displayvideo_advertisers_negative_keyword_lists_delete_task,
    displayvideo_advertisers_negative_keyword_lists_get_builder, displayvideo_advertisers_negative_keyword_lists_get_task,
    displayvideo_advertisers_negative_keyword_lists_list_builder, displayvideo_advertisers_negative_keyword_lists_list_task,
    displayvideo_advertisers_negative_keyword_lists_patch_builder, displayvideo_advertisers_negative_keyword_lists_patch_task,
    displayvideo_advertisers_negative_keyword_lists_negative_keywords_bulk_edit_builder, displayvideo_advertisers_negative_keyword_lists_negative_keywords_bulk_edit_task,
    displayvideo_advertisers_negative_keyword_lists_negative_keywords_create_builder, displayvideo_advertisers_negative_keyword_lists_negative_keywords_create_task,
    displayvideo_advertisers_negative_keyword_lists_negative_keywords_delete_builder, displayvideo_advertisers_negative_keyword_lists_negative_keywords_delete_task,
    displayvideo_advertisers_negative_keyword_lists_negative_keywords_list_builder, displayvideo_advertisers_negative_keyword_lists_negative_keywords_list_task,
    displayvideo_advertisers_negative_keyword_lists_negative_keywords_replace_builder, displayvideo_advertisers_negative_keyword_lists_negative_keywords_replace_task,
    displayvideo_advertisers_targeting_types_assigned_targeting_options_create_builder, displayvideo_advertisers_targeting_types_assigned_targeting_options_create_task,
    displayvideo_advertisers_targeting_types_assigned_targeting_options_delete_builder, displayvideo_advertisers_targeting_types_assigned_targeting_options_delete_task,
    displayvideo_advertisers_targeting_types_assigned_targeting_options_get_builder, displayvideo_advertisers_targeting_types_assigned_targeting_options_get_task,
    displayvideo_advertisers_targeting_types_assigned_targeting_options_list_builder, displayvideo_advertisers_targeting_types_assigned_targeting_options_list_task,
    displayvideo_combined_audiences_get_builder, displayvideo_combined_audiences_get_task,
    displayvideo_combined_audiences_list_builder, displayvideo_combined_audiences_list_task,
    displayvideo_custom_bidding_algorithms_create_builder, displayvideo_custom_bidding_algorithms_create_task,
    displayvideo_custom_bidding_algorithms_get_builder, displayvideo_custom_bidding_algorithms_get_task,
    displayvideo_custom_bidding_algorithms_list_builder, displayvideo_custom_bidding_algorithms_list_task,
    displayvideo_custom_bidding_algorithms_patch_builder, displayvideo_custom_bidding_algorithms_patch_task,
    displayvideo_custom_bidding_algorithms_upload_rules_builder, displayvideo_custom_bidding_algorithms_upload_rules_task,
    displayvideo_custom_bidding_algorithms_upload_script_builder, displayvideo_custom_bidding_algorithms_upload_script_task,
    displayvideo_custom_bidding_algorithms_rules_create_builder, displayvideo_custom_bidding_algorithms_rules_create_task,
    displayvideo_custom_bidding_algorithms_rules_get_builder, displayvideo_custom_bidding_algorithms_rules_get_task,
    displayvideo_custom_bidding_algorithms_rules_list_builder, displayvideo_custom_bidding_algorithms_rules_list_task,
    displayvideo_custom_bidding_algorithms_scripts_create_builder, displayvideo_custom_bidding_algorithms_scripts_create_task,
    displayvideo_custom_bidding_algorithms_scripts_get_builder, displayvideo_custom_bidding_algorithms_scripts_get_task,
    displayvideo_custom_bidding_algorithms_scripts_list_builder, displayvideo_custom_bidding_algorithms_scripts_list_task,
    displayvideo_custom_lists_get_builder, displayvideo_custom_lists_get_task,
    displayvideo_custom_lists_list_builder, displayvideo_custom_lists_list_task,
    displayvideo_first_party_and_partner_audiences_create_builder, displayvideo_first_party_and_partner_audiences_create_task,
    displayvideo_first_party_and_partner_audiences_edit_customer_match_members_builder, displayvideo_first_party_and_partner_audiences_edit_customer_match_members_task,
    displayvideo_first_party_and_partner_audiences_get_builder, displayvideo_first_party_and_partner_audiences_get_task,
    displayvideo_first_party_and_partner_audiences_list_builder, displayvideo_first_party_and_partner_audiences_list_task,
    displayvideo_first_party_and_partner_audiences_patch_builder, displayvideo_first_party_and_partner_audiences_patch_task,
    displayvideo_floodlight_groups_get_builder, displayvideo_floodlight_groups_get_task,
    displayvideo_floodlight_groups_patch_builder, displayvideo_floodlight_groups_patch_task,
    displayvideo_floodlight_groups_floodlight_activities_get_builder, displayvideo_floodlight_groups_floodlight_activities_get_task,
    displayvideo_floodlight_groups_floodlight_activities_list_builder, displayvideo_floodlight_groups_floodlight_activities_list_task,
    displayvideo_google_audiences_get_builder, displayvideo_google_audiences_get_task,
    displayvideo_google_audiences_list_builder, displayvideo_google_audiences_list_task,
    displayvideo_guaranteed_orders_create_builder, displayvideo_guaranteed_orders_create_task,
    displayvideo_guaranteed_orders_edit_guaranteed_order_read_accessors_builder, displayvideo_guaranteed_orders_edit_guaranteed_order_read_accessors_task,
    displayvideo_guaranteed_orders_get_builder, displayvideo_guaranteed_orders_get_task,
    displayvideo_guaranteed_orders_list_builder, displayvideo_guaranteed_orders_list_task,
    displayvideo_guaranteed_orders_patch_builder, displayvideo_guaranteed_orders_patch_task,
    displayvideo_inventory_source_groups_create_builder, displayvideo_inventory_source_groups_create_task,
    displayvideo_inventory_source_groups_delete_builder, displayvideo_inventory_source_groups_delete_task,
    displayvideo_inventory_source_groups_get_builder, displayvideo_inventory_source_groups_get_task,
    displayvideo_inventory_source_groups_list_builder, displayvideo_inventory_source_groups_list_task,
    displayvideo_inventory_source_groups_patch_builder, displayvideo_inventory_source_groups_patch_task,
    displayvideo_inventory_source_groups_assigned_inventory_sources_bulk_edit_builder, displayvideo_inventory_source_groups_assigned_inventory_sources_bulk_edit_task,
    displayvideo_inventory_source_groups_assigned_inventory_sources_create_builder, displayvideo_inventory_source_groups_assigned_inventory_sources_create_task,
    displayvideo_inventory_source_groups_assigned_inventory_sources_delete_builder, displayvideo_inventory_source_groups_assigned_inventory_sources_delete_task,
    displayvideo_inventory_source_groups_assigned_inventory_sources_list_builder, displayvideo_inventory_source_groups_assigned_inventory_sources_list_task,
    displayvideo_inventory_sources_create_builder, displayvideo_inventory_sources_create_task,
    displayvideo_inventory_sources_edit_inventory_source_read_write_accessors_builder, displayvideo_inventory_sources_edit_inventory_source_read_write_accessors_task,
    displayvideo_inventory_sources_get_builder, displayvideo_inventory_sources_get_task,
    displayvideo_inventory_sources_list_builder, displayvideo_inventory_sources_list_task,
    displayvideo_inventory_sources_patch_builder, displayvideo_inventory_sources_patch_task,
    displayvideo_media_download_builder, displayvideo_media_download_task,
    displayvideo_media_upload_builder, displayvideo_media_upload_task,
    displayvideo_partners_edit_assigned_targeting_options_builder, displayvideo_partners_edit_assigned_targeting_options_task,
    displayvideo_partners_get_builder, displayvideo_partners_get_task,
    displayvideo_partners_list_builder, displayvideo_partners_list_task,
    displayvideo_partners_channels_create_builder, displayvideo_partners_channels_create_task,
    displayvideo_partners_channels_get_builder, displayvideo_partners_channels_get_task,
    displayvideo_partners_channels_list_builder, displayvideo_partners_channels_list_task,
    displayvideo_partners_channels_patch_builder, displayvideo_partners_channels_patch_task,
    displayvideo_partners_channels_sites_bulk_edit_builder, displayvideo_partners_channels_sites_bulk_edit_task,
    displayvideo_partners_channels_sites_create_builder, displayvideo_partners_channels_sites_create_task,
    displayvideo_partners_channels_sites_delete_builder, displayvideo_partners_channels_sites_delete_task,
    displayvideo_partners_channels_sites_list_builder, displayvideo_partners_channels_sites_list_task,
    displayvideo_partners_channels_sites_replace_builder, displayvideo_partners_channels_sites_replace_task,
    displayvideo_partners_targeting_types_assigned_targeting_options_create_builder, displayvideo_partners_targeting_types_assigned_targeting_options_create_task,
    displayvideo_partners_targeting_types_assigned_targeting_options_delete_builder, displayvideo_partners_targeting_types_assigned_targeting_options_delete_task,
    displayvideo_partners_targeting_types_assigned_targeting_options_get_builder, displayvideo_partners_targeting_types_assigned_targeting_options_get_task,
    displayvideo_partners_targeting_types_assigned_targeting_options_list_builder, displayvideo_partners_targeting_types_assigned_targeting_options_list_task,
    displayvideo_sdfdownloadtasks_create_builder, displayvideo_sdfdownloadtasks_create_task,
    displayvideo_sdfdownloadtasks_operations_get_builder, displayvideo_sdfdownloadtasks_operations_get_task,
    displayvideo_sdfuploadtasks_operations_get_builder, displayvideo_sdfuploadtasks_operations_get_task,
    displayvideo_targeting_types_targeting_options_get_builder, displayvideo_targeting_types_targeting_options_get_task,
    displayvideo_targeting_types_targeting_options_list_builder, displayvideo_targeting_types_targeting_options_list_task,
    displayvideo_targeting_types_targeting_options_search_builder, displayvideo_targeting_types_targeting_options_search_task,
    displayvideo_users_bulk_edit_assigned_user_roles_builder, displayvideo_users_bulk_edit_assigned_user_roles_task,
    displayvideo_users_create_builder, displayvideo_users_create_task,
    displayvideo_users_delete_builder, displayvideo_users_delete_task,
    displayvideo_users_get_builder, displayvideo_users_get_task,
    displayvideo_users_list_builder, displayvideo_users_list_task,
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
use crate::providers::gcp::clients::displayvideo::AuditAdvertiserResponse;
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
use crate::providers::gcp::clients::displayvideo::BulkListAdGroupAssignedTargetingOptionsResponse;
use crate::providers::gcp::clients::displayvideo::BulkListAdvertiserAssignedTargetingOptionsResponse;
use crate::providers::gcp::clients::displayvideo::BulkListAssignedTargetingOptionsResponse;
use crate::providers::gcp::clients::displayvideo::BulkUpdateLineItemsResponse;
use crate::providers::gcp::clients::displayvideo::Campaign;
use crate::providers::gcp::clients::displayvideo::Channel;
use crate::providers::gcp::clients::displayvideo::CombinedAudience;
use crate::providers::gcp::clients::displayvideo::CreateAssetResponse;
use crate::providers::gcp::clients::displayvideo::Creative;
use crate::providers::gcp::clients::displayvideo::CustomBiddingAlgorithm;
use crate::providers::gcp::clients::displayvideo::CustomBiddingAlgorithmRules;
use crate::providers::gcp::clients::displayvideo::CustomBiddingAlgorithmRulesRef;
use crate::providers::gcp::clients::displayvideo::CustomBiddingScript;
use crate::providers::gcp::clients::displayvideo::CustomBiddingScriptRef;
use crate::providers::gcp::clients::displayvideo::CustomList;
use crate::providers::gcp::clients::displayvideo::DuplicateLineItemResponse;
use crate::providers::gcp::clients::displayvideo::EditCustomerMatchMembersResponse;
use crate::providers::gcp::clients::displayvideo::EditGuaranteedOrderReadAccessorsResponse;
use crate::providers::gcp::clients::displayvideo::Empty;
use crate::providers::gcp::clients::displayvideo::FirstPartyAndPartnerAudience;
use crate::providers::gcp::clients::displayvideo::FloodlightActivity;
use crate::providers::gcp::clients::displayvideo::FloodlightGroup;
use crate::providers::gcp::clients::displayvideo::GoogleAudience;
use crate::providers::gcp::clients::displayvideo::GoogleBytestreamMedia;
use crate::providers::gcp::clients::displayvideo::GuaranteedOrder;
use crate::providers::gcp::clients::displayvideo::InsertionOrder;
use crate::providers::gcp::clients::displayvideo::InventorySource;
use crate::providers::gcp::clients::displayvideo::InventorySourceAccessors;
use crate::providers::gcp::clients::displayvideo::InventorySourceGroup;
use crate::providers::gcp::clients::displayvideo::LineItem;
use crate::providers::gcp::clients::displayvideo::ListAdAssetsResponse;
use crate::providers::gcp::clients::displayvideo::ListAdGroupAdsResponse;
use crate::providers::gcp::clients::displayvideo::ListAdGroupAssignedTargetingOptionsResponse;
use crate::providers::gcp::clients::displayvideo::ListAdGroupsResponse;
use crate::providers::gcp::clients::displayvideo::ListAdvertiserAssignedTargetingOptionsResponse;
use crate::providers::gcp::clients::displayvideo::ListAdvertisersResponse;
use crate::providers::gcp::clients::displayvideo::ListAssignedInventorySourcesResponse;
use crate::providers::gcp::clients::displayvideo::ListAssignedLocationsResponse;
use crate::providers::gcp::clients::displayvideo::ListCampaignsResponse;
use crate::providers::gcp::clients::displayvideo::ListChannelsResponse;
use crate::providers::gcp::clients::displayvideo::ListCombinedAudiencesResponse;
use crate::providers::gcp::clients::displayvideo::ListCreativesResponse;
use crate::providers::gcp::clients::displayvideo::ListCustomBiddingAlgorithmRulesResponse;
use crate::providers::gcp::clients::displayvideo::ListCustomBiddingAlgorithmsResponse;
use crate::providers::gcp::clients::displayvideo::ListCustomBiddingScriptsResponse;
use crate::providers::gcp::clients::displayvideo::ListCustomListsResponse;
use crate::providers::gcp::clients::displayvideo::ListFirstPartyAndPartnerAudiencesResponse;
use crate::providers::gcp::clients::displayvideo::ListFloodlightActivitiesResponse;
use crate::providers::gcp::clients::displayvideo::ListGoogleAudiencesResponse;
use crate::providers::gcp::clients::displayvideo::ListGuaranteedOrdersResponse;
use crate::providers::gcp::clients::displayvideo::ListInsertionOrdersResponse;
use crate::providers::gcp::clients::displayvideo::ListInventorySourceGroupsResponse;
use crate::providers::gcp::clients::displayvideo::ListInventorySourcesResponse;
use crate::providers::gcp::clients::displayvideo::ListInvoicesResponse;
use crate::providers::gcp::clients::displayvideo::ListLineItemAssignedTargetingOptionsResponse;
use crate::providers::gcp::clients::displayvideo::ListLineItemsResponse;
use crate::providers::gcp::clients::displayvideo::ListLocationListsResponse;
use crate::providers::gcp::clients::displayvideo::ListNegativeKeywordListsResponse;
use crate::providers::gcp::clients::displayvideo::ListNegativeKeywordsResponse;
use crate::providers::gcp::clients::displayvideo::ListPartnerAssignedTargetingOptionsResponse;
use crate::providers::gcp::clients::displayvideo::ListPartnersResponse;
use crate::providers::gcp::clients::displayvideo::ListSitesResponse;
use crate::providers::gcp::clients::displayvideo::ListTargetingOptionsResponse;
use crate::providers::gcp::clients::displayvideo::ListUsersResponse;
use crate::providers::gcp::clients::displayvideo::ListYoutubeAssetAssociationsResponse;
use crate::providers::gcp::clients::displayvideo::LocationList;
use crate::providers::gcp::clients::displayvideo::LookupInvoiceCurrencyResponse;
use crate::providers::gcp::clients::displayvideo::NegativeKeyword;
use crate::providers::gcp::clients::displayvideo::NegativeKeywordList;
use crate::providers::gcp::clients::displayvideo::Operation;
use crate::providers::gcp::clients::displayvideo::Partner;
use crate::providers::gcp::clients::displayvideo::ReplaceNegativeKeywordsResponse;
use crate::providers::gcp::clients::displayvideo::ReplaceSitesResponse;
use crate::providers::gcp::clients::displayvideo::SearchTargetingOptionsResponse;
use crate::providers::gcp::clients::displayvideo::Site;
use crate::providers::gcp::clients::displayvideo::TargetingOption;
use crate::providers::gcp::clients::displayvideo::UploadAdAssetResponse;
use crate::providers::gcp::clients::displayvideo::User;
use crate::providers::gcp::clients::displayvideo::YoutubeAssetAssociation;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersAdAssetsBulkCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersAdAssetsCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersAdAssetsGetArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersAdAssetsListArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersAdAssetsUploadArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersAdGroupAdsCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersAdGroupAdsDeleteArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersAdGroupAdsGetArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersAdGroupAdsListArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersAdGroupAdsPatchArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersAdGroupsBulkEditAssignedTargetingOptionsArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersAdGroupsBulkListAssignedTargetingOptionsArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersAdGroupsCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersAdGroupsDeleteArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersAdGroupsGetArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersAdGroupsListArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersAdGroupsPatchArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersAdGroupsTargetingTypesAssignedTargetingOptionsCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersAdGroupsTargetingTypesAssignedTargetingOptionsDeleteArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersAdGroupsTargetingTypesAssignedTargetingOptionsGetArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersAdGroupsTargetingTypesAssignedTargetingOptionsListArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersAdGroupsYoutubeAssetTypesYoutubeAssetAssociationsCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersAdGroupsYoutubeAssetTypesYoutubeAssetAssociationsDeleteArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersAdGroupsYoutubeAssetTypesYoutubeAssetAssociationsListArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersAssetsUploadArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersAuditArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersCampaignsCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersCampaignsDeleteArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersCampaignsGetArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersCampaignsListArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersCampaignsPatchArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersChannelsCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersChannelsGetArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersChannelsListArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersChannelsPatchArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersChannelsSitesBulkEditArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersChannelsSitesCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersChannelsSitesDeleteArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersChannelsSitesListArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersChannelsSitesReplaceArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersCreativesCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersCreativesDeleteArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersCreativesGetArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersCreativesListArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersCreativesPatchArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersDeleteArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersEditAssignedTargetingOptionsArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersGetArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersInsertionOrdersCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersInsertionOrdersDeleteArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersInsertionOrdersGetArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersInsertionOrdersListArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersInsertionOrdersPatchArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersInvoicesListArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersInvoicesLookupInvoiceCurrencyArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersLineItemsBulkEditAssignedTargetingOptionsArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersLineItemsBulkListAssignedTargetingOptionsArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersLineItemsBulkUpdateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersLineItemsCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersLineItemsDeleteArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersLineItemsDuplicateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersLineItemsGetArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersLineItemsListArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersLineItemsPatchArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersLineItemsTargetingTypesAssignedTargetingOptionsCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersLineItemsTargetingTypesAssignedTargetingOptionsDeleteArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersLineItemsTargetingTypesAssignedTargetingOptionsGetArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersLineItemsTargetingTypesAssignedTargetingOptionsListArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersLineItemsYoutubeAssetTypesYoutubeAssetAssociationsCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersLineItemsYoutubeAssetTypesYoutubeAssetAssociationsDeleteArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersLineItemsYoutubeAssetTypesYoutubeAssetAssociationsListArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersListArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersListAssignedTargetingOptionsArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersLocationListsAssignedLocationsBulkEditArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersLocationListsAssignedLocationsCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersLocationListsAssignedLocationsDeleteArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersLocationListsAssignedLocationsListArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersLocationListsCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersLocationListsGetArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersLocationListsListArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersLocationListsPatchArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersNegativeKeywordListsCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersNegativeKeywordListsDeleteArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersNegativeKeywordListsGetArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersNegativeKeywordListsListArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersNegativeKeywordListsNegativeKeywordsBulkEditArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersNegativeKeywordListsNegativeKeywordsCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersNegativeKeywordListsNegativeKeywordsDeleteArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersNegativeKeywordListsNegativeKeywordsListArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersNegativeKeywordListsNegativeKeywordsReplaceArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersNegativeKeywordListsPatchArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersPatchArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersTargetingTypesAssignedTargetingOptionsCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersTargetingTypesAssignedTargetingOptionsDeleteArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersTargetingTypesAssignedTargetingOptionsGetArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoAdvertisersTargetingTypesAssignedTargetingOptionsListArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoCombinedAudiencesGetArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoCombinedAudiencesListArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoCustomBiddingAlgorithmsCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoCustomBiddingAlgorithmsGetArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoCustomBiddingAlgorithmsListArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoCustomBiddingAlgorithmsPatchArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoCustomBiddingAlgorithmsRulesCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoCustomBiddingAlgorithmsRulesGetArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoCustomBiddingAlgorithmsRulesListArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoCustomBiddingAlgorithmsScriptsCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoCustomBiddingAlgorithmsScriptsGetArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoCustomBiddingAlgorithmsScriptsListArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoCustomBiddingAlgorithmsUploadRulesArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoCustomBiddingAlgorithmsUploadScriptArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoCustomListsGetArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoCustomListsListArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoFirstPartyAndPartnerAudiencesCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoFirstPartyAndPartnerAudiencesEditCustomerMatchMembersArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoFirstPartyAndPartnerAudiencesGetArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoFirstPartyAndPartnerAudiencesListArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoFirstPartyAndPartnerAudiencesPatchArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoFloodlightGroupsFloodlightActivitiesGetArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoFloodlightGroupsFloodlightActivitiesListArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoFloodlightGroupsGetArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoFloodlightGroupsPatchArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoGoogleAudiencesGetArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoGoogleAudiencesListArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoGuaranteedOrdersCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoGuaranteedOrdersEditGuaranteedOrderReadAccessorsArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoGuaranteedOrdersGetArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoGuaranteedOrdersListArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoGuaranteedOrdersPatchArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoInventorySourceGroupsAssignedInventorySourcesBulkEditArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoInventorySourceGroupsAssignedInventorySourcesCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoInventorySourceGroupsAssignedInventorySourcesDeleteArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoInventorySourceGroupsAssignedInventorySourcesListArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoInventorySourceGroupsCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoInventorySourceGroupsDeleteArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoInventorySourceGroupsGetArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoInventorySourceGroupsListArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoInventorySourceGroupsPatchArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoInventorySourcesCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoInventorySourcesEditInventorySourceReadWriteAccessorsArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoInventorySourcesGetArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoInventorySourcesListArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoInventorySourcesPatchArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoMediaDownloadArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoMediaUploadArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoPartnersChannelsCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoPartnersChannelsGetArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoPartnersChannelsListArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoPartnersChannelsPatchArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoPartnersChannelsSitesBulkEditArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoPartnersChannelsSitesCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoPartnersChannelsSitesDeleteArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoPartnersChannelsSitesListArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoPartnersChannelsSitesReplaceArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoPartnersEditAssignedTargetingOptionsArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoPartnersGetArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoPartnersListArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoPartnersTargetingTypesAssignedTargetingOptionsCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoPartnersTargetingTypesAssignedTargetingOptionsDeleteArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoPartnersTargetingTypesAssignedTargetingOptionsGetArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoPartnersTargetingTypesAssignedTargetingOptionsListArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoSdfdownloadtasksCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoSdfdownloadtasksOperationsGetArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoSdfuploadtasksOperationsGetArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoTargetingTypesTargetingOptionsGetArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoTargetingTypesTargetingOptionsListArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoTargetingTypesTargetingOptionsSearchArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoUsersBulkEditAssignedUserRolesArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoUsersCreateArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoUsersDeleteArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoUsersGetArgs;
use crate::providers::gcp::clients::displayvideo::DisplayvideoUsersListArgs;
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

    /// Displayvideo advertisers audit.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AuditAdvertiserResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_advertisers_audit(
        &self,
        args: &DisplayvideoAdvertisersAuditArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AuditAdvertiserResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_audit_builder(
            &self.http_client,
            &args.advertiserId,
            &args.readMask,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_audit_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_advertisers_get(
        &self,
        args: &DisplayvideoAdvertisersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Advertiser, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_get_builder(
            &self.http_client,
            &args.advertiserId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAdvertisersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_advertisers_list(
        &self,
        args: &DisplayvideoAdvertisersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAdvertisersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_list_builder(
            &self.http_client,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers list assigned targeting options.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BulkListAdvertiserAssignedTargetingOptionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_advertisers_list_assigned_targeting_options(
        &self,
        args: &DisplayvideoAdvertisersListAssignedTargetingOptionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BulkListAdvertiserAssignedTargetingOptionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_list_assigned_targeting_options_builder(
            &self.http_client,
            &args.advertiserId,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_list_assigned_targeting_options_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Displayvideo advertisers ad assets get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_advertisers_ad_assets_get(
        &self,
        args: &DisplayvideoAdvertisersAdAssetsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AdAsset, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_ad_assets_get_builder(
            &self.http_client,
            &args.advertiserId,
            &args.adAssetId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_ad_assets_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers ad assets list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAdAssetsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_advertisers_ad_assets_list(
        &self,
        args: &DisplayvideoAdvertisersAdAssetsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAdAssetsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_ad_assets_list_builder(
            &self.http_client,
            &args.advertiserId,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_ad_assets_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Displayvideo advertisers ad group ads get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_advertisers_ad_group_ads_get(
        &self,
        args: &DisplayvideoAdvertisersAdGroupAdsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AdGroupAd, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_ad_group_ads_get_builder(
            &self.http_client,
            &args.advertiserId,
            &args.adGroupAdId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_ad_group_ads_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers ad group ads list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAdGroupAdsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_advertisers_ad_group_ads_list(
        &self,
        args: &DisplayvideoAdvertisersAdGroupAdsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAdGroupAdsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_ad_group_ads_list_builder(
            &self.http_client,
            &args.advertiserId,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_ad_group_ads_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers ad groups bulk list assigned targeting options.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BulkListAdGroupAssignedTargetingOptionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_advertisers_ad_groups_bulk_list_assigned_targeting_options(
        &self,
        args: &DisplayvideoAdvertisersAdGroupsBulkListAssignedTargetingOptionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BulkListAdGroupAssignedTargetingOptionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_ad_groups_bulk_list_assigned_targeting_options_builder(
            &self.http_client,
            &args.advertiserId,
            &args.adGroupIds,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_ad_groups_bulk_list_assigned_targeting_options_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Displayvideo advertisers ad groups get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_advertisers_ad_groups_get(
        &self,
        args: &DisplayvideoAdvertisersAdGroupsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AdGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_ad_groups_get_builder(
            &self.http_client,
            &args.advertiserId,
            &args.adGroupId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_ad_groups_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers ad groups list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAdGroupsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_advertisers_ad_groups_list(
        &self,
        args: &DisplayvideoAdvertisersAdGroupsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAdGroupsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_ad_groups_list_builder(
            &self.http_client,
            &args.advertiserId,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_ad_groups_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers ad groups targeting types assigned targeting options get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_advertisers_ad_groups_targeting_types_assigned_targeting_options_get(
        &self,
        args: &DisplayvideoAdvertisersAdGroupsTargetingTypesAssignedTargetingOptionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AssignedTargetingOption, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_ad_groups_targeting_types_assigned_targeting_options_get_builder(
            &self.http_client,
            &args.advertiserId,
            &args.adGroupId,
            &args.targetingType,
            &args.assignedTargetingOptionId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_ad_groups_targeting_types_assigned_targeting_options_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers ad groups targeting types assigned targeting options list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAdGroupAssignedTargetingOptionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_advertisers_ad_groups_targeting_types_assigned_targeting_options_list(
        &self,
        args: &DisplayvideoAdvertisersAdGroupsTargetingTypesAssignedTargetingOptionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAdGroupAssignedTargetingOptionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_ad_groups_targeting_types_assigned_targeting_options_list_builder(
            &self.http_client,
            &args.advertiserId,
            &args.adGroupId,
            &args.targetingType,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_ad_groups_targeting_types_assigned_targeting_options_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Displayvideo advertisers ad groups youtube asset types youtube asset associations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListYoutubeAssetAssociationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_advertisers_ad_groups_youtube_asset_types_youtube_asset_associations_list(
        &self,
        args: &DisplayvideoAdvertisersAdGroupsYoutubeAssetTypesYoutubeAssetAssociationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListYoutubeAssetAssociationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_ad_groups_youtube_asset_types_youtube_asset_associations_list_builder(
            &self.http_client,
            &args.advertiserId,
            &args.adGroupId,
            &args.youtubeAssetType,
            &args.linkedEntity.lineItemId,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_ad_groups_youtube_asset_types_youtube_asset_associations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Displayvideo advertisers campaigns get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_advertisers_campaigns_get(
        &self,
        args: &DisplayvideoAdvertisersCampaignsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Campaign, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_campaigns_get_builder(
            &self.http_client,
            &args.advertiserId,
            &args.campaignId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_campaigns_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers campaigns list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCampaignsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_advertisers_campaigns_list(
        &self,
        args: &DisplayvideoAdvertisersCampaignsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCampaignsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_campaigns_list_builder(
            &self.http_client,
            &args.advertiserId,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_campaigns_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Displayvideo advertisers channels get.
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
    pub fn displayvideo_advertisers_channels_get(
        &self,
        args: &DisplayvideoAdvertisersChannelsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Channel, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_channels_get_builder(
            &self.http_client,
            &args.advertiserId,
            &args.channelId,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_channels_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers channels list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListChannelsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_advertisers_channels_list(
        &self,
        args: &DisplayvideoAdvertisersChannelsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListChannelsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_channels_list_builder(
            &self.http_client,
            &args.advertiserId,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_channels_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Displayvideo advertisers channels sites list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListSitesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_advertisers_channels_sites_list(
        &self,
        args: &DisplayvideoAdvertisersChannelsSitesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSitesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_channels_sites_list_builder(
            &self.http_client,
            &args.advertiserId,
            &args.channelId,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_channels_sites_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Displayvideo advertisers creatives get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_advertisers_creatives_get(
        &self,
        args: &DisplayvideoAdvertisersCreativesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Creative, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_creatives_get_builder(
            &self.http_client,
            &args.advertiserId,
            &args.creativeId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_creatives_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers creatives list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCreativesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_advertisers_creatives_list(
        &self,
        args: &DisplayvideoAdvertisersCreativesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCreativesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_creatives_list_builder(
            &self.http_client,
            &args.advertiserId,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_creatives_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Displayvideo advertisers insertion orders get.
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
    pub fn displayvideo_advertisers_insertion_orders_get(
        &self,
        args: &DisplayvideoAdvertisersInsertionOrdersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<InsertionOrder, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_insertion_orders_get_builder(
            &self.http_client,
            &args.advertiserId,
            &args.insertionOrderId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_insertion_orders_get_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers insertion orders list.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListInsertionOrdersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn displayvideo_advertisers_insertion_orders_list(
        &self,
        args: &DisplayvideoAdvertisersInsertionOrdersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListInsertionOrdersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_insertion_orders_list_builder(
            &self.http_client,
            &args.advertiserId,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_insertion_orders_list_task(builder)
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

    /// Displayvideo advertisers invoices list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListInvoicesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_advertisers_invoices_list(
        &self,
        args: &DisplayvideoAdvertisersInvoicesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListInvoicesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_invoices_list_builder(
            &self.http_client,
            &args.advertiserId,
            &args.issueMonth,
            &args.loiSapinInvoiceType,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_invoices_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers invoices lookup invoice currency.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LookupInvoiceCurrencyResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_advertisers_invoices_lookup_invoice_currency(
        &self,
        args: &DisplayvideoAdvertisersInvoicesLookupInvoiceCurrencyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LookupInvoiceCurrencyResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_invoices_lookup_invoice_currency_builder(
            &self.http_client,
            &args.advertiserId,
            &args.invoiceMonth,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_invoices_lookup_invoice_currency_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers line items bulk edit assigned targeting options.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers line items bulk list assigned targeting options.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BulkListAssignedTargetingOptionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_advertisers_line_items_bulk_list_assigned_targeting_options(
        &self,
        args: &DisplayvideoAdvertisersLineItemsBulkListAssignedTargetingOptionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BulkListAssignedTargetingOptionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_line_items_bulk_list_assigned_targeting_options_builder(
            &self.http_client,
            &args.advertiserId,
            &args.filter,
            &args.lineItemIds,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_line_items_bulk_list_assigned_targeting_options_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Displayvideo advertisers line items get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_advertisers_line_items_get(
        &self,
        args: &DisplayvideoAdvertisersLineItemsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LineItem, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_line_items_get_builder(
            &self.http_client,
            &args.advertiserId,
            &args.lineItemId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_line_items_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers line items list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListLineItemsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_advertisers_line_items_list(
        &self,
        args: &DisplayvideoAdvertisersLineItemsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLineItemsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_line_items_list_builder(
            &self.http_client,
            &args.advertiserId,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_line_items_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers line items targeting types assigned targeting options get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_advertisers_line_items_targeting_types_assigned_targeting_options_get(
        &self,
        args: &DisplayvideoAdvertisersLineItemsTargetingTypesAssignedTargetingOptionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AssignedTargetingOption, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_line_items_targeting_types_assigned_targeting_options_get_builder(
            &self.http_client,
            &args.advertiserId,
            &args.lineItemId,
            &args.targetingType,
            &args.assignedTargetingOptionId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_line_items_targeting_types_assigned_targeting_options_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers line items targeting types assigned targeting options list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListLineItemAssignedTargetingOptionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_advertisers_line_items_targeting_types_assigned_targeting_options_list(
        &self,
        args: &DisplayvideoAdvertisersLineItemsTargetingTypesAssignedTargetingOptionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLineItemAssignedTargetingOptionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_line_items_targeting_types_assigned_targeting_options_list_builder(
            &self.http_client,
            &args.advertiserId,
            &args.lineItemId,
            &args.targetingType,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_line_items_targeting_types_assigned_targeting_options_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Displayvideo advertisers line items youtube asset types youtube asset associations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListYoutubeAssetAssociationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_advertisers_line_items_youtube_asset_types_youtube_asset_associations_list(
        &self,
        args: &DisplayvideoAdvertisersLineItemsYoutubeAssetTypesYoutubeAssetAssociationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListYoutubeAssetAssociationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_line_items_youtube_asset_types_youtube_asset_associations_list_builder(
            &self.http_client,
            &args.advertiserId,
            &args.lineItemId,
            &args.youtubeAssetType,
            &args.linkedEntity.adGroupId,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_line_items_youtube_asset_types_youtube_asset_associations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Displayvideo advertisers location lists get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_advertisers_location_lists_get(
        &self,
        args: &DisplayvideoAdvertisersLocationListsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LocationList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_location_lists_get_builder(
            &self.http_client,
            &args.advertiserId,
            &args.locationListId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_location_lists_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers location lists list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListLocationListsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_advertisers_location_lists_list(
        &self,
        args: &DisplayvideoAdvertisersLocationListsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationListsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_location_lists_list_builder(
            &self.http_client,
            &args.advertiserId,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_location_lists_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers location lists patch.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers location lists assigned locations bulk edit.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers location lists assigned locations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAssignedLocationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_advertisers_location_lists_assigned_locations_list(
        &self,
        args: &DisplayvideoAdvertisersLocationListsAssignedLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAssignedLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_location_lists_assigned_locations_list_builder(
            &self.http_client,
            &args.advertiserId,
            &args.locationListId,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_location_lists_assigned_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers negative keyword lists get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_advertisers_negative_keyword_lists_get(
        &self,
        args: &DisplayvideoAdvertisersNegativeKeywordListsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NegativeKeywordList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_negative_keyword_lists_get_builder(
            &self.http_client,
            &args.advertiserId,
            &args.negativeKeywordListId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_negative_keyword_lists_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers negative keyword lists list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListNegativeKeywordListsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_advertisers_negative_keyword_lists_list(
        &self,
        args: &DisplayvideoAdvertisersNegativeKeywordListsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListNegativeKeywordListsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_negative_keyword_lists_list_builder(
            &self.http_client,
            &args.advertiserId,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_negative_keyword_lists_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers negative keyword lists patch.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers negative keyword lists negative keywords bulk edit.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers negative keyword lists negative keywords list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListNegativeKeywordsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_advertisers_negative_keyword_lists_negative_keywords_list(
        &self,
        args: &DisplayvideoAdvertisersNegativeKeywordListsNegativeKeywordsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListNegativeKeywordsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_negative_keyword_lists_negative_keywords_list_builder(
            &self.http_client,
            &args.advertiserId,
            &args.negativeKeywordListId,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_negative_keyword_lists_negative_keywords_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers negative keyword lists negative keywords replace.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers targeting types assigned targeting options get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_advertisers_targeting_types_assigned_targeting_options_get(
        &self,
        args: &DisplayvideoAdvertisersTargetingTypesAssignedTargetingOptionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AssignedTargetingOption, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_targeting_types_assigned_targeting_options_get_builder(
            &self.http_client,
            &args.advertiserId,
            &args.targetingType,
            &args.assignedTargetingOptionId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_targeting_types_assigned_targeting_options_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo advertisers targeting types assigned targeting options list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAdvertiserAssignedTargetingOptionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_advertisers_targeting_types_assigned_targeting_options_list(
        &self,
        args: &DisplayvideoAdvertisersTargetingTypesAssignedTargetingOptionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAdvertiserAssignedTargetingOptionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_advertisers_targeting_types_assigned_targeting_options_list_builder(
            &self.http_client,
            &args.advertiserId,
            &args.targetingType,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_advertisers_targeting_types_assigned_targeting_options_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo combined audiences get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CombinedAudience result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_combined_audiences_get(
        &self,
        args: &DisplayvideoCombinedAudiencesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CombinedAudience, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_combined_audiences_get_builder(
            &self.http_client,
            &args.combinedAudienceId,
            &args.advertiserId,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_combined_audiences_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo combined audiences list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCombinedAudiencesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_combined_audiences_list(
        &self,
        args: &DisplayvideoCombinedAudiencesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCombinedAudiencesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_combined_audiences_list_builder(
            &self.http_client,
            &args.advertiserId,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_combined_audiences_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Displayvideo custom bidding algorithms get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_custom_bidding_algorithms_get(
        &self,
        args: &DisplayvideoCustomBiddingAlgorithmsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CustomBiddingAlgorithm, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_custom_bidding_algorithms_get_builder(
            &self.http_client,
            &args.customBiddingAlgorithmId,
            &args.advertiserId,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_custom_bidding_algorithms_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo custom bidding algorithms list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCustomBiddingAlgorithmsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_custom_bidding_algorithms_list(
        &self,
        args: &DisplayvideoCustomBiddingAlgorithmsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCustomBiddingAlgorithmsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_custom_bidding_algorithms_list_builder(
            &self.http_client,
            &args.advertiserId,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_custom_bidding_algorithms_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Displayvideo custom bidding algorithms upload rules.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CustomBiddingAlgorithmRulesRef result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_custom_bidding_algorithms_upload_rules(
        &self,
        args: &DisplayvideoCustomBiddingAlgorithmsUploadRulesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CustomBiddingAlgorithmRulesRef, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_custom_bidding_algorithms_upload_rules_builder(
            &self.http_client,
            &args.customBiddingAlgorithmId,
            &args.advertiserId,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_custom_bidding_algorithms_upload_rules_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo custom bidding algorithms upload script.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CustomBiddingScriptRef result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_custom_bidding_algorithms_upload_script(
        &self,
        args: &DisplayvideoCustomBiddingAlgorithmsUploadScriptArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CustomBiddingScriptRef, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_custom_bidding_algorithms_upload_script_builder(
            &self.http_client,
            &args.customBiddingAlgorithmId,
            &args.advertiserId,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_custom_bidding_algorithms_upload_script_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Displayvideo custom bidding algorithms rules get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_custom_bidding_algorithms_rules_get(
        &self,
        args: &DisplayvideoCustomBiddingAlgorithmsRulesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CustomBiddingAlgorithmRules, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_custom_bidding_algorithms_rules_get_builder(
            &self.http_client,
            &args.customBiddingAlgorithmId,
            &args.customBiddingAlgorithmRulesId,
            &args.advertiserId,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_custom_bidding_algorithms_rules_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo custom bidding algorithms rules list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCustomBiddingAlgorithmRulesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_custom_bidding_algorithms_rules_list(
        &self,
        args: &DisplayvideoCustomBiddingAlgorithmsRulesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCustomBiddingAlgorithmRulesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_custom_bidding_algorithms_rules_list_builder(
            &self.http_client,
            &args.customBiddingAlgorithmId,
            &args.advertiserId,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_custom_bidding_algorithms_rules_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Displayvideo custom bidding algorithms scripts get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_custom_bidding_algorithms_scripts_get(
        &self,
        args: &DisplayvideoCustomBiddingAlgorithmsScriptsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CustomBiddingScript, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_custom_bidding_algorithms_scripts_get_builder(
            &self.http_client,
            &args.customBiddingAlgorithmId,
            &args.customBiddingScriptId,
            &args.advertiserId,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_custom_bidding_algorithms_scripts_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo custom bidding algorithms scripts list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCustomBiddingScriptsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_custom_bidding_algorithms_scripts_list(
        &self,
        args: &DisplayvideoCustomBiddingAlgorithmsScriptsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCustomBiddingScriptsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_custom_bidding_algorithms_scripts_list_builder(
            &self.http_client,
            &args.customBiddingAlgorithmId,
            &args.advertiserId,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_custom_bidding_algorithms_scripts_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo custom lists get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CustomList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_custom_lists_get(
        &self,
        args: &DisplayvideoCustomListsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CustomList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_custom_lists_get_builder(
            &self.http_client,
            &args.customListId,
            &args.advertiserId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_custom_lists_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo custom lists list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCustomListsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_custom_lists_list(
        &self,
        args: &DisplayvideoCustomListsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCustomListsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_custom_lists_list_builder(
            &self.http_client,
            &args.advertiserId,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_custom_lists_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Displayvideo first party and partner audiences get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_first_party_and_partner_audiences_get(
        &self,
        args: &DisplayvideoFirstPartyAndPartnerAudiencesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FirstPartyAndPartnerAudience, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_first_party_and_partner_audiences_get_builder(
            &self.http_client,
            &args.firstPartyAndPartnerAudienceId,
            &args.advertiserId,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_first_party_and_partner_audiences_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo first party and partner audiences list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListFirstPartyAndPartnerAudiencesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_first_party_and_partner_audiences_list(
        &self,
        args: &DisplayvideoFirstPartyAndPartnerAudiencesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListFirstPartyAndPartnerAudiencesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_first_party_and_partner_audiences_list_builder(
            &self.http_client,
            &args.advertiserId,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_first_party_and_partner_audiences_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Displayvideo floodlight groups get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_floodlight_groups_get(
        &self,
        args: &DisplayvideoFloodlightGroupsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FloodlightGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_floodlight_groups_get_builder(
            &self.http_client,
            &args.floodlightGroupId,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_floodlight_groups_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Displayvideo floodlight groups floodlight activities get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FloodlightActivity result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_floodlight_groups_floodlight_activities_get(
        &self,
        args: &DisplayvideoFloodlightGroupsFloodlightActivitiesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FloodlightActivity, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_floodlight_groups_floodlight_activities_get_builder(
            &self.http_client,
            &args.floodlightGroupId,
            &args.floodlightActivityId,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_floodlight_groups_floodlight_activities_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo floodlight groups floodlight activities list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListFloodlightActivitiesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_floodlight_groups_floodlight_activities_list(
        &self,
        args: &DisplayvideoFloodlightGroupsFloodlightActivitiesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListFloodlightActivitiesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_floodlight_groups_floodlight_activities_list_builder(
            &self.http_client,
            &args.floodlightGroupId,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_floodlight_groups_floodlight_activities_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo google audiences get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAudience result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_google_audiences_get(
        &self,
        args: &DisplayvideoGoogleAudiencesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAudience, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_google_audiences_get_builder(
            &self.http_client,
            &args.googleAudienceId,
            &args.advertiserId,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_google_audiences_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo google audiences list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListGoogleAudiencesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_google_audiences_list(
        &self,
        args: &DisplayvideoGoogleAudiencesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListGoogleAudiencesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_google_audiences_list_builder(
            &self.http_client,
            &args.advertiserId,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_google_audiences_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Displayvideo guaranteed orders get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_guaranteed_orders_get(
        &self,
        args: &DisplayvideoGuaranteedOrdersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GuaranteedOrder, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_guaranteed_orders_get_builder(
            &self.http_client,
            &args.guaranteedOrderId,
            &args.advertiserId,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_guaranteed_orders_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo guaranteed orders list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListGuaranteedOrdersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_guaranteed_orders_list(
        &self,
        args: &DisplayvideoGuaranteedOrdersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListGuaranteedOrdersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_guaranteed_orders_list_builder(
            &self.http_client,
            &args.advertiserId,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_guaranteed_orders_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Displayvideo inventory source groups get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_inventory_source_groups_get(
        &self,
        args: &DisplayvideoInventorySourceGroupsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<InventorySourceGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_inventory_source_groups_get_builder(
            &self.http_client,
            &args.inventorySourceGroupId,
            &args.advertiserId,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_inventory_source_groups_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo inventory source groups list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListInventorySourceGroupsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_inventory_source_groups_list(
        &self,
        args: &DisplayvideoInventorySourceGroupsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListInventorySourceGroupsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_inventory_source_groups_list_builder(
            &self.http_client,
            &args.advertiserId,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_inventory_source_groups_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Displayvideo inventory source groups assigned inventory sources list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAssignedInventorySourcesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_inventory_source_groups_assigned_inventory_sources_list(
        &self,
        args: &DisplayvideoInventorySourceGroupsAssignedInventorySourcesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAssignedInventorySourcesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_inventory_source_groups_assigned_inventory_sources_list_builder(
            &self.http_client,
            &args.inventorySourceGroupId,
            &args.advertiserId,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_inventory_source_groups_assigned_inventory_sources_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Displayvideo inventory sources get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_inventory_sources_get(
        &self,
        args: &DisplayvideoInventorySourcesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<InventorySource, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_inventory_sources_get_builder(
            &self.http_client,
            &args.inventorySourceId,
            &args.advertiserId,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_inventory_sources_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo inventory sources list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListInventorySourcesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_inventory_sources_list(
        &self,
        args: &DisplayvideoInventorySourcesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListInventorySourcesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_inventory_sources_list_builder(
            &self.http_client,
            &args.advertiserId,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_inventory_sources_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Displayvideo media download.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_media_download(
        &self,
        args: &DisplayvideoMediaDownloadArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleBytestreamMedia, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_media_download_builder(
            &self.http_client,
            &args.resourceName,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_media_download_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo partners get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Partner result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_partners_get(
        &self,
        args: &DisplayvideoPartnersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Partner, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_partners_get_builder(
            &self.http_client,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_partners_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo partners list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListPartnersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_partners_list(
        &self,
        args: &DisplayvideoPartnersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListPartnersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_partners_list_builder(
            &self.http_client,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_partners_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Displayvideo partners channels get.
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
    pub fn displayvideo_partners_channels_get(
        &self,
        args: &DisplayvideoPartnersChannelsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Channel, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_partners_channels_get_builder(
            &self.http_client,
            &args.partnerId,
            &args.channelId,
            &args.advertiserId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_partners_channels_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo partners channels list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListChannelsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_partners_channels_list(
        &self,
        args: &DisplayvideoPartnersChannelsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListChannelsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_partners_channels_list_builder(
            &self.http_client,
            &args.partnerId,
            &args.advertiserId,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_partners_channels_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Displayvideo partners channels sites list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListSitesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_partners_channels_sites_list(
        &self,
        args: &DisplayvideoPartnersChannelsSitesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSitesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_partners_channels_sites_list_builder(
            &self.http_client,
            &args.partnerId,
            &args.channelId,
            &args.advertiserId,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_partners_channels_sites_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo partners targeting types assigned targeting options get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_partners_targeting_types_assigned_targeting_options_get(
        &self,
        args: &DisplayvideoPartnersTargetingTypesAssignedTargetingOptionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AssignedTargetingOption, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_partners_targeting_types_assigned_targeting_options_get_builder(
            &self.http_client,
            &args.partnerId,
            &args.targetingType,
            &args.assignedTargetingOptionId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_partners_targeting_types_assigned_targeting_options_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo partners targeting types assigned targeting options list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListPartnerAssignedTargetingOptionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_partners_targeting_types_assigned_targeting_options_list(
        &self,
        args: &DisplayvideoPartnersTargetingTypesAssignedTargetingOptionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListPartnerAssignedTargetingOptionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_partners_targeting_types_assigned_targeting_options_list_builder(
            &self.http_client,
            &args.partnerId,
            &args.targetingType,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_partners_targeting_types_assigned_targeting_options_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Displayvideo sdfdownloadtasks operations get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_sdfdownloadtasks_operations_get(
        &self,
        args: &DisplayvideoSdfdownloadtasksOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_sdfdownloadtasks_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_sdfdownloadtasks_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo sdfuploadtasks operations get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_sdfuploadtasks_operations_get(
        &self,
        args: &DisplayvideoSdfuploadtasksOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_sdfuploadtasks_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_sdfuploadtasks_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo targeting types targeting options get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TargetingOption result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_targeting_types_targeting_options_get(
        &self,
        args: &DisplayvideoTargetingTypesTargetingOptionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TargetingOption, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_targeting_types_targeting_options_get_builder(
            &self.http_client,
            &args.targetingType,
            &args.targetingOptionId,
            &args.advertiserId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_targeting_types_targeting_options_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo targeting types targeting options list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListTargetingOptionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_targeting_types_targeting_options_list(
        &self,
        args: &DisplayvideoTargetingTypesTargetingOptionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListTargetingOptionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_targeting_types_targeting_options_list_builder(
            &self.http_client,
            &args.targetingType,
            &args.advertiserId,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_targeting_types_targeting_options_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo targeting types targeting options search.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Displayvideo users get.
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
    pub fn displayvideo_users_get(
        &self,
        args: &DisplayvideoUsersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<User, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_users_get_builder(
            &self.http_client,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_users_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Displayvideo users list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListUsersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn displayvideo_users_list(
        &self,
        args: &DisplayvideoUsersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListUsersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = displayvideo_users_list_builder(
            &self.http_client,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = displayvideo_users_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
