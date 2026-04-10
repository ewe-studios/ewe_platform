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
    dfareporting_account_user_profiles_insert_builder, dfareporting_account_user_profiles_insert_task,
    dfareporting_account_user_profiles_patch_builder, dfareporting_account_user_profiles_patch_task,
    dfareporting_account_user_profiles_update_builder, dfareporting_account_user_profiles_update_task,
    dfareporting_accounts_patch_builder, dfareporting_accounts_patch_task,
    dfareporting_accounts_update_builder, dfareporting_accounts_update_task,
    dfareporting_ads_insert_builder, dfareporting_ads_insert_task,
    dfareporting_ads_patch_builder, dfareporting_ads_patch_task,
    dfareporting_ads_update_builder, dfareporting_ads_update_task,
    dfareporting_advertiser_groups_delete_builder, dfareporting_advertiser_groups_delete_task,
    dfareporting_advertiser_groups_insert_builder, dfareporting_advertiser_groups_insert_task,
    dfareporting_advertiser_groups_patch_builder, dfareporting_advertiser_groups_patch_task,
    dfareporting_advertiser_groups_update_builder, dfareporting_advertiser_groups_update_task,
    dfareporting_advertiser_landing_pages_insert_builder, dfareporting_advertiser_landing_pages_insert_task,
    dfareporting_advertiser_landing_pages_patch_builder, dfareporting_advertiser_landing_pages_patch_task,
    dfareporting_advertiser_landing_pages_update_builder, dfareporting_advertiser_landing_pages_update_task,
    dfareporting_advertisers_insert_builder, dfareporting_advertisers_insert_task,
    dfareporting_advertisers_patch_builder, dfareporting_advertisers_patch_task,
    dfareporting_advertisers_update_builder, dfareporting_advertisers_update_task,
    dfareporting_billing_assignments_insert_builder, dfareporting_billing_assignments_insert_task,
    dfareporting_billing_profiles_update_builder, dfareporting_billing_profiles_update_task,
    dfareporting_campaign_creative_associations_insert_builder, dfareporting_campaign_creative_associations_insert_task,
    dfareporting_campaigns_insert_builder, dfareporting_campaigns_insert_task,
    dfareporting_campaigns_patch_builder, dfareporting_campaigns_patch_task,
    dfareporting_campaigns_update_builder, dfareporting_campaigns_update_task,
    dfareporting_content_categories_delete_builder, dfareporting_content_categories_delete_task,
    dfareporting_content_categories_insert_builder, dfareporting_content_categories_insert_task,
    dfareporting_content_categories_patch_builder, dfareporting_content_categories_patch_task,
    dfareporting_content_categories_update_builder, dfareporting_content_categories_update_task,
    dfareporting_conversions_batchinsert_builder, dfareporting_conversions_batchinsert_task,
    dfareporting_conversions_batchupdate_builder, dfareporting_conversions_batchupdate_task,
    dfareporting_creative_assets_insert_builder, dfareporting_creative_assets_insert_task,
    dfareporting_creative_field_values_delete_builder, dfareporting_creative_field_values_delete_task,
    dfareporting_creative_field_values_insert_builder, dfareporting_creative_field_values_insert_task,
    dfareporting_creative_field_values_patch_builder, dfareporting_creative_field_values_patch_task,
    dfareporting_creative_field_values_update_builder, dfareporting_creative_field_values_update_task,
    dfareporting_creative_fields_delete_builder, dfareporting_creative_fields_delete_task,
    dfareporting_creative_fields_insert_builder, dfareporting_creative_fields_insert_task,
    dfareporting_creative_fields_patch_builder, dfareporting_creative_fields_patch_task,
    dfareporting_creative_fields_update_builder, dfareporting_creative_fields_update_task,
    dfareporting_creative_groups_insert_builder, dfareporting_creative_groups_insert_task,
    dfareporting_creative_groups_patch_builder, dfareporting_creative_groups_patch_task,
    dfareporting_creative_groups_update_builder, dfareporting_creative_groups_update_task,
    dfareporting_creatives_insert_builder, dfareporting_creatives_insert_task,
    dfareporting_creatives_patch_builder, dfareporting_creatives_patch_task,
    dfareporting_creatives_update_builder, dfareporting_creatives_update_task,
    dfareporting_dimension_values_query_builder, dfareporting_dimension_values_query_task,
    dfareporting_directory_sites_insert_builder, dfareporting_directory_sites_insert_task,
    dfareporting_dynamic_feeds_insert_builder, dfareporting_dynamic_feeds_insert_task,
    dfareporting_dynamic_feeds_retransform_builder, dfareporting_dynamic_feeds_retransform_task,
    dfareporting_dynamic_feeds_update_builder, dfareporting_dynamic_feeds_update_task,
    dfareporting_dynamic_profiles_insert_builder, dfareporting_dynamic_profiles_insert_task,
    dfareporting_dynamic_profiles_publish_builder, dfareporting_dynamic_profiles_publish_task,
    dfareporting_dynamic_profiles_update_builder, dfareporting_dynamic_profiles_update_task,
    dfareporting_dynamic_targeting_keys_delete_builder, dfareporting_dynamic_targeting_keys_delete_task,
    dfareporting_dynamic_targeting_keys_insert_builder, dfareporting_dynamic_targeting_keys_insert_task,
    dfareporting_event_tags_delete_builder, dfareporting_event_tags_delete_task,
    dfareporting_event_tags_insert_builder, dfareporting_event_tags_insert_task,
    dfareporting_event_tags_patch_builder, dfareporting_event_tags_patch_task,
    dfareporting_event_tags_update_builder, dfareporting_event_tags_update_task,
    dfareporting_floodlight_activities_delete_builder, dfareporting_floodlight_activities_delete_task,
    dfareporting_floodlight_activities_generatetag_builder, dfareporting_floodlight_activities_generatetag_task,
    dfareporting_floodlight_activities_insert_builder, dfareporting_floodlight_activities_insert_task,
    dfareporting_floodlight_activities_patch_builder, dfareporting_floodlight_activities_patch_task,
    dfareporting_floodlight_activities_update_builder, dfareporting_floodlight_activities_update_task,
    dfareporting_floodlight_activity_groups_insert_builder, dfareporting_floodlight_activity_groups_insert_task,
    dfareporting_floodlight_activity_groups_patch_builder, dfareporting_floodlight_activity_groups_patch_task,
    dfareporting_floodlight_activity_groups_update_builder, dfareporting_floodlight_activity_groups_update_task,
    dfareporting_floodlight_configurations_patch_builder, dfareporting_floodlight_configurations_patch_task,
    dfareporting_floodlight_configurations_update_builder, dfareporting_floodlight_configurations_update_task,
    dfareporting_placement_groups_insert_builder, dfareporting_placement_groups_insert_task,
    dfareporting_placement_groups_patch_builder, dfareporting_placement_groups_patch_task,
    dfareporting_placement_groups_update_builder, dfareporting_placement_groups_update_task,
    dfareporting_placement_strategies_delete_builder, dfareporting_placement_strategies_delete_task,
    dfareporting_placement_strategies_insert_builder, dfareporting_placement_strategies_insert_task,
    dfareporting_placement_strategies_patch_builder, dfareporting_placement_strategies_patch_task,
    dfareporting_placement_strategies_update_builder, dfareporting_placement_strategies_update_task,
    dfareporting_placements_generatetags_builder, dfareporting_placements_generatetags_task,
    dfareporting_placements_insert_builder, dfareporting_placements_insert_task,
    dfareporting_placements_patch_builder, dfareporting_placements_patch_task,
    dfareporting_placements_update_builder, dfareporting_placements_update_task,
    dfareporting_remarketing_list_shares_patch_builder, dfareporting_remarketing_list_shares_patch_task,
    dfareporting_remarketing_list_shares_update_builder, dfareporting_remarketing_list_shares_update_task,
    dfareporting_remarketing_lists_insert_builder, dfareporting_remarketing_lists_insert_task,
    dfareporting_remarketing_lists_patch_builder, dfareporting_remarketing_lists_patch_task,
    dfareporting_remarketing_lists_update_builder, dfareporting_remarketing_lists_update_task,
    dfareporting_reports_delete_builder, dfareporting_reports_delete_task,
    dfareporting_reports_insert_builder, dfareporting_reports_insert_task,
    dfareporting_reports_run_builder, dfareporting_reports_run_task,
    dfareporting_reports_update_builder, dfareporting_reports_update_task,
    dfareporting_reports_compatible_fields_query_builder, dfareporting_reports_compatible_fields_query_task,
    dfareporting_sites_insert_builder, dfareporting_sites_insert_task,
    dfareporting_sites_patch_builder, dfareporting_sites_patch_task,
    dfareporting_sites_update_builder, dfareporting_sites_update_task,
    dfareporting_sizes_insert_builder, dfareporting_sizes_insert_task,
    dfareporting_studio_creative_assets_insert_builder, dfareporting_studio_creative_assets_insert_task,
    dfareporting_studio_creatives_insert_builder, dfareporting_studio_creatives_insert_task,
    dfareporting_studio_creatives_publish_builder, dfareporting_studio_creatives_publish_task,
    dfareporting_subaccounts_insert_builder, dfareporting_subaccounts_insert_task,
    dfareporting_subaccounts_patch_builder, dfareporting_subaccounts_patch_task,
    dfareporting_subaccounts_update_builder, dfareporting_subaccounts_update_task,
    dfareporting_targeting_templates_insert_builder, dfareporting_targeting_templates_insert_task,
    dfareporting_targeting_templates_patch_builder, dfareporting_targeting_templates_patch_task,
    dfareporting_targeting_templates_update_builder, dfareporting_targeting_templates_update_task,
    dfareporting_user_roles_delete_builder, dfareporting_user_roles_delete_task,
    dfareporting_user_roles_insert_builder, dfareporting_user_roles_insert_task,
    dfareporting_user_roles_patch_builder, dfareporting_user_roles_patch_task,
    dfareporting_user_roles_update_builder, dfareporting_user_roles_update_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::dfareporting::Account;
use crate::providers::gcp::clients::dfareporting::AccountUserProfile;
use crate::providers::gcp::clients::dfareporting::Ad;
use crate::providers::gcp::clients::dfareporting::Advertiser;
use crate::providers::gcp::clients::dfareporting::AdvertiserGroup;
use crate::providers::gcp::clients::dfareporting::BillingAssignment;
use crate::providers::gcp::clients::dfareporting::BillingProfile;
use crate::providers::gcp::clients::dfareporting::Campaign;
use crate::providers::gcp::clients::dfareporting::CampaignCreativeAssociation;
use crate::providers::gcp::clients::dfareporting::CompatibleFields;
use crate::providers::gcp::clients::dfareporting::ContentCategory;
use crate::providers::gcp::clients::dfareporting::ConversionsBatchInsertResponse;
use crate::providers::gcp::clients::dfareporting::ConversionsBatchUpdateResponse;
use crate::providers::gcp::clients::dfareporting::Creative;
use crate::providers::gcp::clients::dfareporting::CreativeAssetMetadata;
use crate::providers::gcp::clients::dfareporting::CreativeField;
use crate::providers::gcp::clients::dfareporting::CreativeFieldValue;
use crate::providers::gcp::clients::dfareporting::CreativeGroup;
use crate::providers::gcp::clients::dfareporting::DimensionValueList;
use crate::providers::gcp::clients::dfareporting::DirectorySite;
use crate::providers::gcp::clients::dfareporting::DynamicFeed;
use crate::providers::gcp::clients::dfareporting::DynamicProfile;
use crate::providers::gcp::clients::dfareporting::DynamicTargetingKey;
use crate::providers::gcp::clients::dfareporting::EventTag;
use crate::providers::gcp::clients::dfareporting::File;
use crate::providers::gcp::clients::dfareporting::FloodlightActivitiesGenerateTagResponse;
use crate::providers::gcp::clients::dfareporting::FloodlightActivity;
use crate::providers::gcp::clients::dfareporting::FloodlightActivityGroup;
use crate::providers::gcp::clients::dfareporting::FloodlightConfiguration;
use crate::providers::gcp::clients::dfareporting::LandingPage;
use crate::providers::gcp::clients::dfareporting::Placement;
use crate::providers::gcp::clients::dfareporting::PlacementGroup;
use crate::providers::gcp::clients::dfareporting::PlacementStrategy;
use crate::providers::gcp::clients::dfareporting::PlacementsGenerateTagsResponse;
use crate::providers::gcp::clients::dfareporting::RemarketingList;
use crate::providers::gcp::clients::dfareporting::RemarketingListShare;
use crate::providers::gcp::clients::dfareporting::Report;
use crate::providers::gcp::clients::dfareporting::Site;
use crate::providers::gcp::clients::dfareporting::Size;
use crate::providers::gcp::clients::dfareporting::StudioCreative;
use crate::providers::gcp::clients::dfareporting::StudioCreativeAssetsResponse;
use crate::providers::gcp::clients::dfareporting::Subaccount;
use crate::providers::gcp::clients::dfareporting::TargetingTemplate;
use crate::providers::gcp::clients::dfareporting::UserRole;
use crate::providers::gcp::clients::dfareporting::DfareportingAccountUserProfilesInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAccountUserProfilesPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAccountUserProfilesUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAccountsPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAccountsUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAdsInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAdsPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAdsUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAdvertiserGroupsDeleteArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAdvertiserGroupsInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAdvertiserGroupsPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAdvertiserGroupsUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAdvertiserLandingPagesInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAdvertiserLandingPagesPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAdvertiserLandingPagesUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAdvertisersInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAdvertisersPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingAdvertisersUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingBillingAssignmentsInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingBillingProfilesUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCampaignCreativeAssociationsInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCampaignsInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCampaignsPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCampaignsUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingContentCategoriesDeleteArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingContentCategoriesInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingContentCategoriesPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingContentCategoriesUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingConversionsBatchinsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingConversionsBatchupdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCreativeAssetsInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCreativeFieldValuesDeleteArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCreativeFieldValuesInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCreativeFieldValuesPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCreativeFieldValuesUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCreativeFieldsDeleteArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCreativeFieldsInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCreativeFieldsPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCreativeFieldsUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCreativeGroupsInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCreativeGroupsPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCreativeGroupsUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCreativesInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCreativesPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingCreativesUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingDimensionValuesQueryArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingDirectorySitesInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingDynamicFeedsInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingDynamicFeedsRetransformArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingDynamicFeedsUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingDynamicProfilesInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingDynamicProfilesPublishArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingDynamicProfilesUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingDynamicTargetingKeysDeleteArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingDynamicTargetingKeysInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingEventTagsDeleteArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingEventTagsInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingEventTagsPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingEventTagsUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingFloodlightActivitiesDeleteArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingFloodlightActivitiesGeneratetagArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingFloodlightActivitiesInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingFloodlightActivitiesPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingFloodlightActivitiesUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingFloodlightActivityGroupsInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingFloodlightActivityGroupsPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingFloodlightActivityGroupsUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingFloodlightConfigurationsPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingFloodlightConfigurationsUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingPlacementGroupsInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingPlacementGroupsPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingPlacementGroupsUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingPlacementStrategiesDeleteArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingPlacementStrategiesInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingPlacementStrategiesPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingPlacementStrategiesUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingPlacementsGeneratetagsArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingPlacementsInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingPlacementsPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingPlacementsUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingRemarketingListSharesPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingRemarketingListSharesUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingRemarketingListsInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingRemarketingListsPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingRemarketingListsUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingReportsCompatibleFieldsQueryArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingReportsDeleteArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingReportsInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingReportsRunArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingReportsUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingSitesInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingSitesPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingSitesUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingSizesInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingStudioCreativeAssetsInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingStudioCreativesInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingStudioCreativesPublishArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingSubaccountsInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingSubaccountsPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingSubaccountsUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingTargetingTemplatesInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingTargetingTemplatesPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingTargetingTemplatesUpdateArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingUserRolesDeleteArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingUserRolesInsertArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingUserRolesPatchArgs;
use crate::providers::gcp::clients::dfareporting::DfareportingUserRolesUpdateArgs;
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
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
            &args.name,
            &args.objectType,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_dynamic_targeting_keys_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Dfareporting remarketing list shares patch.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
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
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_remarketing_list_shares_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting remarketing list shares update.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Dfareporting remarketing lists patch.
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
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_remarketing_lists_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting remarketing lists update.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Dfareporting targeting templates patch.
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
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = dfareporting_targeting_templates_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dfareporting targeting templates update.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

}
