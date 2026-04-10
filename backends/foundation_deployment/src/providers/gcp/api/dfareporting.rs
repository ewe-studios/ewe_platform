//! DfareportingProvider - State-aware dfareporting API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       dfareporting API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::dfareporting::{
    dfareporting_account_active_ad_summaries_get_builder, dfareporting_account_active_ad_summaries_get_task,
    dfareporting_account_permission_groups_get_builder, dfareporting_account_permission_groups_get_task,
    dfareporting_account_permission_groups_list_builder, dfareporting_account_permission_groups_list_task,
    dfareporting_account_permissions_get_builder, dfareporting_account_permissions_get_task,
    dfareporting_account_permissions_list_builder, dfareporting_account_permissions_list_task,
    dfareporting_account_user_profiles_get_builder, dfareporting_account_user_profiles_get_task,
    dfareporting_account_user_profiles_insert_builder, dfareporting_account_user_profiles_insert_task,
    dfareporting_account_user_profiles_list_builder, dfareporting_account_user_profiles_list_task,
    dfareporting_account_user_profiles_patch_builder, dfareporting_account_user_profiles_patch_task,
    dfareporting_account_user_profiles_update_builder, dfareporting_account_user_profiles_update_task,
    dfareporting_accounts_get_builder, dfareporting_accounts_get_task,
    dfareporting_accounts_list_builder, dfareporting_accounts_list_task,
    dfareporting_accounts_patch_builder, dfareporting_accounts_patch_task,
    dfareporting_accounts_update_builder, dfareporting_accounts_update_task,
    dfareporting_ads_get_builder, dfareporting_ads_get_task,
    dfareporting_ads_insert_builder, dfareporting_ads_insert_task,
    dfareporting_ads_list_builder, dfareporting_ads_list_task,
    dfareporting_ads_patch_builder, dfareporting_ads_patch_task,
    dfareporting_ads_update_builder, dfareporting_ads_update_task,
    dfareporting_advertiser_groups_delete_builder, dfareporting_advertiser_groups_delete_task,
    dfareporting_advertiser_groups_get_builder, dfareporting_advertiser_groups_get_task,
    dfareporting_advertiser_groups_insert_builder, dfareporting_advertiser_groups_insert_task,
    dfareporting_advertiser_groups_list_builder, dfareporting_advertiser_groups_list_task,
    dfareporting_advertiser_groups_patch_builder, dfareporting_advertiser_groups_patch_task,
    dfareporting_advertiser_groups_update_builder, dfareporting_advertiser_groups_update_task,
    dfareporting_advertiser_invoices_list_builder, dfareporting_advertiser_invoices_list_task,
    dfareporting_advertiser_landing_pages_get_builder, dfareporting_advertiser_landing_pages_get_task,
    dfareporting_advertiser_landing_pages_insert_builder, dfareporting_advertiser_landing_pages_insert_task,
    dfareporting_advertiser_landing_pages_list_builder, dfareporting_advertiser_landing_pages_list_task,
    dfareporting_advertiser_landing_pages_patch_builder, dfareporting_advertiser_landing_pages_patch_task,
    dfareporting_advertiser_landing_pages_update_builder, dfareporting_advertiser_landing_pages_update_task,
    dfareporting_advertisers_get_builder, dfareporting_advertisers_get_task,
    dfareporting_advertisers_insert_builder, dfareporting_advertisers_insert_task,
    dfareporting_advertisers_list_builder, dfareporting_advertisers_list_task,
    dfareporting_advertisers_patch_builder, dfareporting_advertisers_patch_task,
    dfareporting_advertisers_update_builder, dfareporting_advertisers_update_task,
    dfareporting_billing_assignments_insert_builder, dfareporting_billing_assignments_insert_task,
    dfareporting_billing_assignments_list_builder, dfareporting_billing_assignments_list_task,
    dfareporting_billing_profiles_get_builder, dfareporting_billing_profiles_get_task,
    dfareporting_billing_profiles_list_builder, dfareporting_billing_profiles_list_task,
    dfareporting_billing_profiles_update_builder, dfareporting_billing_profiles_update_task,
    dfareporting_billing_rates_list_builder, dfareporting_billing_rates_list_task,
    dfareporting_browsers_list_builder, dfareporting_browsers_list_task,
    dfareporting_campaign_creative_associations_insert_builder, dfareporting_campaign_creative_associations_insert_task,
    dfareporting_campaign_creative_associations_list_builder, dfareporting_campaign_creative_associations_list_task,
    dfareporting_campaigns_get_builder, dfareporting_campaigns_get_task,
    dfareporting_campaigns_insert_builder, dfareporting_campaigns_insert_task,
    dfareporting_campaigns_list_builder, dfareporting_campaigns_list_task,
    dfareporting_campaigns_patch_builder, dfareporting_campaigns_patch_task,
    dfareporting_campaigns_update_builder, dfareporting_campaigns_update_task,
    dfareporting_change_logs_get_builder, dfareporting_change_logs_get_task,
    dfareporting_change_logs_list_builder, dfareporting_change_logs_list_task,
    dfareporting_cities_list_builder, dfareporting_cities_list_task,
    dfareporting_connection_types_get_builder, dfareporting_connection_types_get_task,
    dfareporting_connection_types_list_builder, dfareporting_connection_types_list_task,
    dfareporting_content_categories_delete_builder, dfareporting_content_categories_delete_task,
    dfareporting_content_categories_get_builder, dfareporting_content_categories_get_task,
    dfareporting_content_categories_insert_builder, dfareporting_content_categories_insert_task,
    dfareporting_content_categories_list_builder, dfareporting_content_categories_list_task,
    dfareporting_content_categories_patch_builder, dfareporting_content_categories_patch_task,
    dfareporting_content_categories_update_builder, dfareporting_content_categories_update_task,
    dfareporting_conversions_batchinsert_builder, dfareporting_conversions_batchinsert_task,
    dfareporting_conversions_batchupdate_builder, dfareporting_conversions_batchupdate_task,
    dfareporting_countries_get_builder, dfareporting_countries_get_task,
    dfareporting_countries_list_builder, dfareporting_countries_list_task,
    dfareporting_creative_assets_insert_builder, dfareporting_creative_assets_insert_task,
    dfareporting_creative_field_values_delete_builder, dfareporting_creative_field_values_delete_task,
    dfareporting_creative_field_values_get_builder, dfareporting_creative_field_values_get_task,
    dfareporting_creative_field_values_insert_builder, dfareporting_creative_field_values_insert_task,
    dfareporting_creative_field_values_list_builder, dfareporting_creative_field_values_list_task,
    dfareporting_creative_field_values_patch_builder, dfareporting_creative_field_values_patch_task,
    dfareporting_creative_field_values_update_builder, dfareporting_creative_field_values_update_task,
    dfareporting_creative_fields_delete_builder, dfareporting_creative_fields_delete_task,
    dfareporting_creative_fields_get_builder, dfareporting_creative_fields_get_task,
    dfareporting_creative_fields_insert_builder, dfareporting_creative_fields_insert_task,
    dfareporting_creative_fields_list_builder, dfareporting_creative_fields_list_task,
    dfareporting_creative_fields_patch_builder, dfareporting_creative_fields_patch_task,
    dfareporting_creative_fields_update_builder, dfareporting_creative_fields_update_task,
    dfareporting_creative_groups_get_builder, dfareporting_creative_groups_get_task,
    dfareporting_creative_groups_insert_builder, dfareporting_creative_groups_insert_task,
    dfareporting_creative_groups_list_builder, dfareporting_creative_groups_list_task,
    dfareporting_creative_groups_patch_builder, dfareporting_creative_groups_patch_task,
    dfareporting_creative_groups_update_builder, dfareporting_creative_groups_update_task,
    dfareporting_creatives_get_builder, dfareporting_creatives_get_task,
    dfareporting_creatives_insert_builder, dfareporting_creatives_insert_task,
    dfareporting_creatives_list_builder, dfareporting_creatives_list_task,
    dfareporting_creatives_patch_builder, dfareporting_creatives_patch_task,
    dfareporting_creatives_update_builder, dfareporting_creatives_update_task,
    dfareporting_dimension_values_query_builder, dfareporting_dimension_values_query_task,
    dfareporting_directory_sites_get_builder, dfareporting_directory_sites_get_task,
    dfareporting_directory_sites_insert_builder, dfareporting_directory_sites_insert_task,
    dfareporting_directory_sites_list_builder, dfareporting_directory_sites_list_task,
    dfareporting_dynamic_feeds_get_builder, dfareporting_dynamic_feeds_get_task,
    dfareporting_dynamic_feeds_insert_builder, dfareporting_dynamic_feeds_insert_task,
    dfareporting_dynamic_feeds_retransform_builder, dfareporting_dynamic_feeds_retransform_task,
    dfareporting_dynamic_feeds_update_builder, dfareporting_dynamic_feeds_update_task,
    dfareporting_dynamic_profiles_generate_code_builder, dfareporting_dynamic_profiles_generate_code_task,
    dfareporting_dynamic_profiles_get_builder, dfareporting_dynamic_profiles_get_task,
    dfareporting_dynamic_profiles_insert_builder, dfareporting_dynamic_profiles_insert_task,
    dfareporting_dynamic_profiles_publish_builder, dfareporting_dynamic_profiles_publish_task,
    dfareporting_dynamic_profiles_update_builder, dfareporting_dynamic_profiles_update_task,
    dfareporting_dynamic_targeting_keys_delete_builder, dfareporting_dynamic_targeting_keys_delete_task,
    dfareporting_dynamic_targeting_keys_insert_builder, dfareporting_dynamic_targeting_keys_insert_task,
    dfareporting_dynamic_targeting_keys_list_builder, dfareporting_dynamic_targeting_keys_list_task,
    dfareporting_event_tags_delete_builder, dfareporting_event_tags_delete_task,
    dfareporting_event_tags_get_builder, dfareporting_event_tags_get_task,
    dfareporting_event_tags_insert_builder, dfareporting_event_tags_insert_task,
    dfareporting_event_tags_list_builder, dfareporting_event_tags_list_task,
    dfareporting_event_tags_patch_builder, dfareporting_event_tags_patch_task,
    dfareporting_event_tags_update_builder, dfareporting_event_tags_update_task,
    dfareporting_files_get_builder, dfareporting_files_get_task,
    dfareporting_files_list_builder, dfareporting_files_list_task,
    dfareporting_floodlight_activities_delete_builder, dfareporting_floodlight_activities_delete_task,
    dfareporting_floodlight_activities_generatetag_builder, dfareporting_floodlight_activities_generatetag_task,
    dfareporting_floodlight_activities_get_builder, dfareporting_floodlight_activities_get_task,
    dfareporting_floodlight_activities_insert_builder, dfareporting_floodlight_activities_insert_task,
    dfareporting_floodlight_activities_list_builder, dfareporting_floodlight_activities_list_task,
    dfareporting_floodlight_activities_patch_builder, dfareporting_floodlight_activities_patch_task,
    dfareporting_floodlight_activities_update_builder, dfareporting_floodlight_activities_update_task,
    dfareporting_floodlight_activity_groups_get_builder, dfareporting_floodlight_activity_groups_get_task,
    dfareporting_floodlight_activity_groups_insert_builder, dfareporting_floodlight_activity_groups_insert_task,
    dfareporting_floodlight_activity_groups_list_builder, dfareporting_floodlight_activity_groups_list_task,
    dfareporting_floodlight_activity_groups_patch_builder, dfareporting_floodlight_activity_groups_patch_task,
    dfareporting_floodlight_activity_groups_update_builder, dfareporting_floodlight_activity_groups_update_task,
    dfareporting_floodlight_configurations_get_builder, dfareporting_floodlight_configurations_get_task,
    dfareporting_floodlight_configurations_list_builder, dfareporting_floodlight_configurations_list_task,
    dfareporting_floodlight_configurations_patch_builder, dfareporting_floodlight_configurations_patch_task,
    dfareporting_floodlight_configurations_update_builder, dfareporting_floodlight_configurations_update_task,
    dfareporting_languages_list_builder, dfareporting_languages_list_task,
    dfareporting_metros_list_builder, dfareporting_metros_list_task,
    dfareporting_mobile_apps_get_builder, dfareporting_mobile_apps_get_task,
    dfareporting_mobile_apps_list_builder, dfareporting_mobile_apps_list_task,
    dfareporting_mobile_carriers_get_builder, dfareporting_mobile_carriers_get_task,
    dfareporting_mobile_carriers_list_builder, dfareporting_mobile_carriers_list_task,
    dfareporting_operating_system_versions_get_builder, dfareporting_operating_system_versions_get_task,
    dfareporting_operating_system_versions_list_builder, dfareporting_operating_system_versions_list_task,
    dfareporting_operating_systems_get_builder, dfareporting_operating_systems_get_task,
    dfareporting_operating_systems_list_builder, dfareporting_operating_systems_list_task,
    dfareporting_placement_groups_get_builder, dfareporting_placement_groups_get_task,
    dfareporting_placement_groups_insert_builder, dfareporting_placement_groups_insert_task,
    dfareporting_placement_groups_list_builder, dfareporting_placement_groups_list_task,
    dfareporting_placement_groups_patch_builder, dfareporting_placement_groups_patch_task,
    dfareporting_placement_groups_update_builder, dfareporting_placement_groups_update_task,
    dfareporting_placement_strategies_delete_builder, dfareporting_placement_strategies_delete_task,
    dfareporting_placement_strategies_get_builder, dfareporting_placement_strategies_get_task,
    dfareporting_placement_strategies_insert_builder, dfareporting_placement_strategies_insert_task,
    dfareporting_placement_strategies_list_builder, dfareporting_placement_strategies_list_task,
    dfareporting_placement_strategies_patch_builder, dfareporting_placement_strategies_patch_task,
    dfareporting_placement_strategies_update_builder, dfareporting_placement_strategies_update_task,
    dfareporting_placements_generatetags_builder, dfareporting_placements_generatetags_task,
    dfareporting_placements_get_builder, dfareporting_placements_get_task,
    dfareporting_placements_insert_builder, dfareporting_placements_insert_task,
    dfareporting_placements_list_builder, dfareporting_placements_list_task,
    dfareporting_placements_patch_builder, dfareporting_placements_patch_task,
    dfareporting_placements_update_builder, dfareporting_placements_update_task,
    dfareporting_platform_types_get_builder, dfareporting_platform_types_get_task,
    dfareporting_platform_types_list_builder, dfareporting_platform_types_list_task,
    dfareporting_postal_codes_get_builder, dfareporting_postal_codes_get_task,
    dfareporting_postal_codes_list_builder, dfareporting_postal_codes_list_task,
    dfareporting_regions_list_builder, dfareporting_regions_list_task,
    dfareporting_remarketing_list_shares_get_builder, dfareporting_remarketing_list_shares_get_task,
    dfareporting_remarketing_list_shares_patch_builder, dfareporting_remarketing_list_shares_patch_task,
    dfareporting_remarketing_list_shares_update_builder, dfareporting_remarketing_list_shares_update_task,
    dfareporting_remarketing_lists_get_builder, dfareporting_remarketing_lists_get_task,
    dfareporting_remarketing_lists_insert_builder, dfareporting_remarketing_lists_insert_task,
    dfareporting_remarketing_lists_list_builder, dfareporting_remarketing_lists_list_task,
    dfareporting_remarketing_lists_patch_builder, dfareporting_remarketing_lists_patch_task,
    dfareporting_remarketing_lists_update_builder, dfareporting_remarketing_lists_update_task,
    dfareporting_reports_delete_builder, dfareporting_reports_delete_task,
    dfareporting_reports_get_builder, dfareporting_reports_get_task,
    dfareporting_reports_insert_builder, dfareporting_reports_insert_task,
    dfareporting_reports_list_builder, dfareporting_reports_list_task,
    dfareporting_reports_run_builder, dfareporting_reports_run_task,
    dfareporting_reports_update_builder, dfareporting_reports_update_task,
    dfareporting_reports_compatible_fields_query_builder, dfareporting_reports_compatible_fields_query_task,
    dfareporting_reports_files_get_builder, dfareporting_reports_files_get_task,
    dfareporting_reports_files_list_builder, dfareporting_reports_files_list_task,
    dfareporting_sites_get_builder, dfareporting_sites_get_task,
    dfareporting_sites_insert_builder, dfareporting_sites_insert_task,
    dfareporting_sites_list_builder, dfareporting_sites_list_task,
    dfareporting_sites_patch_builder, dfareporting_sites_patch_task,
    dfareporting_sites_update_builder, dfareporting_sites_update_task,
    dfareporting_sizes_get_builder, dfareporting_sizes_get_task,
    dfareporting_sizes_insert_builder, dfareporting_sizes_insert_task,
    dfareporting_sizes_list_builder, dfareporting_sizes_list_task,
    dfareporting_studio_creative_assets_insert_builder, dfareporting_studio_creative_assets_insert_task,
    dfareporting_studio_creatives_get_builder, dfareporting_studio_creatives_get_task,
    dfareporting_studio_creatives_insert_builder, dfareporting_studio_creatives_insert_task,
    dfareporting_studio_creatives_publish_builder, dfareporting_studio_creatives_publish_task,
    dfareporting_subaccounts_get_builder, dfareporting_subaccounts_get_task,
    dfareporting_subaccounts_insert_builder, dfareporting_subaccounts_insert_task,
    dfareporting_subaccounts_list_builder, dfareporting_subaccounts_list_task,
    dfareporting_subaccounts_patch_builder, dfareporting_subaccounts_patch_task,
    dfareporting_subaccounts_update_builder, dfareporting_subaccounts_update_task,
    dfareporting_targetable_remarketing_lists_get_builder, dfareporting_targetable_remarketing_lists_get_task,
    dfareporting_targetable_remarketing_lists_list_builder, dfareporting_targetable_remarketing_lists_list_task,
    dfareporting_targeting_templates_get_builder, dfareporting_targeting_templates_get_task,
    dfareporting_targeting_templates_insert_builder, dfareporting_targeting_templates_insert_task,
    dfareporting_targeting_templates_list_builder, dfareporting_targeting_templates_list_task,
    dfareporting_targeting_templates_patch_builder, dfareporting_targeting_templates_patch_task,
    dfareporting_targeting_templates_update_builder, dfareporting_targeting_templates_update_task,
    dfareporting_tv_campaign_details_get_builder, dfareporting_tv_campaign_details_get_task,
    dfareporting_tv_campaign_summaries_list_builder, dfareporting_tv_campaign_summaries_list_task,
    dfareporting_user_profiles_get_builder, dfareporting_user_profiles_get_task,
    dfareporting_user_profiles_list_builder, dfareporting_user_profiles_list_task,
    dfareporting_user_role_permission_groups_get_builder, dfareporting_user_role_permission_groups_get_task,
    dfareporting_user_role_permission_groups_list_builder, dfareporting_user_role_permission_groups_list_task,
    dfareporting_user_role_permissions_get_builder, dfareporting_user_role_permissions_get_task,
    dfareporting_user_role_permissions_list_builder, dfareporting_user_role_permissions_list_task,
    dfareporting_user_roles_delete_builder, dfareporting_user_roles_delete_task,
    dfareporting_user_roles_get_builder, dfareporting_user_roles_get_task,
    dfareporting_user_roles_insert_builder, dfareporting_user_roles_insert_task,
    dfareporting_user_roles_list_builder, dfareporting_user_roles_list_task,
    dfareporting_user_roles_patch_builder, dfareporting_user_roles_patch_task,
    dfareporting_user_roles_update_builder, dfareporting_user_roles_update_task,
    dfareporting_video_formats_get_builder, dfareporting_video_formats_get_task,
    dfareporting_video_formats_list_builder, dfareporting_video_formats_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::dfareporting::Account;
use crate::providers::gcp::clients::dfareporting::AccountActiveAdSummary;
use crate::providers::gcp::clients::dfareporting::AccountPermission;
use crate::providers::gcp::clients::dfareporting::AccountPermissionGroup;
use crate::providers::gcp::clients::dfareporting::AccountPermissionGroupsListResponse;
use crate::providers::gcp::clients::dfareporting::AccountPermissionsListResponse;
use crate::providers::gcp::clients::dfareporting::AccountUserProfile;
use crate::providers::gcp::clients::dfareporting::AccountUserProfilesListResponse;
use crate::providers::gcp::clients::dfareporting::AccountsListResponse;
use crate::providers::gcp::clients::dfareporting::Ad;
use crate::providers::gcp::clients::dfareporting::AdsListResponse;
use crate::providers::gcp::clients::dfareporting::Advertiser;
use crate::providers::gcp::clients::dfareporting::AdvertiserGroup;
use crate::providers::gcp::clients::dfareporting::AdvertiserGroupsListResponse;
use crate::providers::gcp::clients::dfareporting::AdvertiserInvoicesListResponse;
use crate::providers::gcp::clients::dfareporting::AdvertiserLandingPagesListResponse;
use crate::providers::gcp::clients::dfareporting::AdvertisersListResponse;
use crate::providers::gcp::clients::dfareporting::BillingAssignment;
use crate::providers::gcp::clients::dfareporting::BillingAssignmentsListResponse;
use crate::providers::gcp::clients::dfareporting::BillingProfile;
use crate::providers::gcp::clients::dfareporting::BillingProfilesListResponse;
use crate::providers::gcp::clients::dfareporting::BillingRatesListResponse;
use crate::providers::gcp::clients::dfareporting::BrowsersListResponse;
use crate::providers::gcp::clients::dfareporting::Campaign;
use crate::providers::gcp::clients::dfareporting::CampaignCreativeAssociation;
use crate::providers::gcp::clients::dfareporting::CampaignCreativeAssociationsListResponse;
use crate::providers::gcp::clients::dfareporting::CampaignsListResponse;
use crate::providers::gcp::clients::dfareporting::ChangeLog;
use crate::providers::gcp::clients::dfareporting::ChangeLogsListResponse;
use crate::providers::gcp::clients::dfareporting::CitiesListResponse;
use crate::providers::gcp::clients::dfareporting::CompatibleFields;
use crate::providers::gcp::clients::dfareporting::ConnectionType;
use crate::providers::gcp::clients::dfareporting::ConnectionTypesListResponse;
use crate::providers::gcp::clients::dfareporting::ContentCategoriesListResponse;
use crate::providers::gcp::clients::dfareporting::ContentCategory;
use crate::providers::gcp::clients::dfareporting::ConversionsBatchInsertResponse;
use crate::providers::gcp::clients::dfareporting::ConversionsBatchUpdateResponse;
use crate::providers::gcp::clients::dfareporting::CountriesListResponse;
use crate::providers::gcp::clients::dfareporting::Country;
use crate::providers::gcp::clients::dfareporting::Creative;
use crate::providers::gcp::clients::dfareporting::CreativeAssetMetadata;
use crate::providers::gcp::clients::dfareporting::CreativeField;
use crate::providers::gcp::clients::dfareporting::CreativeFieldValue;
use crate::providers::gcp::clients::dfareporting::CreativeFieldValuesListResponse;
use crate::providers::gcp::clients::dfareporting::CreativeFieldsListResponse;
use crate::providers::gcp::clients::dfareporting::CreativeGroup;
use crate::providers::gcp::clients::dfareporting::CreativeGroupsListResponse;
use crate::providers::gcp::clients::dfareporting::CreativesListResponse;
use crate::providers::gcp::clients::dfareporting::DimensionValueList;
use crate::providers::gcp::clients::dfareporting::DirectorySite;
use crate::providers::gcp::clients::dfareporting::DirectorySitesListResponse;
use crate::providers::gcp::clients::dfareporting::DynamicFeed;
use crate::providers::gcp::clients::dfareporting::DynamicProfile;
use crate::providers::gcp::clients::dfareporting::DynamicProfileGenerateCodeResponse;
use crate::providers::gcp::clients::dfareporting::DynamicTargetingKey;
use crate::providers::gcp::clients::dfareporting::DynamicTargetingKeysListResponse;
use crate::providers::gcp::clients::dfareporting::EventTag;
use crate::providers::gcp::clients::dfareporting::EventTagsListResponse;
use crate::providers::gcp::clients::dfareporting::File;
use crate::providers::gcp::clients::dfareporting::FileList;
use crate::providers::gcp::clients::dfareporting::FloodlightActivitiesGenerateTagResponse;
use crate::providers::gcp::clients::dfareporting::FloodlightActivitiesListResponse;
use crate::providers::gcp::clients::dfareporting::FloodlightActivity;
use crate::providers::gcp::clients::dfareporting::FloodlightActivityGroup;
use crate::providers::gcp::clients::dfareporting::FloodlightActivityGroupsListResponse;
use crate::providers::gcp::clients::dfareporting::FloodlightConfiguration;
use crate::providers::gcp::clients::dfareporting::FloodlightConfigurationsListResponse;
use crate::providers::gcp::clients::dfareporting::LandingPage;
use crate::providers::gcp::clients::dfareporting::LanguagesListResponse;
use crate::providers::gcp::clients::dfareporting::MetrosListResponse;
use crate::providers::gcp::clients::dfareporting::MobileApp;
use crate::providers::gcp::clients::dfareporting::MobileAppsListResponse;
use crate::providers::gcp::clients::dfareporting::MobileCarrier;
use crate::providers::gcp::clients::dfareporting::MobileCarriersListResponse;
use crate::providers::gcp::clients::dfareporting::OperatingSystem;
use crate::providers::gcp::clients::dfareporting::OperatingSystemVersion;
use crate::providers::gcp::clients::dfareporting::OperatingSystemVersionsListResponse;
use crate::providers::gcp::clients::dfareporting::OperatingSystemsListResponse;
use crate::providers::gcp::clients::dfareporting::Placement;
use crate::providers::gcp::clients::dfareporting::PlacementGroup;
use crate::providers::gcp::clients::dfareporting::PlacementGroupsListResponse;
use crate::providers::gcp::clients::dfareporting::PlacementStrategiesListResponse;
use crate::providers::gcp::clients::dfareporting::PlacementStrategy;
use crate::providers::gcp::clients::dfareporting::PlacementsGenerateTagsResponse;
use crate::providers::gcp::clients::dfareporting::PlacementsListResponse;
use crate::providers::gcp::clients::dfareporting::PlatformType;
use crate::providers::gcp::clients::dfareporting::PlatformTypesListResponse;
use crate::providers::gcp::clients::dfareporting::PostalCode;
use crate::providers::gcp::clients::dfareporting::PostalCodesListResponse;
use crate::providers::gcp::clients::dfareporting::RegionsListResponse;
use crate::providers::gcp::clients::dfareporting::RemarketingList;
use crate::providers::gcp::clients::dfareporting::RemarketingListShare;
use crate::providers::gcp::clients::dfareporting::RemarketingListsListResponse;
use crate::providers::gcp::clients::dfareporting::Report;
use crate::providers::gcp::clients::dfareporting::ReportList;
use crate::providers::gcp::clients::dfareporting::Site;
use crate::providers::gcp::clients::dfareporting::SitesListResponse;
use crate::providers::gcp::clients::dfareporting::Size;
use crate::providers::gcp::clients::dfareporting::SizesListResponse;
use crate::providers::gcp::clients::dfareporting::StudioCreative;
use crate::providers::gcp::clients::dfareporting::StudioCreativeAssetsResponse;
use crate::providers::gcp::clients::dfareporting::Subaccount;
use crate::providers::gcp::clients::dfareporting::SubaccountsListResponse;
use crate::providers::gcp::clients::dfareporting::TargetableRemarketingList;
use crate::providers::gcp::clients::dfareporting::TargetableRemarketingListsListResponse;
use crate::providers::gcp::clients::dfareporting::TargetingTemplate;
use crate::providers::gcp::clients::dfareporting::TargetingTemplatesListResponse;
use crate::providers::gcp::clients::dfareporting::TvCampaignDetail;
use crate::providers::gcp::clients::dfareporting::TvCampaignSummariesListResponse;
use crate::providers::gcp::clients::dfareporting::UserProfile;
use crate::providers::gcp::clients::dfareporting::UserProfileList;
use crate::providers::gcp::clients::dfareporting::UserRole;
use crate::providers::gcp::clients::dfareporting::UserRolePermission;
use crate::providers::gcp::clients::dfareporting::UserRolePermissionGroup;
use crate::providers::gcp::clients::dfareporting::UserRolePermissionGroupsListResponse;
use crate::providers::gcp::clients::dfareporting::UserRolePermissionsListResponse;
use crate::providers::gcp::clients::dfareporting::UserRolesListResponse;
use crate::providers::gcp::clients::dfareporting::VideoFormat;
use crate::providers::gcp::clients::dfareporting::VideoFormatsListResponse;
use crate::providers::gcp::clients::dfareporting::DfareportingAccountActiveAdSummariesGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAccountPermissionGroupsGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAccountPermissionGroupsListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAccountPermissionsGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAccountPermissionsListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAccountUserProfilesGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAccountUserProfilesInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAccountUserProfilesListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAccountUserProfilesPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAccountUserProfilesUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAccountsGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAccountsListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAccountsPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAccountsUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAdsGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAdsInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAdsListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAdsPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAdsUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAdvertiserGroupsDeleteArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAdvertiserGroupsGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAdvertiserGroupsInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAdvertiserGroupsListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAdvertiserGroupsPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAdvertiserGroupsUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAdvertiserInvoicesListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAdvertiserLandingPagesGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAdvertiserLandingPagesInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAdvertiserLandingPagesListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAdvertiserLandingPagesPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAdvertiserLandingPagesUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAdvertisersGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAdvertisersInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAdvertisersListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAdvertisersPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAdvertisersUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingBillingAssignmentsInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingBillingAssignmentsListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingBillingProfilesGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingBillingProfilesListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingBillingProfilesUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingBillingRatesListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingBrowsersListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCampaignCreativeAssociationsInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCampaignCreativeAssociationsListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCampaignsGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCampaignsInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCampaignsListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCampaignsPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCampaignsUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingChangeLogsGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingChangeLogsListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCitiesListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingConnectionTypesGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingConnectionTypesListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingContentCategoriesDeleteArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingContentCategoriesGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingContentCategoriesInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingContentCategoriesListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingContentCategoriesPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingContentCategoriesUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingConversionsBatchinsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingConversionsBatchupdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCountriesGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCountriesListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCreativeAssetsInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCreativeFieldValuesDeleteArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCreativeFieldValuesGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCreativeFieldValuesInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCreativeFieldValuesListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCreativeFieldValuesPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCreativeFieldValuesUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCreativeFieldsDeleteArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCreativeFieldsGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCreativeFieldsInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCreativeFieldsListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCreativeFieldsPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCreativeFieldsUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCreativeGroupsGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCreativeGroupsInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCreativeGroupsListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCreativeGroupsPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCreativeGroupsUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCreativesGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCreativesInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCreativesListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCreativesPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCreativesUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingDimensionValuesQueryArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingDirectorySitesGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingDirectorySitesInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingDirectorySitesListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingDynamicFeedsGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingDynamicFeedsInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingDynamicFeedsRetransformArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingDynamicFeedsUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingDynamicProfilesGenerateCodeArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingDynamicProfilesGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingDynamicProfilesInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingDynamicProfilesPublishArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingDynamicProfilesUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingDynamicTargetingKeysDeleteArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingDynamicTargetingKeysInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingDynamicTargetingKeysListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingEventTagsDeleteArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingEventTagsGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingEventTagsInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingEventTagsListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingEventTagsPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingEventTagsUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingFilesGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingFilesListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingFloodlightActivitiesDeleteArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingFloodlightActivitiesGeneratetagArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingFloodlightActivitiesGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingFloodlightActivitiesInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingFloodlightActivitiesListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingFloodlightActivitiesPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingFloodlightActivitiesUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingFloodlightActivityGroupsGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingFloodlightActivityGroupsInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingFloodlightActivityGroupsListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingFloodlightActivityGroupsPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingFloodlightActivityGroupsUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingFloodlightConfigurationsGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingFloodlightConfigurationsListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingFloodlightConfigurationsPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingFloodlightConfigurationsUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingLanguagesListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingMetrosListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingMobileAppsGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingMobileAppsListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingMobileCarriersGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingMobileCarriersListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingOperatingSystemVersionsGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingOperatingSystemVersionsListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingOperatingSystemsGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingOperatingSystemsListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingPlacementGroupsGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingPlacementGroupsInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingPlacementGroupsListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingPlacementGroupsPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingPlacementGroupsUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingPlacementStrategiesDeleteArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingPlacementStrategiesGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingPlacementStrategiesInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingPlacementStrategiesListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingPlacementStrategiesPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingPlacementStrategiesUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingPlacementsGeneratetagsArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingPlacementsGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingPlacementsInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingPlacementsListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingPlacementsPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingPlacementsUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingPlatformTypesGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingPlatformTypesListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingPostalCodesGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingPostalCodesListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingRegionsListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingRemarketingListSharesGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingRemarketingListSharesPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingRemarketingListSharesUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingRemarketingListsGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingRemarketingListsInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingRemarketingListsListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingRemarketingListsPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingRemarketingListsUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingReportsCompatibleFieldsQueryArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingReportsDeleteArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingReportsFilesGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingReportsFilesListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingReportsGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingReportsInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingReportsListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingReportsRunArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingReportsUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingSitesGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingSitesInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingSitesListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingSitesPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingSitesUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingSizesGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingSizesInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingSizesListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingStudioCreativeAssetsInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingStudioCreativesGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingStudioCreativesInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingStudioCreativesPublishArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingSubaccountsGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingSubaccountsInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingSubaccountsListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingSubaccountsPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingSubaccountsUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingTargetableRemarketingListsGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingTargetableRemarketingListsListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingTargetingTemplatesGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingTargetingTemplatesInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingTargetingTemplatesListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingTargetingTemplatesPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingTargetingTemplatesUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingTvCampaignDetailsGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingTvCampaignSummariesListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingUserProfilesGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingUserProfilesListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingUserRolePermissionGroupsGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingUserRolePermissionGroupsListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingUserRolePermissionsGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingUserRolePermissionsListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingUserRolesDeleteArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingUserRolesGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingUserRolesInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingUserRolesListArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingUserRolesPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingUserRolesUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingVideoFormatsGetArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingVideoFormatsListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// DfareportingProvider with automatic state tracking.
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
/// let provider = DfareportingProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct DfareportingProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> DfareportingProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new DfareportingProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Dfareporting account active ad summaries get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccountActiveAdSummary result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_account_active_ad_summaries_get(
        &self,
        args: &DfareportingAccountActiveAdSummariesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccountActiveAdSummary, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_account_active_ad_summaries_get_builder(
            &self.http_client,
            &args.profileId,
            &args.summaryAccountId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_account_active_ad_summaries_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting account permission groups get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccountPermissionGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_account_permission_groups_get(
        &self,
        args: &DfareportingAccountPermissionGroupsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccountPermissionGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_account_permission_groups_get_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_account_permission_groups_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting account permission groups list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccountPermissionGroupsListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_account_permission_groups_list(
        &self,
        args: &DfareportingAccountPermissionGroupsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccountPermissionGroupsListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_account_permission_groups_list_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_account_permission_groups_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting account permissions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccountPermission result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_account_permissions_get(
        &self,
        args: &DfareportingAccountPermissionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccountPermission, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_account_permissions_get_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_account_permissions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting account permissions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccountPermissionsListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_account_permissions_list(
        &self,
        args: &DfareportingAccountPermissionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccountPermissionsListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_account_permissions_list_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_account_permissions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting account user profiles get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccountUserProfile result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_account_user_profiles_get(
        &self,
        args: &DfareportingAccountUserProfilesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccountUserProfile, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_account_user_profiles_get_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_account_user_profiles_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting account user profiles insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccountUserProfile result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_account_user_profiles_insert(
        &self,
        args: &DfareportingAccountUserProfilesInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccountUserProfile, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_account_user_profiles_insert_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_account_user_profiles_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting account user profiles list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccountUserProfilesListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_account_user_profiles_list(
        &self,
        args: &DfareportingAccountUserProfilesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccountUserProfilesListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_account_user_profiles_list_builder(
            &self.http_client,
            &args.profileId,
            &args.active,
            &args.ids,
            &args.maxResults,
            &args.pageToken,
            &args.searchString,
            &args.sortField,
            &args.sortOrder,
            &args.subaccountId,
            &args.userRoleId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_account_user_profiles_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting account user profiles patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccountUserProfile result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_account_user_profiles_patch(
        &self,
        args: &DfareportingAccountUserProfilesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccountUserProfile, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_account_user_profiles_patch_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_account_user_profiles_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting account user profiles update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccountUserProfile result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_account_user_profiles_update(
        &self,
        args: &DfareportingAccountUserProfilesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccountUserProfile, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_account_user_profiles_update_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_account_user_profiles_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting accounts get.
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
    pub fn dfareporting_accounts_get(
        &self,
        args: &DfareportingAccountsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Account, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_accounts_get_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_accounts_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting accounts list.
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
    pub fn dfareporting_accounts_list(
        &self,
        args: &DfareportingAccountsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccountsListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_accounts_list_builder(
            &self.http_client,
            &args.profileId,
            &args.active,
            &args.ids,
            &args.maxResults,
            &args.pageToken,
            &args.searchString,
            &args.sortField,
            &args.sortOrder,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_accounts_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting accounts patch.
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
    pub fn dfareporting_accounts_patch(
        &self,
        args: &DfareportingAccountsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Account, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_accounts_patch_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_accounts_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting accounts update.
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
    pub fn dfareporting_accounts_update(
        &self,
        args: &DfareportingAccountsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Account, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_accounts_update_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_accounts_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting ads get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Ad result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_ads_get(
        &self,
        args: &DfareportingAdsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Ad, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_ads_get_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_ads_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting ads insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Ad result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_ads_insert(
        &self,
        args: &DfareportingAdsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Ad, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_ads_insert_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_ads_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting ads list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AdsListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_ads_list(
        &self,
        args: &DfareportingAdsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AdsListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_ads_list_builder(
            &self.http_client,
            &args.profileId,
            &args.active,
            &args.advertiserId,
            &args.archived,
            &args.audienceSegmentIds,
            &args.campaignIds,
            &args.compatibility,
            &args.creativeIds,
            &args.creativeOptimizationConfigurationIds,
            &args.dynamicClickTracker,
            &args.ids,
            &args.landingPageIds,
            &args.maxResults,
            &args.overriddenEventTagId,
            &args.pageToken,
            &args.placementIds,
            &args.remarketingListIds,
            &args.searchString,
            &args.sizeIds,
            &args.sortField,
            &args.sortOrder,
            &args.sslCompliant,
            &args.sslRequired,
            &args.type_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_ads_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting ads patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Ad result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_ads_patch(
        &self,
        args: &DfareportingAdsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Ad, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_ads_patch_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_ads_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting ads update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Ad result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_ads_update(
        &self,
        args: &DfareportingAdsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Ad, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_ads_update_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_ads_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting advertiser groups delete.
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
    pub fn dfareporting_advertiser_groups_delete(
        &self,
        args: &DfareportingAdvertiserGroupsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_advertiser_groups_delete_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_advertiser_groups_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting advertiser groups get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AdvertiserGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_advertiser_groups_get(
        &self,
        args: &DfareportingAdvertiserGroupsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AdvertiserGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_advertiser_groups_get_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_advertiser_groups_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting advertiser groups insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AdvertiserGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_advertiser_groups_insert(
        &self,
        args: &DfareportingAdvertiserGroupsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AdvertiserGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_advertiser_groups_insert_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_advertiser_groups_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting advertiser groups list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AdvertiserGroupsListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_advertiser_groups_list(
        &self,
        args: &DfareportingAdvertiserGroupsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AdvertiserGroupsListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_advertiser_groups_list_builder(
            &self.http_client,
            &args.profileId,
            &args.ids,
            &args.maxResults,
            &args.pageToken,
            &args.searchString,
            &args.sortField,
            &args.sortOrder,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_advertiser_groups_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting advertiser groups patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AdvertiserGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_advertiser_groups_patch(
        &self,
        args: &DfareportingAdvertiserGroupsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AdvertiserGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_advertiser_groups_patch_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_advertiser_groups_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting advertiser groups update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AdvertiserGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_advertiser_groups_update(
        &self,
        args: &DfareportingAdvertiserGroupsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AdvertiserGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_advertiser_groups_update_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_advertiser_groups_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting advertiser invoices list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AdvertiserInvoicesListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_advertiser_invoices_list(
        &self,
        args: &DfareportingAdvertiserInvoicesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AdvertiserInvoicesListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_advertiser_invoices_list_builder(
            &self.http_client,
            &args.profileId,
            &args.advertiserId,
            &args.issueMonth,
            &args.maxResults,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_advertiser_invoices_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting advertiser landing pages get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LandingPage result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_advertiser_landing_pages_get(
        &self,
        args: &DfareportingAdvertiserLandingPagesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LandingPage, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_advertiser_landing_pages_get_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_advertiser_landing_pages_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting advertiser landing pages insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LandingPage result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_advertiser_landing_pages_insert(
        &self,
        args: &DfareportingAdvertiserLandingPagesInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LandingPage, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_advertiser_landing_pages_insert_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_advertiser_landing_pages_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting advertiser landing pages list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AdvertiserLandingPagesListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_advertiser_landing_pages_list(
        &self,
        args: &DfareportingAdvertiserLandingPagesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AdvertiserLandingPagesListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_advertiser_landing_pages_list_builder(
            &self.http_client,
            &args.profileId,
            &args.advertiserIds,
            &args.archived,
            &args.campaignIds,
            &args.ids,
            &args.maxResults,
            &args.pageToken,
            &args.searchString,
            &args.sortField,
            &args.sortOrder,
            &args.subaccountId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_advertiser_landing_pages_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting advertiser landing pages patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LandingPage result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_advertiser_landing_pages_patch(
        &self,
        args: &DfareportingAdvertiserLandingPagesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LandingPage, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_advertiser_landing_pages_patch_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_advertiser_landing_pages_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting advertiser landing pages update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LandingPage result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_advertiser_landing_pages_update(
        &self,
        args: &DfareportingAdvertiserLandingPagesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LandingPage, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_advertiser_landing_pages_update_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_advertiser_landing_pages_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting advertisers get.
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
    pub fn dfareporting_advertisers_get(
        &self,
        args: &DfareportingAdvertisersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Advertiser, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_advertisers_get_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_advertisers_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting advertisers insert.
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
    pub fn dfareporting_advertisers_insert(
        &self,
        args: &DfareportingAdvertisersInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Advertiser, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_advertisers_insert_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_advertisers_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting advertisers list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AdvertisersListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_advertisers_list(
        &self,
        args: &DfareportingAdvertisersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AdvertisersListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_advertisers_list_builder(
            &self.http_client,
            &args.profileId,
            &args.advertiserGroupIds,
            &args.floodlightConfigurationIds,
            &args.ids,
            &args.includeAdvertisersWithoutGroupsOnly,
            &args.maxResults,
            &args.onlyParent,
            &args.pageToken,
            &args.searchString,
            &args.sortField,
            &args.sortOrder,
            &args.status,
            &args.subaccountId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_advertisers_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting advertisers patch.
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
    pub fn dfareporting_advertisers_patch(
        &self,
        args: &DfareportingAdvertisersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Advertiser, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_advertisers_patch_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_advertisers_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting advertisers update.
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
    pub fn dfareporting_advertisers_update(
        &self,
        args: &DfareportingAdvertisersUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Advertiser, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_advertisers_update_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_advertisers_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting billing assignments insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BillingAssignment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_billing_assignments_insert(
        &self,
        args: &DfareportingBillingAssignmentsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BillingAssignment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_billing_assignments_insert_builder(
            &self.http_client,
            &args.profileId,
            &args.billingProfileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_billing_assignments_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting billing assignments list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BillingAssignmentsListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_billing_assignments_list(
        &self,
        args: &DfareportingBillingAssignmentsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BillingAssignmentsListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_billing_assignments_list_builder(
            &self.http_client,
            &args.profileId,
            &args.billingProfileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_billing_assignments_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting billing profiles get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BillingProfile result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_billing_profiles_get(
        &self,
        args: &DfareportingBillingProfilesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BillingProfile, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_billing_profiles_get_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_billing_profiles_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting billing profiles list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BillingProfilesListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_billing_profiles_list(
        &self,
        args: &DfareportingBillingProfilesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BillingProfilesListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_billing_profiles_list_builder(
            &self.http_client,
            &args.profileId,
            &args.currency_code,
            &args.ids,
            &args.maxResults,
            &args.name,
            &args.onlySuggestion,
            &args.pageToken,
            &args.sortField,
            &args.sortOrder,
            &args.status,
            &args.subaccountIds,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_billing_profiles_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting billing profiles update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BillingProfile result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_billing_profiles_update(
        &self,
        args: &DfareportingBillingProfilesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BillingProfile, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_billing_profiles_update_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_billing_profiles_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting billing rates list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BillingRatesListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_billing_rates_list(
        &self,
        args: &DfareportingBillingRatesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BillingRatesListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_billing_rates_list_builder(
            &self.http_client,
            &args.profileId,
            &args.billingProfileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_billing_rates_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting browsers list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BrowsersListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_browsers_list(
        &self,
        args: &DfareportingBrowsersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BrowsersListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_browsers_list_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_browsers_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting campaign creative associations insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CampaignCreativeAssociation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_campaign_creative_associations_insert(
        &self,
        args: &DfareportingCampaignCreativeAssociationsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CampaignCreativeAssociation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_campaign_creative_associations_insert_builder(
            &self.http_client,
            &args.profileId,
            &args.campaignId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_campaign_creative_associations_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting campaign creative associations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CampaignCreativeAssociationsListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_campaign_creative_associations_list(
        &self,
        args: &DfareportingCampaignCreativeAssociationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CampaignCreativeAssociationsListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_campaign_creative_associations_list_builder(
            &self.http_client,
            &args.profileId,
            &args.campaignId,
            &args.maxResults,
            &args.pageToken,
            &args.sortOrder,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_campaign_creative_associations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting campaigns get.
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
    pub fn dfareporting_campaigns_get(
        &self,
        args: &DfareportingCampaignsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Campaign, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_campaigns_get_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_campaigns_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting campaigns insert.
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
    pub fn dfareporting_campaigns_insert(
        &self,
        args: &DfareportingCampaignsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Campaign, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_campaigns_insert_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_campaigns_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting campaigns list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CampaignsListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_campaigns_list(
        &self,
        args: &DfareportingCampaignsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CampaignsListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_campaigns_list_builder(
            &self.http_client,
            &args.profileId,
            &args.advertiserGroupIds,
            &args.advertiserIds,
            &args.archived,
            &args.atLeastOneOptimizationActivity,
            &args.excludedIds,
            &args.ids,
            &args.maxResults,
            &args.overriddenEventTagId,
            &args.pageToken,
            &args.searchString,
            &args.sortField,
            &args.sortOrder,
            &args.subaccountId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_campaigns_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting campaigns patch.
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
    pub fn dfareporting_campaigns_patch(
        &self,
        args: &DfareportingCampaignsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Campaign, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_campaigns_patch_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_campaigns_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting campaigns update.
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
    pub fn dfareporting_campaigns_update(
        &self,
        args: &DfareportingCampaignsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Campaign, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_campaigns_update_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_campaigns_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting change logs get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ChangeLog result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_change_logs_get(
        &self,
        args: &DfareportingChangeLogsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ChangeLog, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_change_logs_get_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_change_logs_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting change logs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ChangeLogsListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_change_logs_list(
        &self,
        args: &DfareportingChangeLogsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ChangeLogsListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_change_logs_list_builder(
            &self.http_client,
            &args.profileId,
            &args.action,
            &args.ids,
            &args.maxChangeTime,
            &args.maxResults,
            &args.minChangeTime,
            &args.objectIds,
            &args.objectType,
            &args.pageToken,
            &args.searchString,
            &args.userProfileIds,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_change_logs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting cities list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CitiesListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_cities_list(
        &self,
        args: &DfareportingCitiesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CitiesListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_cities_list_builder(
            &self.http_client,
            &args.profileId,
            &args.countryDartIds,
            &args.dartIds,
            &args.namePrefix,
            &args.regionDartIds,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_cities_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting connection types get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ConnectionType result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_connection_types_get(
        &self,
        args: &DfareportingConnectionTypesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ConnectionType, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_connection_types_get_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_connection_types_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting connection types list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ConnectionTypesListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_connection_types_list(
        &self,
        args: &DfareportingConnectionTypesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ConnectionTypesListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_connection_types_list_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_connection_types_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting content categories delete.
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
    pub fn dfareporting_content_categories_delete(
        &self,
        args: &DfareportingContentCategoriesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_content_categories_delete_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_content_categories_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting content categories get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ContentCategory result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_content_categories_get(
        &self,
        args: &DfareportingContentCategoriesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ContentCategory, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_content_categories_get_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_content_categories_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting content categories insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ContentCategory result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_content_categories_insert(
        &self,
        args: &DfareportingContentCategoriesInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ContentCategory, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_content_categories_insert_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_content_categories_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting content categories list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ContentCategoriesListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_content_categories_list(
        &self,
        args: &DfareportingContentCategoriesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ContentCategoriesListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_content_categories_list_builder(
            &self.http_client,
            &args.profileId,
            &args.ids,
            &args.maxResults,
            &args.pageToken,
            &args.searchString,
            &args.sortField,
            &args.sortOrder,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_content_categories_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting content categories patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ContentCategory result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_content_categories_patch(
        &self,
        args: &DfareportingContentCategoriesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ContentCategory, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_content_categories_patch_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_content_categories_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting content categories update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ContentCategory result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_content_categories_update(
        &self,
        args: &DfareportingContentCategoriesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ContentCategory, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_content_categories_update_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_content_categories_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting conversions batchinsert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ConversionsBatchInsertResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_conversions_batchinsert(
        &self,
        args: &DfareportingConversionsBatchinsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ConversionsBatchInsertResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_conversions_batchinsert_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_conversions_batchinsert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting conversions batchupdate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ConversionsBatchUpdateResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_conversions_batchupdate(
        &self,
        args: &DfareportingConversionsBatchupdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ConversionsBatchUpdateResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_conversions_batchupdate_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_conversions_batchupdate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting countries get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Country result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_countries_get(
        &self,
        args: &DfareportingCountriesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Country, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_countries_get_builder(
            &self.http_client,
            &args.profileId,
            &args.dartId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_countries_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting countries list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CountriesListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_countries_list(
        &self,
        args: &DfareportingCountriesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CountriesListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_countries_list_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_countries_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting creative assets insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CreativeAssetMetadata result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_creative_assets_insert(
        &self,
        args: &DfareportingCreativeAssetsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CreativeAssetMetadata, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_creative_assets_insert_builder(
            &self.http_client,
            &args.profileId,
            &args.advertiserId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_creative_assets_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting creative field values delete.
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
    pub fn dfareporting_creative_field_values_delete(
        &self,
        args: &DfareportingCreativeFieldValuesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_creative_field_values_delete_builder(
            &self.http_client,
            &args.profileId,
            &args.creativeFieldId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_creative_field_values_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting creative field values get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CreativeFieldValue result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_creative_field_values_get(
        &self,
        args: &DfareportingCreativeFieldValuesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CreativeFieldValue, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_creative_field_values_get_builder(
            &self.http_client,
            &args.profileId,
            &args.creativeFieldId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_creative_field_values_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting creative field values insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CreativeFieldValue result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_creative_field_values_insert(
        &self,
        args: &DfareportingCreativeFieldValuesInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CreativeFieldValue, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_creative_field_values_insert_builder(
            &self.http_client,
            &args.profileId,
            &args.creativeFieldId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_creative_field_values_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting creative field values list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CreativeFieldValuesListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_creative_field_values_list(
        &self,
        args: &DfareportingCreativeFieldValuesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CreativeFieldValuesListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_creative_field_values_list_builder(
            &self.http_client,
            &args.profileId,
            &args.creativeFieldId,
            &args.ids,
            &args.maxResults,
            &args.pageToken,
            &args.searchString,
            &args.sortField,
            &args.sortOrder,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_creative_field_values_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting creative field values patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CreativeFieldValue result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_creative_field_values_patch(
        &self,
        args: &DfareportingCreativeFieldValuesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CreativeFieldValue, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_creative_field_values_patch_builder(
            &self.http_client,
            &args.profileId,
            &args.creativeFieldId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_creative_field_values_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting creative field values update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CreativeFieldValue result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_creative_field_values_update(
        &self,
        args: &DfareportingCreativeFieldValuesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CreativeFieldValue, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_creative_field_values_update_builder(
            &self.http_client,
            &args.profileId,
            &args.creativeFieldId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_creative_field_values_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting creative fields delete.
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
    pub fn dfareporting_creative_fields_delete(
        &self,
        args: &DfareportingCreativeFieldsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_creative_fields_delete_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_creative_fields_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting creative fields get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CreativeField result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_creative_fields_get(
        &self,
        args: &DfareportingCreativeFieldsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CreativeField, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_creative_fields_get_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_creative_fields_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting creative fields insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CreativeField result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_creative_fields_insert(
        &self,
        args: &DfareportingCreativeFieldsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CreativeField, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_creative_fields_insert_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_creative_fields_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting creative fields list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CreativeFieldsListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_creative_fields_list(
        &self,
        args: &DfareportingCreativeFieldsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CreativeFieldsListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_creative_fields_list_builder(
            &self.http_client,
            &args.profileId,
            &args.advertiserIds,
            &args.ids,
            &args.maxResults,
            &args.pageToken,
            &args.searchString,
            &args.sortField,
            &args.sortOrder,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_creative_fields_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting creative fields patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CreativeField result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_creative_fields_patch(
        &self,
        args: &DfareportingCreativeFieldsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CreativeField, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_creative_fields_patch_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_creative_fields_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting creative fields update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CreativeField result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_creative_fields_update(
        &self,
        args: &DfareportingCreativeFieldsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CreativeField, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_creative_fields_update_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_creative_fields_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting creative groups get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CreativeGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_creative_groups_get(
        &self,
        args: &DfareportingCreativeGroupsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CreativeGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_creative_groups_get_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_creative_groups_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting creative groups insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CreativeGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_creative_groups_insert(
        &self,
        args: &DfareportingCreativeGroupsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CreativeGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_creative_groups_insert_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_creative_groups_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting creative groups list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CreativeGroupsListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_creative_groups_list(
        &self,
        args: &DfareportingCreativeGroupsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CreativeGroupsListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_creative_groups_list_builder(
            &self.http_client,
            &args.profileId,
            &args.advertiserIds,
            &args.groupNumber,
            &args.ids,
            &args.maxResults,
            &args.pageToken,
            &args.searchString,
            &args.sortField,
            &args.sortOrder,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_creative_groups_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting creative groups patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CreativeGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_creative_groups_patch(
        &self,
        args: &DfareportingCreativeGroupsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CreativeGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_creative_groups_patch_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_creative_groups_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting creative groups update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CreativeGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_creative_groups_update(
        &self,
        args: &DfareportingCreativeGroupsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CreativeGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_creative_groups_update_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_creative_groups_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting creatives get.
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
    pub fn dfareporting_creatives_get(
        &self,
        args: &DfareportingCreativesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Creative, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_creatives_get_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_creatives_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting creatives insert.
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
    pub fn dfareporting_creatives_insert(
        &self,
        args: &DfareportingCreativesInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Creative, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_creatives_insert_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_creatives_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting creatives list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CreativesListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_creatives_list(
        &self,
        args: &DfareportingCreativesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CreativesListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_creatives_list_builder(
            &self.http_client,
            &args.profileId,
            &args.active,
            &args.advertiserId,
            &args.archived,
            &args.campaignId,
            &args.companionCreativeIds,
            &args.creativeFieldIds,
            &args.ids,
            &args.maxResults,
            &args.pageToken,
            &args.renderingIds,
            &args.searchString,
            &args.sizeIds,
            &args.sortField,
            &args.sortOrder,
            &args.studioCreativeId,
            &args.types,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_creatives_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting creatives patch.
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
    pub fn dfareporting_creatives_patch(
        &self,
        args: &DfareportingCreativesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Creative, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_creatives_patch_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_creatives_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting creatives update.
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
    pub fn dfareporting_creatives_update(
        &self,
        args: &DfareportingCreativesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Creative, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_creatives_update_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_creatives_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting dimension values query.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DimensionValueList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_dimension_values_query(
        &self,
        args: &DfareportingDimensionValuesQueryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DimensionValueList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_dimension_values_query_builder(
            &self.http_client,
            &args.profileId,
            &args.maxResults,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_dimension_values_query_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting directory sites get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DirectorySite result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_directory_sites_get(
        &self,
        args: &DfareportingDirectorySitesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DirectorySite, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_directory_sites_get_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_directory_sites_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting directory sites insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DirectorySite result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_directory_sites_insert(
        &self,
        args: &DfareportingDirectorySitesInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DirectorySite, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_directory_sites_insert_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_directory_sites_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting directory sites list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DirectorySitesListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_directory_sites_list(
        &self,
        args: &DfareportingDirectorySitesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DirectorySitesListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_directory_sites_list_builder(
            &self.http_client,
            &args.profileId,
            &args.acceptsInStreamVideoPlacements,
            &args.acceptsInterstitialPlacements,
            &args.acceptsPublisherPaidPlacements,
            &args.active,
            &args.dfpNetworkCode,
            &args.ids,
            &args.maxResults,
            &args.pageToken,
            &args.searchString,
            &args.sortField,
            &args.sortOrder,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_directory_sites_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting dynamic feeds get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DynamicFeed result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_dynamic_feeds_get(
        &self,
        args: &DfareportingDynamicFeedsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DynamicFeed, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_dynamic_feeds_get_builder(
            &self.http_client,
            &args.dynamicFeedId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_dynamic_feeds_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting dynamic feeds insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DynamicFeed result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_dynamic_feeds_insert(
        &self,
        args: &DfareportingDynamicFeedsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DynamicFeed, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_dynamic_feeds_insert_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_dynamic_feeds_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting dynamic feeds retransform.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DynamicFeed result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_dynamic_feeds_retransform(
        &self,
        args: &DfareportingDynamicFeedsRetransformArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DynamicFeed, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_dynamic_feeds_retransform_builder(
            &self.http_client,
            &args.dynamicFeedId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_dynamic_feeds_retransform_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting dynamic feeds update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DynamicFeed result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_dynamic_feeds_update(
        &self,
        args: &DfareportingDynamicFeedsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DynamicFeed, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_dynamic_feeds_update_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_dynamic_feeds_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting dynamic profiles generate code.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DynamicProfileGenerateCodeResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_dynamic_profiles_generate_code(
        &self,
        args: &DfareportingDynamicProfilesGenerateCodeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DynamicProfileGenerateCodeResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_dynamic_profiles_generate_code_builder(
            &self.http_client,
            &args.dynamicProfileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_dynamic_profiles_generate_code_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting dynamic profiles get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DynamicProfile result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_dynamic_profiles_get(
        &self,
        args: &DfareportingDynamicProfilesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DynamicProfile, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_dynamic_profiles_get_builder(
            &self.http_client,
            &args.dynamicProfileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_dynamic_profiles_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting dynamic profiles insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DynamicProfile result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_dynamic_profiles_insert(
        &self,
        args: &DfareportingDynamicProfilesInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DynamicProfile, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_dynamic_profiles_insert_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_dynamic_profiles_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting dynamic profiles publish.
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
    pub fn dfareporting_dynamic_profiles_publish(
        &self,
        args: &DfareportingDynamicProfilesPublishArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_dynamic_profiles_publish_builder(
            &self.http_client,
            &args.dynamicProfileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_dynamic_profiles_publish_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting dynamic profiles update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DynamicProfile result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_dynamic_profiles_update(
        &self,
        args: &DfareportingDynamicProfilesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DynamicProfile, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_dynamic_profiles_update_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_dynamic_profiles_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting dynamic targeting keys delete.
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
    pub fn dfareporting_dynamic_targeting_keys_delete(
        &self,
        args: &DfareportingDynamicTargetingKeysDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_dynamic_targeting_keys_delete_builder(
            &self.http_client,
            &args.profileId,
            &args.objectId,
            &args.name,
            &args.objectType,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_dynamic_targeting_keys_delete_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting dynamic targeting keys insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DynamicTargetingKey result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_dynamic_targeting_keys_insert(
        &self,
        args: &DfareportingDynamicTargetingKeysInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DynamicTargetingKey, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_dynamic_targeting_keys_insert_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_dynamic_targeting_keys_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting dynamic targeting keys list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DynamicTargetingKeysListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_dynamic_targeting_keys_list(
        &self,
        args: &DfareportingDynamicTargetingKeysListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DynamicTargetingKeysListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_dynamic_targeting_keys_list_builder(
            &self.http_client,
            &args.profileId,
            &args.advertiserId,
            &args.names,
            &args.objectId,
            &args.objectType,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_dynamic_targeting_keys_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting event tags delete.
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
    pub fn dfareporting_event_tags_delete(
        &self,
        args: &DfareportingEventTagsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_event_tags_delete_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_event_tags_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting event tags get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EventTag result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_event_tags_get(
        &self,
        args: &DfareportingEventTagsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EventTag, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_event_tags_get_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_event_tags_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting event tags insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EventTag result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_event_tags_insert(
        &self,
        args: &DfareportingEventTagsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EventTag, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_event_tags_insert_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_event_tags_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting event tags list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EventTagsListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_event_tags_list(
        &self,
        args: &DfareportingEventTagsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EventTagsListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_event_tags_list_builder(
            &self.http_client,
            &args.profileId,
            &args.adId,
            &args.advertiserId,
            &args.campaignId,
            &args.definitionsOnly,
            &args.enabled,
            &args.eventTagTypes,
            &args.ids,
            &args.searchString,
            &args.sortField,
            &args.sortOrder,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_event_tags_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting event tags patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EventTag result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_event_tags_patch(
        &self,
        args: &DfareportingEventTagsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EventTag, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_event_tags_patch_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_event_tags_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting event tags update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EventTag result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_event_tags_update(
        &self,
        args: &DfareportingEventTagsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EventTag, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_event_tags_update_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_event_tags_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting files get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the File result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_files_get(
        &self,
        args: &DfareportingFilesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<File, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_files_get_builder(
            &self.http_client,
            &args.reportId,
            &args.fileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_files_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting files list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FileList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_files_list(
        &self,
        args: &DfareportingFilesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FileList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_files_list_builder(
            &self.http_client,
            &args.profileId,
            &args.maxResults,
            &args.pageToken,
            &args.scope,
            &args.sortField,
            &args.sortOrder,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_files_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting floodlight activities delete.
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
    pub fn dfareporting_floodlight_activities_delete(
        &self,
        args: &DfareportingFloodlightActivitiesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_floodlight_activities_delete_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_floodlight_activities_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting floodlight activities generatetag.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FloodlightActivitiesGenerateTagResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_floodlight_activities_generatetag(
        &self,
        args: &DfareportingFloodlightActivitiesGeneratetagArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FloodlightActivitiesGenerateTagResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_floodlight_activities_generatetag_builder(
            &self.http_client,
            &args.profileId,
            &args.floodlightActivityId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_floodlight_activities_generatetag_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting floodlight activities get.
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
    pub fn dfareporting_floodlight_activities_get(
        &self,
        args: &DfareportingFloodlightActivitiesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FloodlightActivity, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_floodlight_activities_get_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_floodlight_activities_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting floodlight activities insert.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_floodlight_activities_insert(
        &self,
        args: &DfareportingFloodlightActivitiesInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FloodlightActivity, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_floodlight_activities_insert_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_floodlight_activities_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting floodlight activities list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FloodlightActivitiesListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_floodlight_activities_list(
        &self,
        args: &DfareportingFloodlightActivitiesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FloodlightActivitiesListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_floodlight_activities_list_builder(
            &self.http_client,
            &args.profileId,
            &args.advertiserId,
            &args.floodlightActivityGroupIds,
            &args.floodlightActivityGroupName,
            &args.floodlightActivityGroupTagString,
            &args.floodlightActivityGroupType,
            &args.floodlightConfigurationId,
            &args.ids,
            &args.maxResults,
            &args.pageToken,
            &args.searchString,
            &args.sortField,
            &args.sortOrder,
            &args.tagString,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_floodlight_activities_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting floodlight activities patch.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_floodlight_activities_patch(
        &self,
        args: &DfareportingFloodlightActivitiesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FloodlightActivity, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_floodlight_activities_patch_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_floodlight_activities_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting floodlight activities update.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_floodlight_activities_update(
        &self,
        args: &DfareportingFloodlightActivitiesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FloodlightActivity, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_floodlight_activities_update_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_floodlight_activities_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting floodlight activity groups get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FloodlightActivityGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_floodlight_activity_groups_get(
        &self,
        args: &DfareportingFloodlightActivityGroupsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FloodlightActivityGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_floodlight_activity_groups_get_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_floodlight_activity_groups_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting floodlight activity groups insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FloodlightActivityGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_floodlight_activity_groups_insert(
        &self,
        args: &DfareportingFloodlightActivityGroupsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FloodlightActivityGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_floodlight_activity_groups_insert_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_floodlight_activity_groups_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting floodlight activity groups list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FloodlightActivityGroupsListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_floodlight_activity_groups_list(
        &self,
        args: &DfareportingFloodlightActivityGroupsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FloodlightActivityGroupsListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_floodlight_activity_groups_list_builder(
            &self.http_client,
            &args.profileId,
            &args.advertiserId,
            &args.floodlightConfigurationId,
            &args.ids,
            &args.maxResults,
            &args.pageToken,
            &args.searchString,
            &args.sortField,
            &args.sortOrder,
            &args.type_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_floodlight_activity_groups_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting floodlight activity groups patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FloodlightActivityGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_floodlight_activity_groups_patch(
        &self,
        args: &DfareportingFloodlightActivityGroupsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FloodlightActivityGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_floodlight_activity_groups_patch_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_floodlight_activity_groups_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting floodlight activity groups update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FloodlightActivityGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_floodlight_activity_groups_update(
        &self,
        args: &DfareportingFloodlightActivityGroupsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FloodlightActivityGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_floodlight_activity_groups_update_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_floodlight_activity_groups_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting floodlight configurations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FloodlightConfiguration result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_floodlight_configurations_get(
        &self,
        args: &DfareportingFloodlightConfigurationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FloodlightConfiguration, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_floodlight_configurations_get_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_floodlight_configurations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting floodlight configurations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FloodlightConfigurationsListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_floodlight_configurations_list(
        &self,
        args: &DfareportingFloodlightConfigurationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FloodlightConfigurationsListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_floodlight_configurations_list_builder(
            &self.http_client,
            &args.profileId,
            &args.ids,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_floodlight_configurations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting floodlight configurations patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FloodlightConfiguration result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_floodlight_configurations_patch(
        &self,
        args: &DfareportingFloodlightConfigurationsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FloodlightConfiguration, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_floodlight_configurations_patch_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_floodlight_configurations_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting floodlight configurations update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FloodlightConfiguration result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_floodlight_configurations_update(
        &self,
        args: &DfareportingFloodlightConfigurationsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FloodlightConfiguration, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_floodlight_configurations_update_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_floodlight_configurations_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting languages list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LanguagesListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_languages_list(
        &self,
        args: &DfareportingLanguagesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LanguagesListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_languages_list_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_languages_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting metros list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the MetrosListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_metros_list(
        &self,
        args: &DfareportingMetrosListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<MetrosListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_metros_list_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_metros_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting mobile apps get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the MobileApp result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_mobile_apps_get(
        &self,
        args: &DfareportingMobileAppsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<MobileApp, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_mobile_apps_get_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_mobile_apps_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting mobile apps list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the MobileAppsListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_mobile_apps_list(
        &self,
        args: &DfareportingMobileAppsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<MobileAppsListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_mobile_apps_list_builder(
            &self.http_client,
            &args.profileId,
            &args.directories,
            &args.ids,
            &args.maxResults,
            &args.pageToken,
            &args.searchString,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_mobile_apps_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting mobile carriers get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the MobileCarrier result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_mobile_carriers_get(
        &self,
        args: &DfareportingMobileCarriersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<MobileCarrier, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_mobile_carriers_get_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_mobile_carriers_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting mobile carriers list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the MobileCarriersListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_mobile_carriers_list(
        &self,
        args: &DfareportingMobileCarriersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<MobileCarriersListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_mobile_carriers_list_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_mobile_carriers_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting operating system versions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the OperatingSystemVersion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_operating_system_versions_get(
        &self,
        args: &DfareportingOperatingSystemVersionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<OperatingSystemVersion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_operating_system_versions_get_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_operating_system_versions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting operating system versions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the OperatingSystemVersionsListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_operating_system_versions_list(
        &self,
        args: &DfareportingOperatingSystemVersionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<OperatingSystemVersionsListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_operating_system_versions_list_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_operating_system_versions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting operating systems get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the OperatingSystem result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_operating_systems_get(
        &self,
        args: &DfareportingOperatingSystemsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<OperatingSystem, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_operating_systems_get_builder(
            &self.http_client,
            &args.profileId,
            &args.dartId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_operating_systems_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting operating systems list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the OperatingSystemsListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_operating_systems_list(
        &self,
        args: &DfareportingOperatingSystemsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<OperatingSystemsListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_operating_systems_list_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_operating_systems_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting placement groups get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PlacementGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_placement_groups_get(
        &self,
        args: &DfareportingPlacementGroupsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PlacementGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_placement_groups_get_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_placement_groups_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting placement groups insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PlacementGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_placement_groups_insert(
        &self,
        args: &DfareportingPlacementGroupsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PlacementGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_placement_groups_insert_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_placement_groups_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting placement groups list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PlacementGroupsListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_placement_groups_list(
        &self,
        args: &DfareportingPlacementGroupsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PlacementGroupsListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_placement_groups_list_builder(
            &self.http_client,
            &args.profileId,
            &args.activeStatus,
            &args.advertiserIds,
            &args.campaignIds,
            &args.contentCategoryIds,
            &args.directorySiteIds,
            &args.ids,
            &args.maxEndDate,
            &args.maxResults,
            &args.maxStartDate,
            &args.minEndDate,
            &args.minStartDate,
            &args.pageToken,
            &args.placementGroupType,
            &args.placementStrategyIds,
            &args.pricingTypes,
            &args.searchString,
            &args.siteIds,
            &args.sortField,
            &args.sortOrder,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_placement_groups_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting placement groups patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PlacementGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_placement_groups_patch(
        &self,
        args: &DfareportingPlacementGroupsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PlacementGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_placement_groups_patch_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_placement_groups_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting placement groups update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PlacementGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_placement_groups_update(
        &self,
        args: &DfareportingPlacementGroupsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PlacementGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_placement_groups_update_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_placement_groups_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting placement strategies delete.
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
    pub fn dfareporting_placement_strategies_delete(
        &self,
        args: &DfareportingPlacementStrategiesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_placement_strategies_delete_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_placement_strategies_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting placement strategies get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PlacementStrategy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_placement_strategies_get(
        &self,
        args: &DfareportingPlacementStrategiesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PlacementStrategy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_placement_strategies_get_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_placement_strategies_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting placement strategies insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PlacementStrategy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_placement_strategies_insert(
        &self,
        args: &DfareportingPlacementStrategiesInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PlacementStrategy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_placement_strategies_insert_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_placement_strategies_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting placement strategies list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PlacementStrategiesListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_placement_strategies_list(
        &self,
        args: &DfareportingPlacementStrategiesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PlacementStrategiesListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_placement_strategies_list_builder(
            &self.http_client,
            &args.profileId,
            &args.ids,
            &args.maxResults,
            &args.pageToken,
            &args.searchString,
            &args.sortField,
            &args.sortOrder,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_placement_strategies_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting placement strategies patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PlacementStrategy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_placement_strategies_patch(
        &self,
        args: &DfareportingPlacementStrategiesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PlacementStrategy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_placement_strategies_patch_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_placement_strategies_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting placement strategies update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PlacementStrategy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_placement_strategies_update(
        &self,
        args: &DfareportingPlacementStrategiesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PlacementStrategy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_placement_strategies_update_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_placement_strategies_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting placements generatetags.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PlacementsGenerateTagsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_placements_generatetags(
        &self,
        args: &DfareportingPlacementsGeneratetagsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PlacementsGenerateTagsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_placements_generatetags_builder(
            &self.http_client,
            &args.profileId,
            &args.campaignId,
            &args.placementIds,
            &args.tagFormats,
            &args.tagProperties.dcDbmMacroIncluded,
            &args.tagProperties.gppMacrosIncluded,
            &args.tagProperties.tcfGdprMacrosIncluded,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_placements_generatetags_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting placements get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Placement result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_placements_get(
        &self,
        args: &DfareportingPlacementsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Placement, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_placements_get_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_placements_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting placements insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Placement result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_placements_insert(
        &self,
        args: &DfareportingPlacementsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Placement, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_placements_insert_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_placements_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting placements list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PlacementsListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_placements_list(
        &self,
        args: &DfareportingPlacementsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PlacementsListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_placements_list_builder(
            &self.http_client,
            &args.profileId,
            &args.activeStatus,
            &args.advertiserIds,
            &args.campaignIds,
            &args.compatibilities,
            &args.contentCategoryIds,
            &args.directorySiteIds,
            &args.groupIds,
            &args.ids,
            &args.maxEndDate,
            &args.maxResults,
            &args.maxStartDate,
            &args.minEndDate,
            &args.minStartDate,
            &args.pageToken,
            &args.paymentSource,
            &args.placementStrategyIds,
            &args.pricingTypes,
            &args.searchString,
            &args.siteIds,
            &args.sizeIds,
            &args.sortField,
            &args.sortOrder,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_placements_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting placements patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Placement result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_placements_patch(
        &self,
        args: &DfareportingPlacementsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Placement, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_placements_patch_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_placements_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting placements update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Placement result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_placements_update(
        &self,
        args: &DfareportingPlacementsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Placement, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_placements_update_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_placements_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting platform types get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PlatformType result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_platform_types_get(
        &self,
        args: &DfareportingPlatformTypesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PlatformType, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_platform_types_get_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_platform_types_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting platform types list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PlatformTypesListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_platform_types_list(
        &self,
        args: &DfareportingPlatformTypesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PlatformTypesListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_platform_types_list_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_platform_types_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting postal codes get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PostalCode result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_postal_codes_get(
        &self,
        args: &DfareportingPostalCodesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PostalCode, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_postal_codes_get_builder(
            &self.http_client,
            &args.profileId,
            &args.code,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_postal_codes_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting postal codes list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PostalCodesListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_postal_codes_list(
        &self,
        args: &DfareportingPostalCodesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PostalCodesListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_postal_codes_list_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_postal_codes_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting regions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RegionsListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_regions_list(
        &self,
        args: &DfareportingRegionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RegionsListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_regions_list_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_regions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting remarketing list shares get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RemarketingListShare result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_remarketing_list_shares_get(
        &self,
        args: &DfareportingRemarketingListSharesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RemarketingListShare, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_remarketing_list_shares_get_builder(
            &self.http_client,
            &args.profileId,
            &args.remarketingListId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_remarketing_list_shares_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting remarketing list shares patch.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RemarketingListShare result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_remarketing_list_shares_patch(
        &self,
        args: &DfareportingRemarketingListSharesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RemarketingListShare, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_remarketing_list_shares_patch_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_remarketing_list_shares_patch_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting remarketing list shares update.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RemarketingListShare result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_remarketing_list_shares_update(
        &self,
        args: &DfareportingRemarketingListSharesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RemarketingListShare, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_remarketing_list_shares_update_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_remarketing_list_shares_update_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting remarketing lists get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RemarketingList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_remarketing_lists_get(
        &self,
        args: &DfareportingRemarketingListsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RemarketingList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_remarketing_lists_get_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_remarketing_lists_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting remarketing lists insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RemarketingList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_remarketing_lists_insert(
        &self,
        args: &DfareportingRemarketingListsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RemarketingList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_remarketing_lists_insert_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_remarketing_lists_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting remarketing lists list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RemarketingListsListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_remarketing_lists_list(
        &self,
        args: &DfareportingRemarketingListsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RemarketingListsListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_remarketing_lists_list_builder(
            &self.http_client,
            &args.profileId,
            &args.active,
            &args.advertiserId,
            &args.floodlightActivityId,
            &args.maxResults,
            &args.name,
            &args.pageToken,
            &args.sortField,
            &args.sortOrder,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_remarketing_lists_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting remarketing lists patch.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RemarketingList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_remarketing_lists_patch(
        &self,
        args: &DfareportingRemarketingListsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RemarketingList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_remarketing_lists_patch_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_remarketing_lists_patch_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting remarketing lists update.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RemarketingList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_remarketing_lists_update(
        &self,
        args: &DfareportingRemarketingListsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RemarketingList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_remarketing_lists_update_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_remarketing_lists_update_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting reports delete.
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
    pub fn dfareporting_reports_delete(
        &self,
        args: &DfareportingReportsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_reports_delete_builder(
            &self.http_client,
            &args.profileId,
            &args.reportId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_reports_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting reports get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Report result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_reports_get(
        &self,
        args: &DfareportingReportsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Report, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_reports_get_builder(
            &self.http_client,
            &args.profileId,
            &args.reportId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_reports_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting reports insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Report result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_reports_insert(
        &self,
        args: &DfareportingReportsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Report, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_reports_insert_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_reports_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting reports list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ReportList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_reports_list(
        &self,
        args: &DfareportingReportsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ReportList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_reports_list_builder(
            &self.http_client,
            &args.profileId,
            &args.maxResults,
            &args.pageToken,
            &args.scope,
            &args.sortField,
            &args.sortOrder,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_reports_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting reports run.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the File result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_reports_run(
        &self,
        args: &DfareportingReportsRunArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<File, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_reports_run_builder(
            &self.http_client,
            &args.profileId,
            &args.reportId,
            &args.synchronous,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_reports_run_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting reports update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Report result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_reports_update(
        &self,
        args: &DfareportingReportsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Report, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_reports_update_builder(
            &self.http_client,
            &args.profileId,
            &args.reportId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_reports_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting reports compatible fields query.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CompatibleFields result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_reports_compatible_fields_query(
        &self,
        args: &DfareportingReportsCompatibleFieldsQueryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CompatibleFields, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_reports_compatible_fields_query_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_reports_compatible_fields_query_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting reports files get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the File result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_reports_files_get(
        &self,
        args: &DfareportingReportsFilesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<File, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_reports_files_get_builder(
            &self.http_client,
            &args.profileId,
            &args.reportId,
            &args.fileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_reports_files_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting reports files list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FileList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_reports_files_list(
        &self,
        args: &DfareportingReportsFilesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FileList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_reports_files_list_builder(
            &self.http_client,
            &args.profileId,
            &args.reportId,
            &args.maxResults,
            &args.pageToken,
            &args.sortField,
            &args.sortOrder,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_reports_files_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting sites get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_sites_get(
        &self,
        args: &DfareportingSitesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Site, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_sites_get_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_sites_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting sites insert.
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
    pub fn dfareporting_sites_insert(
        &self,
        args: &DfareportingSitesInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Site, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_sites_insert_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_sites_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting sites list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SitesListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_sites_list(
        &self,
        args: &DfareportingSitesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SitesListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_sites_list_builder(
            &self.http_client,
            &args.profileId,
            &args.acceptsInStreamVideoPlacements,
            &args.acceptsInterstitialPlacements,
            &args.acceptsPublisherPaidPlacements,
            &args.adWordsSite,
            &args.approved,
            &args.campaignIds,
            &args.directorySiteIds,
            &args.ids,
            &args.maxResults,
            &args.pageToken,
            &args.searchString,
            &args.sortField,
            &args.sortOrder,
            &args.subaccountId,
            &args.unmappedSite,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_sites_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting sites patch.
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
    pub fn dfareporting_sites_patch(
        &self,
        args: &DfareportingSitesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Site, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_sites_patch_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_sites_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting sites update.
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
    pub fn dfareporting_sites_update(
        &self,
        args: &DfareportingSitesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Site, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_sites_update_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_sites_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting sizes get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Size result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_sizes_get(
        &self,
        args: &DfareportingSizesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Size, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_sizes_get_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_sizes_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting sizes insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Size result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_sizes_insert(
        &self,
        args: &DfareportingSizesInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Size, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_sizes_insert_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_sizes_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting sizes list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SizesListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_sizes_list(
        &self,
        args: &DfareportingSizesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SizesListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_sizes_list_builder(
            &self.http_client,
            &args.profileId,
            &args.height,
            &args.iabStandard,
            &args.ids,
            &args.width,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_sizes_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting studio creative assets insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the StudioCreativeAssetsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_studio_creative_assets_insert(
        &self,
        args: &DfareportingStudioCreativeAssetsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<StudioCreativeAssetsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_studio_creative_assets_insert_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_studio_creative_assets_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting studio creatives get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the StudioCreative result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_studio_creatives_get(
        &self,
        args: &DfareportingStudioCreativesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<StudioCreative, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_studio_creatives_get_builder(
            &self.http_client,
            &args.studioCreativeId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_studio_creatives_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting studio creatives insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the StudioCreative result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_studio_creatives_insert(
        &self,
        args: &DfareportingStudioCreativesInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<StudioCreative, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_studio_creatives_insert_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_studio_creatives_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting studio creatives publish.
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
    pub fn dfareporting_studio_creatives_publish(
        &self,
        args: &DfareportingStudioCreativesPublishArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_studio_creatives_publish_builder(
            &self.http_client,
            &args.studioCreativeId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_studio_creatives_publish_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting subaccounts get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Subaccount result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_subaccounts_get(
        &self,
        args: &DfareportingSubaccountsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Subaccount, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_subaccounts_get_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_subaccounts_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting subaccounts insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Subaccount result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_subaccounts_insert(
        &self,
        args: &DfareportingSubaccountsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Subaccount, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_subaccounts_insert_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_subaccounts_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting subaccounts list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SubaccountsListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_subaccounts_list(
        &self,
        args: &DfareportingSubaccountsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SubaccountsListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_subaccounts_list_builder(
            &self.http_client,
            &args.profileId,
            &args.ids,
            &args.maxResults,
            &args.pageToken,
            &args.searchString,
            &args.sortField,
            &args.sortOrder,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_subaccounts_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting subaccounts patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Subaccount result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_subaccounts_patch(
        &self,
        args: &DfareportingSubaccountsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Subaccount, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_subaccounts_patch_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_subaccounts_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting subaccounts update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Subaccount result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_subaccounts_update(
        &self,
        args: &DfareportingSubaccountsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Subaccount, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_subaccounts_update_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_subaccounts_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting targetable remarketing lists get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TargetableRemarketingList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_targetable_remarketing_lists_get(
        &self,
        args: &DfareportingTargetableRemarketingListsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TargetableRemarketingList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_targetable_remarketing_lists_get_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_targetable_remarketing_lists_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting targetable remarketing lists list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TargetableRemarketingListsListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_targetable_remarketing_lists_list(
        &self,
        args: &DfareportingTargetableRemarketingListsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TargetableRemarketingListsListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_targetable_remarketing_lists_list_builder(
            &self.http_client,
            &args.profileId,
            &args.active,
            &args.advertiserId,
            &args.maxResults,
            &args.name,
            &args.pageToken,
            &args.sortField,
            &args.sortOrder,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_targetable_remarketing_lists_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting targeting templates get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TargetingTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_targeting_templates_get(
        &self,
        args: &DfareportingTargetingTemplatesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TargetingTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_targeting_templates_get_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_targeting_templates_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting targeting templates insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TargetingTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_targeting_templates_insert(
        &self,
        args: &DfareportingTargetingTemplatesInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TargetingTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_targeting_templates_insert_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_targeting_templates_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting targeting templates list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TargetingTemplatesListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_targeting_templates_list(
        &self,
        args: &DfareportingTargetingTemplatesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TargetingTemplatesListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_targeting_templates_list_builder(
            &self.http_client,
            &args.profileId,
            &args.advertiserId,
            &args.ids,
            &args.maxResults,
            &args.pageToken,
            &args.searchString,
            &args.sortField,
            &args.sortOrder,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_targeting_templates_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting targeting templates patch.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TargetingTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_targeting_templates_patch(
        &self,
        args: &DfareportingTargetingTemplatesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TargetingTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_targeting_templates_patch_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_targeting_templates_patch_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting targeting templates update.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TargetingTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_targeting_templates_update(
        &self,
        args: &DfareportingTargetingTemplatesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TargetingTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_targeting_templates_update_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_targeting_templates_update_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting tv campaign details get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TvCampaignDetail result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_tv_campaign_details_get(
        &self,
        args: &DfareportingTvCampaignDetailsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TvCampaignDetail, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_tv_campaign_details_get_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
            &args.accountId,
            &args.countryDartId,
            &args.tvDataProvider,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_tv_campaign_details_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting tv campaign summaries list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TvCampaignSummariesListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_tv_campaign_summaries_list(
        &self,
        args: &DfareportingTvCampaignSummariesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TvCampaignSummariesListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_tv_campaign_summaries_list_builder(
            &self.http_client,
            &args.profileId,
            &args.accountId,
            &args.countryDartId,
            &args.name,
            &args.tvDataProvider,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_tv_campaign_summaries_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting user profiles get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UserProfile result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_user_profiles_get(
        &self,
        args: &DfareportingUserProfilesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UserProfile, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_user_profiles_get_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_user_profiles_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting user profiles list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UserProfileList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_user_profiles_list(
        &self,
        args: &DfareportingUserProfilesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UserProfileList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_user_profiles_list_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_user_profiles_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting user role permission groups get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UserRolePermissionGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_user_role_permission_groups_get(
        &self,
        args: &DfareportingUserRolePermissionGroupsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UserRolePermissionGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_user_role_permission_groups_get_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_user_role_permission_groups_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting user role permission groups list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UserRolePermissionGroupsListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_user_role_permission_groups_list(
        &self,
        args: &DfareportingUserRolePermissionGroupsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UserRolePermissionGroupsListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_user_role_permission_groups_list_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_user_role_permission_groups_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting user role permissions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UserRolePermission result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_user_role_permissions_get(
        &self,
        args: &DfareportingUserRolePermissionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UserRolePermission, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_user_role_permissions_get_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_user_role_permissions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting user role permissions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UserRolePermissionsListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_user_role_permissions_list(
        &self,
        args: &DfareportingUserRolePermissionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UserRolePermissionsListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_user_role_permissions_list_builder(
            &self.http_client,
            &args.profileId,
            &args.ids,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_user_role_permissions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting user roles delete.
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
    pub fn dfareporting_user_roles_delete(
        &self,
        args: &DfareportingUserRolesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_user_roles_delete_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_user_roles_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting user roles get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UserRole result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_user_roles_get(
        &self,
        args: &DfareportingUserRolesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UserRole, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_user_roles_get_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_user_roles_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting user roles insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UserRole result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_user_roles_insert(
        &self,
        args: &DfareportingUserRolesInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UserRole, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_user_roles_insert_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_user_roles_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting user roles list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UserRolesListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_user_roles_list(
        &self,
        args: &DfareportingUserRolesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UserRolesListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_user_roles_list_builder(
            &self.http_client,
            &args.profileId,
            &args.accountUserRoleOnly,
            &args.ids,
            &args.maxResults,
            &args.pageToken,
            &args.searchString,
            &args.sortField,
            &args.sortOrder,
            &args.subaccountId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_user_roles_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting user roles patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UserRole result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_user_roles_patch(
        &self,
        args: &DfareportingUserRolesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UserRole, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_user_roles_patch_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_user_roles_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting user roles update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UserRole result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dfareporting_user_roles_update(
        &self,
        args: &DfareportingUserRolesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UserRole, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_user_roles_update_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_user_roles_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting video formats get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the VideoFormat result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_video_formats_get(
        &self,
        args: &DfareportingVideoFormatsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<VideoFormat, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_video_formats_get_builder(
            &self.http_client,
            &args.profileId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_video_formats_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting video formats list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the VideoFormatsListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dfareporting_video_formats_list(
        &self,
        args: &DfareportingVideoFormatsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<VideoFormatsListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dfareporting_video_formats_list_builder(
            &self.http_client,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_video_formats_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
