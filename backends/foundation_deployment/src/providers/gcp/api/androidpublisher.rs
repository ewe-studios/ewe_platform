//! AndroidpublisherProvider - State-aware androidpublisher API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       androidpublisher API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::androidpublisher::{
    androidpublisher_applications_data_safety_builder, androidpublisher_applications_data_safety_task,
    androidpublisher_applications_device_tier_configs_create_builder, androidpublisher_applications_device_tier_configs_create_task,
    androidpublisher_applications_device_tier_configs_get_builder, androidpublisher_applications_device_tier_configs_get_task,
    androidpublisher_applications_device_tier_configs_list_builder, androidpublisher_applications_device_tier_configs_list_task,
    androidpublisher_applications_tracks_releases_list_builder, androidpublisher_applications_tracks_releases_list_task,
    androidpublisher_apprecovery_add_targeting_builder, androidpublisher_apprecovery_add_targeting_task,
    androidpublisher_apprecovery_cancel_builder, androidpublisher_apprecovery_cancel_task,
    androidpublisher_apprecovery_create_builder, androidpublisher_apprecovery_create_task,
    androidpublisher_apprecovery_deploy_builder, androidpublisher_apprecovery_deploy_task,
    androidpublisher_apprecovery_list_builder, androidpublisher_apprecovery_list_task,
    androidpublisher_edits_commit_builder, androidpublisher_edits_commit_task,
    androidpublisher_edits_delete_builder, androidpublisher_edits_delete_task,
    androidpublisher_edits_get_builder, androidpublisher_edits_get_task,
    androidpublisher_edits_insert_builder, androidpublisher_edits_insert_task,
    androidpublisher_edits_validate_builder, androidpublisher_edits_validate_task,
    androidpublisher_edits_apks_addexternallyhosted_builder, androidpublisher_edits_apks_addexternallyhosted_task,
    androidpublisher_edits_apks_list_builder, androidpublisher_edits_apks_list_task,
    androidpublisher_edits_apks_upload_builder, androidpublisher_edits_apks_upload_task,
    androidpublisher_edits_bundles_list_builder, androidpublisher_edits_bundles_list_task,
    androidpublisher_edits_bundles_upload_builder, androidpublisher_edits_bundles_upload_task,
    androidpublisher_edits_countryavailability_get_builder, androidpublisher_edits_countryavailability_get_task,
    androidpublisher_edits_deobfuscationfiles_upload_builder, androidpublisher_edits_deobfuscationfiles_upload_task,
    androidpublisher_edits_details_get_builder, androidpublisher_edits_details_get_task,
    androidpublisher_edits_details_patch_builder, androidpublisher_edits_details_patch_task,
    androidpublisher_edits_details_update_builder, androidpublisher_edits_details_update_task,
    androidpublisher_edits_expansionfiles_get_builder, androidpublisher_edits_expansionfiles_get_task,
    androidpublisher_edits_expansionfiles_patch_builder, androidpublisher_edits_expansionfiles_patch_task,
    androidpublisher_edits_expansionfiles_update_builder, androidpublisher_edits_expansionfiles_update_task,
    androidpublisher_edits_expansionfiles_upload_builder, androidpublisher_edits_expansionfiles_upload_task,
    androidpublisher_edits_images_delete_builder, androidpublisher_edits_images_delete_task,
    androidpublisher_edits_images_deleteall_builder, androidpublisher_edits_images_deleteall_task,
    androidpublisher_edits_images_list_builder, androidpublisher_edits_images_list_task,
    androidpublisher_edits_images_upload_builder, androidpublisher_edits_images_upload_task,
    androidpublisher_edits_listings_delete_builder, androidpublisher_edits_listings_delete_task,
    androidpublisher_edits_listings_deleteall_builder, androidpublisher_edits_listings_deleteall_task,
    androidpublisher_edits_listings_get_builder, androidpublisher_edits_listings_get_task,
    androidpublisher_edits_listings_list_builder, androidpublisher_edits_listings_list_task,
    androidpublisher_edits_listings_patch_builder, androidpublisher_edits_listings_patch_task,
    androidpublisher_edits_listings_update_builder, androidpublisher_edits_listings_update_task,
    androidpublisher_edits_testers_get_builder, androidpublisher_edits_testers_get_task,
    androidpublisher_edits_testers_patch_builder, androidpublisher_edits_testers_patch_task,
    androidpublisher_edits_testers_update_builder, androidpublisher_edits_testers_update_task,
    androidpublisher_edits_tracks_create_builder, androidpublisher_edits_tracks_create_task,
    androidpublisher_edits_tracks_get_builder, androidpublisher_edits_tracks_get_task,
    androidpublisher_edits_tracks_list_builder, androidpublisher_edits_tracks_list_task,
    androidpublisher_edits_tracks_patch_builder, androidpublisher_edits_tracks_patch_task,
    androidpublisher_edits_tracks_update_builder, androidpublisher_edits_tracks_update_task,
    androidpublisher_externaltransactions_createexternaltransaction_builder, androidpublisher_externaltransactions_createexternaltransaction_task,
    androidpublisher_externaltransactions_getexternaltransaction_builder, androidpublisher_externaltransactions_getexternaltransaction_task,
    androidpublisher_externaltransactions_refundexternaltransaction_builder, androidpublisher_externaltransactions_refundexternaltransaction_task,
    androidpublisher_generatedapks_download_builder, androidpublisher_generatedapks_download_task,
    androidpublisher_generatedapks_list_builder, androidpublisher_generatedapks_list_task,
    androidpublisher_grants_create_builder, androidpublisher_grants_create_task,
    androidpublisher_grants_delete_builder, androidpublisher_grants_delete_task,
    androidpublisher_grants_patch_builder, androidpublisher_grants_patch_task,
    androidpublisher_inappproducts_batch_delete_builder, androidpublisher_inappproducts_batch_delete_task,
    androidpublisher_inappproducts_batch_get_builder, androidpublisher_inappproducts_batch_get_task,
    androidpublisher_inappproducts_batch_update_builder, androidpublisher_inappproducts_batch_update_task,
    androidpublisher_inappproducts_delete_builder, androidpublisher_inappproducts_delete_task,
    androidpublisher_inappproducts_get_builder, androidpublisher_inappproducts_get_task,
    androidpublisher_inappproducts_insert_builder, androidpublisher_inappproducts_insert_task,
    androidpublisher_inappproducts_list_builder, androidpublisher_inappproducts_list_task,
    androidpublisher_inappproducts_patch_builder, androidpublisher_inappproducts_patch_task,
    androidpublisher_inappproducts_update_builder, androidpublisher_inappproducts_update_task,
    androidpublisher_internalappsharingartifacts_uploadapk_builder, androidpublisher_internalappsharingartifacts_uploadapk_task,
    androidpublisher_internalappsharingartifacts_uploadbundle_builder, androidpublisher_internalappsharingartifacts_uploadbundle_task,
    androidpublisher_monetization_convert_region_prices_builder, androidpublisher_monetization_convert_region_prices_task,
    androidpublisher_monetization_onetimeproducts_batch_delete_builder, androidpublisher_monetization_onetimeproducts_batch_delete_task,
    androidpublisher_monetization_onetimeproducts_batch_get_builder, androidpublisher_monetization_onetimeproducts_batch_get_task,
    androidpublisher_monetization_onetimeproducts_batch_update_builder, androidpublisher_monetization_onetimeproducts_batch_update_task,
    androidpublisher_monetization_onetimeproducts_delete_builder, androidpublisher_monetization_onetimeproducts_delete_task,
    androidpublisher_monetization_onetimeproducts_get_builder, androidpublisher_monetization_onetimeproducts_get_task,
    androidpublisher_monetization_onetimeproducts_list_builder, androidpublisher_monetization_onetimeproducts_list_task,
    androidpublisher_monetization_onetimeproducts_patch_builder, androidpublisher_monetization_onetimeproducts_patch_task,
    androidpublisher_monetization_onetimeproducts_purchase_options_batch_delete_builder, androidpublisher_monetization_onetimeproducts_purchase_options_batch_delete_task,
    androidpublisher_monetization_onetimeproducts_purchase_options_batch_update_states_builder, androidpublisher_monetization_onetimeproducts_purchase_options_batch_update_states_task,
    androidpublisher_monetization_onetimeproducts_purchase_options_offers_activate_builder, androidpublisher_monetization_onetimeproducts_purchase_options_offers_activate_task,
    androidpublisher_monetization_onetimeproducts_purchase_options_offers_batch_delete_builder, androidpublisher_monetization_onetimeproducts_purchase_options_offers_batch_delete_task,
    androidpublisher_monetization_onetimeproducts_purchase_options_offers_batch_get_builder, androidpublisher_monetization_onetimeproducts_purchase_options_offers_batch_get_task,
    androidpublisher_monetization_onetimeproducts_purchase_options_offers_batch_update_builder, androidpublisher_monetization_onetimeproducts_purchase_options_offers_batch_update_task,
    androidpublisher_monetization_onetimeproducts_purchase_options_offers_batch_update_states_builder, androidpublisher_monetization_onetimeproducts_purchase_options_offers_batch_update_states_task,
    androidpublisher_monetization_onetimeproducts_purchase_options_offers_cancel_builder, androidpublisher_monetization_onetimeproducts_purchase_options_offers_cancel_task,
    androidpublisher_monetization_onetimeproducts_purchase_options_offers_deactivate_builder, androidpublisher_monetization_onetimeproducts_purchase_options_offers_deactivate_task,
    androidpublisher_monetization_onetimeproducts_purchase_options_offers_list_builder, androidpublisher_monetization_onetimeproducts_purchase_options_offers_list_task,
    androidpublisher_monetization_subscriptions_archive_builder, androidpublisher_monetization_subscriptions_archive_task,
    androidpublisher_monetization_subscriptions_batch_get_builder, androidpublisher_monetization_subscriptions_batch_get_task,
    androidpublisher_monetization_subscriptions_batch_update_builder, androidpublisher_monetization_subscriptions_batch_update_task,
    androidpublisher_monetization_subscriptions_create_builder, androidpublisher_monetization_subscriptions_create_task,
    androidpublisher_monetization_subscriptions_delete_builder, androidpublisher_monetization_subscriptions_delete_task,
    androidpublisher_monetization_subscriptions_get_builder, androidpublisher_monetization_subscriptions_get_task,
    androidpublisher_monetization_subscriptions_list_builder, androidpublisher_monetization_subscriptions_list_task,
    androidpublisher_monetization_subscriptions_patch_builder, androidpublisher_monetization_subscriptions_patch_task,
    androidpublisher_monetization_subscriptions_base_plans_activate_builder, androidpublisher_monetization_subscriptions_base_plans_activate_task,
    androidpublisher_monetization_subscriptions_base_plans_batch_migrate_prices_builder, androidpublisher_monetization_subscriptions_base_plans_batch_migrate_prices_task,
    androidpublisher_monetization_subscriptions_base_plans_batch_update_states_builder, androidpublisher_monetization_subscriptions_base_plans_batch_update_states_task,
    androidpublisher_monetization_subscriptions_base_plans_deactivate_builder, androidpublisher_monetization_subscriptions_base_plans_deactivate_task,
    androidpublisher_monetization_subscriptions_base_plans_delete_builder, androidpublisher_monetization_subscriptions_base_plans_delete_task,
    androidpublisher_monetization_subscriptions_base_plans_migrate_prices_builder, androidpublisher_monetization_subscriptions_base_plans_migrate_prices_task,
    androidpublisher_monetization_subscriptions_base_plans_offers_activate_builder, androidpublisher_monetization_subscriptions_base_plans_offers_activate_task,
    androidpublisher_monetization_subscriptions_base_plans_offers_batch_get_builder, androidpublisher_monetization_subscriptions_base_plans_offers_batch_get_task,
    androidpublisher_monetization_subscriptions_base_plans_offers_batch_update_builder, androidpublisher_monetization_subscriptions_base_plans_offers_batch_update_task,
    androidpublisher_monetization_subscriptions_base_plans_offers_batch_update_states_builder, androidpublisher_monetization_subscriptions_base_plans_offers_batch_update_states_task,
    androidpublisher_monetization_subscriptions_base_plans_offers_create_builder, androidpublisher_monetization_subscriptions_base_plans_offers_create_task,
    androidpublisher_monetization_subscriptions_base_plans_offers_deactivate_builder, androidpublisher_monetization_subscriptions_base_plans_offers_deactivate_task,
    androidpublisher_monetization_subscriptions_base_plans_offers_delete_builder, androidpublisher_monetization_subscriptions_base_plans_offers_delete_task,
    androidpublisher_monetization_subscriptions_base_plans_offers_get_builder, androidpublisher_monetization_subscriptions_base_plans_offers_get_task,
    androidpublisher_monetization_subscriptions_base_plans_offers_list_builder, androidpublisher_monetization_subscriptions_base_plans_offers_list_task,
    androidpublisher_monetization_subscriptions_base_plans_offers_patch_builder, androidpublisher_monetization_subscriptions_base_plans_offers_patch_task,
    androidpublisher_orders_batchget_builder, androidpublisher_orders_batchget_task,
    androidpublisher_orders_get_builder, androidpublisher_orders_get_task,
    androidpublisher_orders_refund_builder, androidpublisher_orders_refund_task,
    androidpublisher_purchases_products_acknowledge_builder, androidpublisher_purchases_products_acknowledge_task,
    androidpublisher_purchases_products_consume_builder, androidpublisher_purchases_products_consume_task,
    androidpublisher_purchases_products_get_builder, androidpublisher_purchases_products_get_task,
    androidpublisher_purchases_productsv2_getproductpurchasev2_builder, androidpublisher_purchases_productsv2_getproductpurchasev2_task,
    androidpublisher_purchases_subscriptions_acknowledge_builder, androidpublisher_purchases_subscriptions_acknowledge_task,
    androidpublisher_purchases_subscriptions_cancel_builder, androidpublisher_purchases_subscriptions_cancel_task,
    androidpublisher_purchases_subscriptions_defer_builder, androidpublisher_purchases_subscriptions_defer_task,
    androidpublisher_purchases_subscriptions_get_builder, androidpublisher_purchases_subscriptions_get_task,
    androidpublisher_purchases_subscriptions_refund_builder, androidpublisher_purchases_subscriptions_refund_task,
    androidpublisher_purchases_subscriptions_revoke_builder, androidpublisher_purchases_subscriptions_revoke_task,
    androidpublisher_purchases_subscriptionsv2_cancel_builder, androidpublisher_purchases_subscriptionsv2_cancel_task,
    androidpublisher_purchases_subscriptionsv2_defer_builder, androidpublisher_purchases_subscriptionsv2_defer_task,
    androidpublisher_purchases_subscriptionsv2_get_builder, androidpublisher_purchases_subscriptionsv2_get_task,
    androidpublisher_purchases_subscriptionsv2_revoke_builder, androidpublisher_purchases_subscriptionsv2_revoke_task,
    androidpublisher_purchases_voidedpurchases_list_builder, androidpublisher_purchases_voidedpurchases_list_task,
    androidpublisher_reviews_get_builder, androidpublisher_reviews_get_task,
    androidpublisher_reviews_list_builder, androidpublisher_reviews_list_task,
    androidpublisher_reviews_reply_builder, androidpublisher_reviews_reply_task,
    androidpublisher_systemapks_variants_create_builder, androidpublisher_systemapks_variants_create_task,
    androidpublisher_systemapks_variants_download_builder, androidpublisher_systemapks_variants_download_task,
    androidpublisher_systemapks_variants_get_builder, androidpublisher_systemapks_variants_get_task,
    androidpublisher_systemapks_variants_list_builder, androidpublisher_systemapks_variants_list_task,
    androidpublisher_users_create_builder, androidpublisher_users_create_task,
    androidpublisher_users_delete_builder, androidpublisher_users_delete_task,
    androidpublisher_users_list_builder, androidpublisher_users_list_task,
    androidpublisher_users_patch_builder, androidpublisher_users_patch_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::androidpublisher::AddTargetingResponse;
use crate::providers::gcp::clients::androidpublisher::Apk;
use crate::providers::gcp::clients::androidpublisher::ApksAddExternallyHostedResponse;
use crate::providers::gcp::clients::androidpublisher::ApksListResponse;
use crate::providers::gcp::clients::androidpublisher::AppDetails;
use crate::providers::gcp::clients::androidpublisher::AppEdit;
use crate::providers::gcp::clients::androidpublisher::AppRecoveryAction;
use crate::providers::gcp::clients::androidpublisher::BatchGetOneTimeProductOffersResponse;
use crate::providers::gcp::clients::androidpublisher::BatchGetOneTimeProductsResponse;
use crate::providers::gcp::clients::androidpublisher::BatchGetOrdersResponse;
use crate::providers::gcp::clients::androidpublisher::BatchGetSubscriptionOffersResponse;
use crate::providers::gcp::clients::androidpublisher::BatchGetSubscriptionsResponse;
use crate::providers::gcp::clients::androidpublisher::BatchMigrateBasePlanPricesResponse;
use crate::providers::gcp::clients::androidpublisher::BatchUpdateBasePlanStatesResponse;
use crate::providers::gcp::clients::androidpublisher::BatchUpdateOneTimeProductOfferStatesResponse;
use crate::providers::gcp::clients::androidpublisher::BatchUpdateOneTimeProductOffersResponse;
use crate::providers::gcp::clients::androidpublisher::BatchUpdateOneTimeProductsResponse;
use crate::providers::gcp::clients::androidpublisher::BatchUpdatePurchaseOptionStatesResponse;
use crate::providers::gcp::clients::androidpublisher::BatchUpdateSubscriptionOfferStatesResponse;
use crate::providers::gcp::clients::androidpublisher::BatchUpdateSubscriptionOffersResponse;
use crate::providers::gcp::clients::androidpublisher::BatchUpdateSubscriptionsResponse;
use crate::providers::gcp::clients::androidpublisher::Bundle;
use crate::providers::gcp::clients::androidpublisher::BundlesListResponse;
use crate::providers::gcp::clients::androidpublisher::CancelAppRecoveryResponse;
use crate::providers::gcp::clients::androidpublisher::CancelSubscriptionPurchaseResponse;
use crate::providers::gcp::clients::androidpublisher::ConvertRegionPricesResponse;
use crate::providers::gcp::clients::androidpublisher::DeferSubscriptionPurchaseResponse;
use crate::providers::gcp::clients::androidpublisher::DeobfuscationFilesUploadResponse;
use crate::providers::gcp::clients::androidpublisher::DeployAppRecoveryResponse;
use crate::providers::gcp::clients::androidpublisher::DeviceTierConfig;
use crate::providers::gcp::clients::androidpublisher::ExpansionFile;
use crate::providers::gcp::clients::androidpublisher::ExpansionFilesUploadResponse;
use crate::providers::gcp::clients::androidpublisher::ExternalTransaction;
use crate::providers::gcp::clients::androidpublisher::GeneratedApksListResponse;
use crate::providers::gcp::clients::androidpublisher::Grant;
use crate::providers::gcp::clients::androidpublisher::ImagesDeleteAllResponse;
use crate::providers::gcp::clients::androidpublisher::ImagesListResponse;
use crate::providers::gcp::clients::androidpublisher::ImagesUploadResponse;
use crate::providers::gcp::clients::androidpublisher::InAppProduct;
use crate::providers::gcp::clients::androidpublisher::InappproductsBatchGetResponse;
use crate::providers::gcp::clients::androidpublisher::InappproductsBatchUpdateResponse;
use crate::providers::gcp::clients::androidpublisher::InappproductsListResponse;
use crate::providers::gcp::clients::androidpublisher::InternalAppSharingArtifact;
use crate::providers::gcp::clients::androidpublisher::ListAppRecoveriesResponse;
use crate::providers::gcp::clients::androidpublisher::ListDeviceTierConfigsResponse;
use crate::providers::gcp::clients::androidpublisher::ListOneTimeProductOffersResponse;
use crate::providers::gcp::clients::androidpublisher::ListOneTimeProductsResponse;
use crate::providers::gcp::clients::androidpublisher::ListReleaseSummariesResponse;
use crate::providers::gcp::clients::androidpublisher::ListSubscriptionOffersResponse;
use crate::providers::gcp::clients::androidpublisher::ListSubscriptionsResponse;
use crate::providers::gcp::clients::androidpublisher::ListUsersResponse;
use crate::providers::gcp::clients::androidpublisher::Listing;
use crate::providers::gcp::clients::androidpublisher::ListingsListResponse;
use crate::providers::gcp::clients::androidpublisher::MigrateBasePlanPricesResponse;
use crate::providers::gcp::clients::androidpublisher::OneTimeProduct;
use crate::providers::gcp::clients::androidpublisher::OneTimeProductOffer;
use crate::providers::gcp::clients::androidpublisher::Order;
use crate::providers::gcp::clients::androidpublisher::ProductPurchase;
use crate::providers::gcp::clients::androidpublisher::ProductPurchaseV2;
use crate::providers::gcp::clients::androidpublisher::Review;
use crate::providers::gcp::clients::androidpublisher::ReviewsListResponse;
use crate::providers::gcp::clients::androidpublisher::ReviewsReplyResponse;
use crate::providers::gcp::clients::androidpublisher::RevokeSubscriptionPurchaseResponse;
use crate::providers::gcp::clients::androidpublisher::SafetyLabelsUpdateResponse;
use crate::providers::gcp::clients::androidpublisher::Subscription;
use crate::providers::gcp::clients::androidpublisher::SubscriptionOffer;
use crate::providers::gcp::clients::androidpublisher::SubscriptionPurchase;
use crate::providers::gcp::clients::androidpublisher::SubscriptionPurchaseV2;
use crate::providers::gcp::clients::androidpublisher::SubscriptionPurchasesDeferResponse;
use crate::providers::gcp::clients::androidpublisher::SystemApksListResponse;
use crate::providers::gcp::clients::androidpublisher::Testers;
use crate::providers::gcp::clients::androidpublisher::Track;
use crate::providers::gcp::clients::androidpublisher::TrackCountryAvailability;
use crate::providers::gcp::clients::androidpublisher::TracksListResponse;
use crate::providers::gcp::clients::androidpublisher::User;
use crate::providers::gcp::clients::androidpublisher::Variant;
use crate::providers::gcp::clients::androidpublisher::VoidedPurchasesListResponse;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherApplicationsDataSafetyArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherApplicationsDeviceTierConfigsCreateArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherApplicationsDeviceTierConfigsGetArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherApplicationsDeviceTierConfigsListArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherApplicationsTracksReleasesListArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherApprecoveryAddTargetingArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherApprecoveryCancelArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherApprecoveryCreateArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherApprecoveryDeployArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherApprecoveryListArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherEditsApksAddexternallyhostedArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherEditsApksListArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherEditsApksUploadArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherEditsBundlesListArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherEditsBundlesUploadArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherEditsCommitArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherEditsCountryavailabilityGetArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherEditsDeleteArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherEditsDeobfuscationfilesUploadArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherEditsDetailsGetArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherEditsDetailsPatchArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherEditsDetailsUpdateArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherEditsExpansionfilesGetArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherEditsExpansionfilesPatchArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherEditsExpansionfilesUpdateArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherEditsExpansionfilesUploadArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherEditsGetArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherEditsImagesDeleteArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherEditsImagesDeleteallArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherEditsImagesListArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherEditsImagesUploadArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherEditsInsertArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherEditsListingsDeleteArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherEditsListingsDeleteallArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherEditsListingsGetArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherEditsListingsListArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherEditsListingsPatchArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherEditsListingsUpdateArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherEditsTestersGetArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherEditsTestersPatchArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherEditsTestersUpdateArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherEditsTracksCreateArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherEditsTracksGetArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherEditsTracksListArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherEditsTracksPatchArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherEditsTracksUpdateArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherEditsValidateArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherExternaltransactionsCreateexternaltransactionArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherExternaltransactionsGetexternaltransactionArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherExternaltransactionsRefundexternaltransactionArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherGeneratedapksDownloadArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherGeneratedapksListArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherGrantsCreateArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherGrantsDeleteArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherGrantsPatchArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherInappproductsBatchDeleteArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherInappproductsBatchGetArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherInappproductsBatchUpdateArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherInappproductsDeleteArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherInappproductsGetArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherInappproductsInsertArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherInappproductsListArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherInappproductsPatchArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherInappproductsUpdateArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherInternalappsharingartifactsUploadapkArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherInternalappsharingartifactsUploadbundleArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherMonetizationConvertRegionPricesArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherMonetizationOnetimeproductsBatchDeleteArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherMonetizationOnetimeproductsBatchGetArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherMonetizationOnetimeproductsBatchUpdateArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherMonetizationOnetimeproductsDeleteArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherMonetizationOnetimeproductsGetArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherMonetizationOnetimeproductsListArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherMonetizationOnetimeproductsPatchArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherMonetizationOnetimeproductsPurchaseOptionsBatchDeleteArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherMonetizationOnetimeproductsPurchaseOptionsBatchUpdateStatesArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherMonetizationOnetimeproductsPurchaseOptionsOffersActivateArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherMonetizationOnetimeproductsPurchaseOptionsOffersBatchDeleteArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherMonetizationOnetimeproductsPurchaseOptionsOffersBatchGetArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherMonetizationOnetimeproductsPurchaseOptionsOffersBatchUpdateArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherMonetizationOnetimeproductsPurchaseOptionsOffersBatchUpdateStatesArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherMonetizationOnetimeproductsPurchaseOptionsOffersCancelArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherMonetizationOnetimeproductsPurchaseOptionsOffersDeactivateArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherMonetizationOnetimeproductsPurchaseOptionsOffersListArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherMonetizationSubscriptionsArchiveArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherMonetizationSubscriptionsBasePlansActivateArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherMonetizationSubscriptionsBasePlansBatchMigratePricesArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherMonetizationSubscriptionsBasePlansBatchUpdateStatesArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherMonetizationSubscriptionsBasePlansDeactivateArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherMonetizationSubscriptionsBasePlansDeleteArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherMonetizationSubscriptionsBasePlansMigratePricesArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherMonetizationSubscriptionsBasePlansOffersActivateArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherMonetizationSubscriptionsBasePlansOffersBatchGetArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherMonetizationSubscriptionsBasePlansOffersBatchUpdateArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherMonetizationSubscriptionsBasePlansOffersBatchUpdateStatesArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherMonetizationSubscriptionsBasePlansOffersCreateArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherMonetizationSubscriptionsBasePlansOffersDeactivateArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherMonetizationSubscriptionsBasePlansOffersDeleteArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherMonetizationSubscriptionsBasePlansOffersGetArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherMonetizationSubscriptionsBasePlansOffersListArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherMonetizationSubscriptionsBasePlansOffersPatchArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherMonetizationSubscriptionsBatchGetArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherMonetizationSubscriptionsBatchUpdateArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherMonetizationSubscriptionsCreateArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherMonetizationSubscriptionsDeleteArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherMonetizationSubscriptionsGetArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherMonetizationSubscriptionsListArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherMonetizationSubscriptionsPatchArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherOrdersBatchgetArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherOrdersGetArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherOrdersRefundArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherPurchasesProductsAcknowledgeArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherPurchasesProductsConsumeArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherPurchasesProductsGetArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherPurchasesProductsv2Getproductpurchasev2Args;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherPurchasesSubscriptionsAcknowledgeArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherPurchasesSubscriptionsCancelArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherPurchasesSubscriptionsDeferArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherPurchasesSubscriptionsGetArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherPurchasesSubscriptionsRefundArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherPurchasesSubscriptionsRevokeArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherPurchasesSubscriptionsv2CancelArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherPurchasesSubscriptionsv2DeferArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherPurchasesSubscriptionsv2GetArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherPurchasesSubscriptionsv2RevokeArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherPurchasesVoidedpurchasesListArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherReviewsGetArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherReviewsListArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherReviewsReplyArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherSystemapksVariantsCreateArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherSystemapksVariantsDownloadArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherSystemapksVariantsGetArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherSystemapksVariantsListArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherUsersCreateArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherUsersDeleteArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherUsersListArgs;
use crate::providers::gcp::clients::androidpublisher::AndroidpublisherUsersPatchArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// AndroidpublisherProvider with automatic state tracking.
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
/// let provider = AndroidpublisherProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct AndroidpublisherProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> AndroidpublisherProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new AndroidpublisherProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Androidpublisher applications data safety.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SafetyLabelsUpdateResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_applications_data_safety(
        &self,
        args: &AndroidpublisherApplicationsDataSafetyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SafetyLabelsUpdateResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_applications_data_safety_builder(
            &self.http_client,
            &args.packageName,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_applications_data_safety_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher applications device tier configs create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeviceTierConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_applications_device_tier_configs_create(
        &self,
        args: &AndroidpublisherApplicationsDeviceTierConfigsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeviceTierConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_applications_device_tier_configs_create_builder(
            &self.http_client,
            &args.packageName,
            &args.allowUnknownDevices,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_applications_device_tier_configs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher applications device tier configs get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeviceTierConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidpublisher_applications_device_tier_configs_get(
        &self,
        args: &AndroidpublisherApplicationsDeviceTierConfigsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeviceTierConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_applications_device_tier_configs_get_builder(
            &self.http_client,
            &args.packageName,
            &args.deviceTierConfigId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_applications_device_tier_configs_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher applications device tier configs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDeviceTierConfigsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidpublisher_applications_device_tier_configs_list(
        &self,
        args: &AndroidpublisherApplicationsDeviceTierConfigsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDeviceTierConfigsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_applications_device_tier_configs_list_builder(
            &self.http_client,
            &args.packageName,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_applications_device_tier_configs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher applications tracks releases list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListReleaseSummariesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidpublisher_applications_tracks_releases_list(
        &self,
        args: &AndroidpublisherApplicationsTracksReleasesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListReleaseSummariesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_applications_tracks_releases_list_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_applications_tracks_releases_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher apprecovery add targeting.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AddTargetingResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_apprecovery_add_targeting(
        &self,
        args: &AndroidpublisherApprecoveryAddTargetingArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AddTargetingResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_apprecovery_add_targeting_builder(
            &self.http_client,
            &args.packageName,
            &args.appRecoveryId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_apprecovery_add_targeting_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher apprecovery cancel.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CancelAppRecoveryResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_apprecovery_cancel(
        &self,
        args: &AndroidpublisherApprecoveryCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CancelAppRecoveryResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_apprecovery_cancel_builder(
            &self.http_client,
            &args.packageName,
            &args.appRecoveryId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_apprecovery_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher apprecovery create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AppRecoveryAction result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_apprecovery_create(
        &self,
        args: &AndroidpublisherApprecoveryCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AppRecoveryAction, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_apprecovery_create_builder(
            &self.http_client,
            &args.packageName,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_apprecovery_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher apprecovery deploy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeployAppRecoveryResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_apprecovery_deploy(
        &self,
        args: &AndroidpublisherApprecoveryDeployArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeployAppRecoveryResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_apprecovery_deploy_builder(
            &self.http_client,
            &args.packageName,
            &args.appRecoveryId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_apprecovery_deploy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher apprecovery list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAppRecoveriesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidpublisher_apprecovery_list(
        &self,
        args: &AndroidpublisherApprecoveryListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAppRecoveriesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_apprecovery_list_builder(
            &self.http_client,
            &args.packageName,
            &args.versionCode,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_apprecovery_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher edits commit.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AppEdit result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_edits_commit(
        &self,
        args: &AndroidpublisherEditsCommitArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AppEdit, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_edits_commit_builder(
            &self.http_client,
            &args.packageName,
            &args.editId,
            &args.changesInReviewBehavior,
            &args.changesNotSentForReview,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_edits_commit_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher edits delete.
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
    pub fn androidpublisher_edits_delete(
        &self,
        args: &AndroidpublisherEditsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_edits_delete_builder(
            &self.http_client,
            &args.packageName,
            &args.editId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_edits_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher edits get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AppEdit result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidpublisher_edits_get(
        &self,
        args: &AndroidpublisherEditsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AppEdit, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_edits_get_builder(
            &self.http_client,
            &args.packageName,
            &args.editId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_edits_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher edits insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AppEdit result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_edits_insert(
        &self,
        args: &AndroidpublisherEditsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AppEdit, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_edits_insert_builder(
            &self.http_client,
            &args.packageName,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_edits_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher edits validate.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AppEdit result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidpublisher_edits_validate(
        &self,
        args: &AndroidpublisherEditsValidateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AppEdit, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_edits_validate_builder(
            &self.http_client,
            &args.packageName,
            &args.editId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_edits_validate_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher edits apks addexternallyhosted.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApksAddExternallyHostedResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_edits_apks_addexternallyhosted(
        &self,
        args: &AndroidpublisherEditsApksAddexternallyhostedArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApksAddExternallyHostedResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_edits_apks_addexternallyhosted_builder(
            &self.http_client,
            &args.packageName,
            &args.editId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_edits_apks_addexternallyhosted_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher edits apks list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApksListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidpublisher_edits_apks_list(
        &self,
        args: &AndroidpublisherEditsApksListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApksListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_edits_apks_list_builder(
            &self.http_client,
            &args.packageName,
            &args.editId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_edits_apks_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher edits apks upload.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Apk result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_edits_apks_upload(
        &self,
        args: &AndroidpublisherEditsApksUploadArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Apk, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_edits_apks_upload_builder(
            &self.http_client,
            &args.packageName,
            &args.editId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_edits_apks_upload_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher edits bundles list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BundlesListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidpublisher_edits_bundles_list(
        &self,
        args: &AndroidpublisherEditsBundlesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BundlesListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_edits_bundles_list_builder(
            &self.http_client,
            &args.packageName,
            &args.editId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_edits_bundles_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher edits bundles upload.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Bundle result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_edits_bundles_upload(
        &self,
        args: &AndroidpublisherEditsBundlesUploadArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Bundle, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_edits_bundles_upload_builder(
            &self.http_client,
            &args.packageName,
            &args.editId,
            &args.ackBundleInstallationWarning,
            &args.deviceTierConfigId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_edits_bundles_upload_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher edits countryavailability get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TrackCountryAvailability result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidpublisher_edits_countryavailability_get(
        &self,
        args: &AndroidpublisherEditsCountryavailabilityGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TrackCountryAvailability, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_edits_countryavailability_get_builder(
            &self.http_client,
            &args.packageName,
            &args.editId,
            &args.track,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_edits_countryavailability_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher edits deobfuscationfiles upload.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeobfuscationFilesUploadResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_edits_deobfuscationfiles_upload(
        &self,
        args: &AndroidpublisherEditsDeobfuscationfilesUploadArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeobfuscationFilesUploadResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_edits_deobfuscationfiles_upload_builder(
            &self.http_client,
            &args.packageName,
            &args.editId,
            &args.apkVersionCode,
            &args.deobfuscationFileType,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_edits_deobfuscationfiles_upload_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher edits details get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AppDetails result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidpublisher_edits_details_get(
        &self,
        args: &AndroidpublisherEditsDetailsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AppDetails, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_edits_details_get_builder(
            &self.http_client,
            &args.packageName,
            &args.editId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_edits_details_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher edits details patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AppDetails result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_edits_details_patch(
        &self,
        args: &AndroidpublisherEditsDetailsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AppDetails, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_edits_details_patch_builder(
            &self.http_client,
            &args.packageName,
            &args.editId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_edits_details_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher edits details update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AppDetails result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_edits_details_update(
        &self,
        args: &AndroidpublisherEditsDetailsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AppDetails, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_edits_details_update_builder(
            &self.http_client,
            &args.packageName,
            &args.editId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_edits_details_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher edits expansionfiles get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ExpansionFile result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidpublisher_edits_expansionfiles_get(
        &self,
        args: &AndroidpublisherEditsExpansionfilesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ExpansionFile, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_edits_expansionfiles_get_builder(
            &self.http_client,
            &args.packageName,
            &args.editId,
            &args.apkVersionCode,
            &args.expansionFileType,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_edits_expansionfiles_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher edits expansionfiles patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ExpansionFile result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_edits_expansionfiles_patch(
        &self,
        args: &AndroidpublisherEditsExpansionfilesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ExpansionFile, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_edits_expansionfiles_patch_builder(
            &self.http_client,
            &args.packageName,
            &args.editId,
            &args.apkVersionCode,
            &args.expansionFileType,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_edits_expansionfiles_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher edits expansionfiles update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ExpansionFile result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_edits_expansionfiles_update(
        &self,
        args: &AndroidpublisherEditsExpansionfilesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ExpansionFile, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_edits_expansionfiles_update_builder(
            &self.http_client,
            &args.packageName,
            &args.editId,
            &args.apkVersionCode,
            &args.expansionFileType,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_edits_expansionfiles_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher edits expansionfiles upload.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ExpansionFilesUploadResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_edits_expansionfiles_upload(
        &self,
        args: &AndroidpublisherEditsExpansionfilesUploadArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ExpansionFilesUploadResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_edits_expansionfiles_upload_builder(
            &self.http_client,
            &args.packageName,
            &args.editId,
            &args.apkVersionCode,
            &args.expansionFileType,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_edits_expansionfiles_upload_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher edits images delete.
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
    pub fn androidpublisher_edits_images_delete(
        &self,
        args: &AndroidpublisherEditsImagesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_edits_images_delete_builder(
            &self.http_client,
            &args.packageName,
            &args.editId,
            &args.language,
            &args.imageType,
            &args.imageId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_edits_images_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher edits images deleteall.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ImagesDeleteAllResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_edits_images_deleteall(
        &self,
        args: &AndroidpublisherEditsImagesDeleteallArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ImagesDeleteAllResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_edits_images_deleteall_builder(
            &self.http_client,
            &args.packageName,
            &args.editId,
            &args.language,
            &args.imageType,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_edits_images_deleteall_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher edits images list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ImagesListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidpublisher_edits_images_list(
        &self,
        args: &AndroidpublisherEditsImagesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ImagesListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_edits_images_list_builder(
            &self.http_client,
            &args.packageName,
            &args.editId,
            &args.language,
            &args.imageType,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_edits_images_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher edits images upload.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ImagesUploadResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_edits_images_upload(
        &self,
        args: &AndroidpublisherEditsImagesUploadArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ImagesUploadResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_edits_images_upload_builder(
            &self.http_client,
            &args.packageName,
            &args.editId,
            &args.language,
            &args.imageType,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_edits_images_upload_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher edits listings delete.
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
    pub fn androidpublisher_edits_listings_delete(
        &self,
        args: &AndroidpublisherEditsListingsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_edits_listings_delete_builder(
            &self.http_client,
            &args.packageName,
            &args.editId,
            &args.language,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_edits_listings_delete_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher edits listings deleteall.
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
    pub fn androidpublisher_edits_listings_deleteall(
        &self,
        args: &AndroidpublisherEditsListingsDeleteallArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_edits_listings_deleteall_builder(
            &self.http_client,
            &args.packageName,
            &args.editId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_edits_listings_deleteall_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher edits listings get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Listing result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidpublisher_edits_listings_get(
        &self,
        args: &AndroidpublisherEditsListingsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Listing, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_edits_listings_get_builder(
            &self.http_client,
            &args.packageName,
            &args.editId,
            &args.language,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_edits_listings_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher edits listings list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListingsListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidpublisher_edits_listings_list(
        &self,
        args: &AndroidpublisherEditsListingsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListingsListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_edits_listings_list_builder(
            &self.http_client,
            &args.packageName,
            &args.editId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_edits_listings_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher edits listings patch.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Listing result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidpublisher_edits_listings_patch(
        &self,
        args: &AndroidpublisherEditsListingsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Listing, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_edits_listings_patch_builder(
            &self.http_client,
            &args.packageName,
            &args.editId,
            &args.language,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_edits_listings_patch_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher edits listings update.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Listing result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidpublisher_edits_listings_update(
        &self,
        args: &AndroidpublisherEditsListingsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Listing, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_edits_listings_update_builder(
            &self.http_client,
            &args.packageName,
            &args.editId,
            &args.language,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_edits_listings_update_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher edits testers get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Testers result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidpublisher_edits_testers_get(
        &self,
        args: &AndroidpublisherEditsTestersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Testers, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_edits_testers_get_builder(
            &self.http_client,
            &args.packageName,
            &args.editId,
            &args.track,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_edits_testers_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher edits testers patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Testers result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_edits_testers_patch(
        &self,
        args: &AndroidpublisherEditsTestersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Testers, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_edits_testers_patch_builder(
            &self.http_client,
            &args.packageName,
            &args.editId,
            &args.track,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_edits_testers_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher edits testers update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Testers result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_edits_testers_update(
        &self,
        args: &AndroidpublisherEditsTestersUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Testers, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_edits_testers_update_builder(
            &self.http_client,
            &args.packageName,
            &args.editId,
            &args.track,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_edits_testers_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher edits tracks create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Track result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_edits_tracks_create(
        &self,
        args: &AndroidpublisherEditsTracksCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Track, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_edits_tracks_create_builder(
            &self.http_client,
            &args.packageName,
            &args.editId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_edits_tracks_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher edits tracks get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Track result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidpublisher_edits_tracks_get(
        &self,
        args: &AndroidpublisherEditsTracksGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Track, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_edits_tracks_get_builder(
            &self.http_client,
            &args.packageName,
            &args.editId,
            &args.track,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_edits_tracks_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher edits tracks list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TracksListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidpublisher_edits_tracks_list(
        &self,
        args: &AndroidpublisherEditsTracksListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TracksListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_edits_tracks_list_builder(
            &self.http_client,
            &args.packageName,
            &args.editId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_edits_tracks_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher edits tracks patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Track result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_edits_tracks_patch(
        &self,
        args: &AndroidpublisherEditsTracksPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Track, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_edits_tracks_patch_builder(
            &self.http_client,
            &args.packageName,
            &args.editId,
            &args.track,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_edits_tracks_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher edits tracks update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Track result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_edits_tracks_update(
        &self,
        args: &AndroidpublisherEditsTracksUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Track, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_edits_tracks_update_builder(
            &self.http_client,
            &args.packageName,
            &args.editId,
            &args.track,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_edits_tracks_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher externaltransactions createexternaltransaction.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ExternalTransaction result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_externaltransactions_createexternaltransaction(
        &self,
        args: &AndroidpublisherExternaltransactionsCreateexternaltransactionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ExternalTransaction, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_externaltransactions_createexternaltransaction_builder(
            &self.http_client,
            &args.parent,
            &args.externalTransactionId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_externaltransactions_createexternaltransaction_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher externaltransactions getexternaltransaction.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ExternalTransaction result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidpublisher_externaltransactions_getexternaltransaction(
        &self,
        args: &AndroidpublisherExternaltransactionsGetexternaltransactionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ExternalTransaction, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_externaltransactions_getexternaltransaction_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_externaltransactions_getexternaltransaction_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher externaltransactions refundexternaltransaction.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ExternalTransaction result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_externaltransactions_refundexternaltransaction(
        &self,
        args: &AndroidpublisherExternaltransactionsRefundexternaltransactionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ExternalTransaction, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_externaltransactions_refundexternaltransaction_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_externaltransactions_refundexternaltransaction_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher generatedapks download.
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
    pub fn androidpublisher_generatedapks_download(
        &self,
        args: &AndroidpublisherGeneratedapksDownloadArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_generatedapks_download_builder(
            &self.http_client,
            &args.packageName,
            &args.versionCode,
            &args.downloadId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_generatedapks_download_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher generatedapks list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GeneratedApksListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidpublisher_generatedapks_list(
        &self,
        args: &AndroidpublisherGeneratedapksListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GeneratedApksListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_generatedapks_list_builder(
            &self.http_client,
            &args.packageName,
            &args.versionCode,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_generatedapks_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher grants create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Grant result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_grants_create(
        &self,
        args: &AndroidpublisherGrantsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Grant, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_grants_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_grants_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher grants delete.
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
    pub fn androidpublisher_grants_delete(
        &self,
        args: &AndroidpublisherGrantsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_grants_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_grants_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher grants patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Grant result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_grants_patch(
        &self,
        args: &AndroidpublisherGrantsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Grant, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_grants_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_grants_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher inappproducts batch delete.
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
    pub fn androidpublisher_inappproducts_batch_delete(
        &self,
        args: &AndroidpublisherInappproductsBatchDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_inappproducts_batch_delete_builder(
            &self.http_client,
            &args.packageName,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_inappproducts_batch_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher inappproducts batch get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the InappproductsBatchGetResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidpublisher_inappproducts_batch_get(
        &self,
        args: &AndroidpublisherInappproductsBatchGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<InappproductsBatchGetResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_inappproducts_batch_get_builder(
            &self.http_client,
            &args.packageName,
            &args.sku,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_inappproducts_batch_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher inappproducts batch update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the InappproductsBatchUpdateResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_inappproducts_batch_update(
        &self,
        args: &AndroidpublisherInappproductsBatchUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<InappproductsBatchUpdateResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_inappproducts_batch_update_builder(
            &self.http_client,
            &args.packageName,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_inappproducts_batch_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher inappproducts delete.
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
    pub fn androidpublisher_inappproducts_delete(
        &self,
        args: &AndroidpublisherInappproductsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_inappproducts_delete_builder(
            &self.http_client,
            &args.packageName,
            &args.sku,
            &args.latencyTolerance,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_inappproducts_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher inappproducts get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the InAppProduct result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidpublisher_inappproducts_get(
        &self,
        args: &AndroidpublisherInappproductsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<InAppProduct, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_inappproducts_get_builder(
            &self.http_client,
            &args.packageName,
            &args.sku,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_inappproducts_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher inappproducts insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the InAppProduct result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_inappproducts_insert(
        &self,
        args: &AndroidpublisherInappproductsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<InAppProduct, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_inappproducts_insert_builder(
            &self.http_client,
            &args.packageName,
            &args.autoConvertMissingPrices,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_inappproducts_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher inappproducts list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the InappproductsListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidpublisher_inappproducts_list(
        &self,
        args: &AndroidpublisherInappproductsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<InappproductsListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_inappproducts_list_builder(
            &self.http_client,
            &args.packageName,
            &args.maxResults,
            &args.startIndex,
            &args.token,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_inappproducts_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher inappproducts patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the InAppProduct result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_inappproducts_patch(
        &self,
        args: &AndroidpublisherInappproductsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<InAppProduct, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_inappproducts_patch_builder(
            &self.http_client,
            &args.packageName,
            &args.sku,
            &args.autoConvertMissingPrices,
            &args.latencyTolerance,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_inappproducts_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher inappproducts update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the InAppProduct result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_inappproducts_update(
        &self,
        args: &AndroidpublisherInappproductsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<InAppProduct, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_inappproducts_update_builder(
            &self.http_client,
            &args.packageName,
            &args.sku,
            &args.allowMissing,
            &args.autoConvertMissingPrices,
            &args.latencyTolerance,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_inappproducts_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher internalappsharingartifacts uploadapk.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the InternalAppSharingArtifact result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_internalappsharingartifacts_uploadapk(
        &self,
        args: &AndroidpublisherInternalappsharingartifactsUploadapkArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<InternalAppSharingArtifact, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_internalappsharingartifacts_uploadapk_builder(
            &self.http_client,
            &args.packageName,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_internalappsharingartifacts_uploadapk_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher internalappsharingartifacts uploadbundle.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the InternalAppSharingArtifact result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_internalappsharingartifacts_uploadbundle(
        &self,
        args: &AndroidpublisherInternalappsharingartifactsUploadbundleArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<InternalAppSharingArtifact, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_internalappsharingartifacts_uploadbundle_builder(
            &self.http_client,
            &args.packageName,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_internalappsharingartifacts_uploadbundle_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher monetization convert region prices.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ConvertRegionPricesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_monetization_convert_region_prices(
        &self,
        args: &AndroidpublisherMonetizationConvertRegionPricesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ConvertRegionPricesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_monetization_convert_region_prices_builder(
            &self.http_client,
            &args.packageName,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_monetization_convert_region_prices_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher monetization onetimeproducts batch delete.
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
    pub fn androidpublisher_monetization_onetimeproducts_batch_delete(
        &self,
        args: &AndroidpublisherMonetizationOnetimeproductsBatchDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_monetization_onetimeproducts_batch_delete_builder(
            &self.http_client,
            &args.packageName,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_monetization_onetimeproducts_batch_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher monetization onetimeproducts batch get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchGetOneTimeProductsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidpublisher_monetization_onetimeproducts_batch_get(
        &self,
        args: &AndroidpublisherMonetizationOnetimeproductsBatchGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchGetOneTimeProductsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_monetization_onetimeproducts_batch_get_builder(
            &self.http_client,
            &args.packageName,
            &args.productIds,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_monetization_onetimeproducts_batch_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher monetization onetimeproducts batch update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchUpdateOneTimeProductsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_monetization_onetimeproducts_batch_update(
        &self,
        args: &AndroidpublisherMonetizationOnetimeproductsBatchUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchUpdateOneTimeProductsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_monetization_onetimeproducts_batch_update_builder(
            &self.http_client,
            &args.packageName,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_monetization_onetimeproducts_batch_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher monetization onetimeproducts delete.
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
    pub fn androidpublisher_monetization_onetimeproducts_delete(
        &self,
        args: &AndroidpublisherMonetizationOnetimeproductsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_monetization_onetimeproducts_delete_builder(
            &self.http_client,
            &args.packageName,
            &args.productId,
            &args.latencyTolerance,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_monetization_onetimeproducts_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher monetization onetimeproducts get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the OneTimeProduct result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidpublisher_monetization_onetimeproducts_get(
        &self,
        args: &AndroidpublisherMonetizationOnetimeproductsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<OneTimeProduct, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_monetization_onetimeproducts_get_builder(
            &self.http_client,
            &args.packageName,
            &args.productId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_monetization_onetimeproducts_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher monetization onetimeproducts list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListOneTimeProductsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidpublisher_monetization_onetimeproducts_list(
        &self,
        args: &AndroidpublisherMonetizationOnetimeproductsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOneTimeProductsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_monetization_onetimeproducts_list_builder(
            &self.http_client,
            &args.packageName,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_monetization_onetimeproducts_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher monetization onetimeproducts patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the OneTimeProduct result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_monetization_onetimeproducts_patch(
        &self,
        args: &AndroidpublisherMonetizationOnetimeproductsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<OneTimeProduct, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_monetization_onetimeproducts_patch_builder(
            &self.http_client,
            &args.packageName,
            &args.productId,
            &args.allowMissing,
            &args.latencyTolerance,
            &args.regionsVersion.version,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_monetization_onetimeproducts_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher monetization onetimeproducts purchase options batch delete.
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
    pub fn androidpublisher_monetization_onetimeproducts_purchase_options_batch_delete(
        &self,
        args: &AndroidpublisherMonetizationOnetimeproductsPurchaseOptionsBatchDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_monetization_onetimeproducts_purchase_options_batch_delete_builder(
            &self.http_client,
            &args.packageName,
            &args.productId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_monetization_onetimeproducts_purchase_options_batch_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher monetization onetimeproducts purchase options batch update states.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchUpdatePurchaseOptionStatesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_monetization_onetimeproducts_purchase_options_batch_update_states(
        &self,
        args: &AndroidpublisherMonetizationOnetimeproductsPurchaseOptionsBatchUpdateStatesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchUpdatePurchaseOptionStatesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_monetization_onetimeproducts_purchase_options_batch_update_states_builder(
            &self.http_client,
            &args.packageName,
            &args.productId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_monetization_onetimeproducts_purchase_options_batch_update_states_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher monetization onetimeproducts purchase options offers activate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the OneTimeProductOffer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_monetization_onetimeproducts_purchase_options_offers_activate(
        &self,
        args: &AndroidpublisherMonetizationOnetimeproductsPurchaseOptionsOffersActivateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<OneTimeProductOffer, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_monetization_onetimeproducts_purchase_options_offers_activate_builder(
            &self.http_client,
            &args.packageName,
            &args.productId,
            &args.purchaseOptionId,
            &args.offerId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_monetization_onetimeproducts_purchase_options_offers_activate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher monetization onetimeproducts purchase options offers batch delete.
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
    pub fn androidpublisher_monetization_onetimeproducts_purchase_options_offers_batch_delete(
        &self,
        args: &AndroidpublisherMonetizationOnetimeproductsPurchaseOptionsOffersBatchDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_monetization_onetimeproducts_purchase_options_offers_batch_delete_builder(
            &self.http_client,
            &args.packageName,
            &args.productId,
            &args.purchaseOptionId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_monetization_onetimeproducts_purchase_options_offers_batch_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher monetization onetimeproducts purchase options offers batch get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchGetOneTimeProductOffersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidpublisher_monetization_onetimeproducts_purchase_options_offers_batch_get(
        &self,
        args: &AndroidpublisherMonetizationOnetimeproductsPurchaseOptionsOffersBatchGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchGetOneTimeProductOffersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_monetization_onetimeproducts_purchase_options_offers_batch_get_builder(
            &self.http_client,
            &args.packageName,
            &args.productId,
            &args.purchaseOptionId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_monetization_onetimeproducts_purchase_options_offers_batch_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher monetization onetimeproducts purchase options offers batch update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchUpdateOneTimeProductOffersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_monetization_onetimeproducts_purchase_options_offers_batch_update(
        &self,
        args: &AndroidpublisherMonetizationOnetimeproductsPurchaseOptionsOffersBatchUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchUpdateOneTimeProductOffersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_monetization_onetimeproducts_purchase_options_offers_batch_update_builder(
            &self.http_client,
            &args.packageName,
            &args.productId,
            &args.purchaseOptionId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_monetization_onetimeproducts_purchase_options_offers_batch_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher monetization onetimeproducts purchase options offers batch update states.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchUpdateOneTimeProductOfferStatesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_monetization_onetimeproducts_purchase_options_offers_batch_update_states(
        &self,
        args: &AndroidpublisherMonetizationOnetimeproductsPurchaseOptionsOffersBatchUpdateStatesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchUpdateOneTimeProductOfferStatesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_monetization_onetimeproducts_purchase_options_offers_batch_update_states_builder(
            &self.http_client,
            &args.packageName,
            &args.productId,
            &args.purchaseOptionId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_monetization_onetimeproducts_purchase_options_offers_batch_update_states_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher monetization onetimeproducts purchase options offers cancel.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the OneTimeProductOffer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_monetization_onetimeproducts_purchase_options_offers_cancel(
        &self,
        args: &AndroidpublisherMonetizationOnetimeproductsPurchaseOptionsOffersCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<OneTimeProductOffer, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_monetization_onetimeproducts_purchase_options_offers_cancel_builder(
            &self.http_client,
            &args.packageName,
            &args.productId,
            &args.purchaseOptionId,
            &args.offerId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_monetization_onetimeproducts_purchase_options_offers_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher monetization onetimeproducts purchase options offers deactivate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the OneTimeProductOffer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_monetization_onetimeproducts_purchase_options_offers_deactivate(
        &self,
        args: &AndroidpublisherMonetizationOnetimeproductsPurchaseOptionsOffersDeactivateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<OneTimeProductOffer, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_monetization_onetimeproducts_purchase_options_offers_deactivate_builder(
            &self.http_client,
            &args.packageName,
            &args.productId,
            &args.purchaseOptionId,
            &args.offerId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_monetization_onetimeproducts_purchase_options_offers_deactivate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher monetization onetimeproducts purchase options offers list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListOneTimeProductOffersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidpublisher_monetization_onetimeproducts_purchase_options_offers_list(
        &self,
        args: &AndroidpublisherMonetizationOnetimeproductsPurchaseOptionsOffersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOneTimeProductOffersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_monetization_onetimeproducts_purchase_options_offers_list_builder(
            &self.http_client,
            &args.packageName,
            &args.productId,
            &args.purchaseOptionId,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_monetization_onetimeproducts_purchase_options_offers_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher monetization subscriptions archive.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Subscription result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_monetization_subscriptions_archive(
        &self,
        args: &AndroidpublisherMonetizationSubscriptionsArchiveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Subscription, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_monetization_subscriptions_archive_builder(
            &self.http_client,
            &args.packageName,
            &args.productId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_monetization_subscriptions_archive_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher monetization subscriptions batch get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchGetSubscriptionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidpublisher_monetization_subscriptions_batch_get(
        &self,
        args: &AndroidpublisherMonetizationSubscriptionsBatchGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchGetSubscriptionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_monetization_subscriptions_batch_get_builder(
            &self.http_client,
            &args.packageName,
            &args.productIds,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_monetization_subscriptions_batch_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher monetization subscriptions batch update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchUpdateSubscriptionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_monetization_subscriptions_batch_update(
        &self,
        args: &AndroidpublisherMonetizationSubscriptionsBatchUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchUpdateSubscriptionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_monetization_subscriptions_batch_update_builder(
            &self.http_client,
            &args.packageName,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_monetization_subscriptions_batch_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher monetization subscriptions create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Subscription result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_monetization_subscriptions_create(
        &self,
        args: &AndroidpublisherMonetizationSubscriptionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Subscription, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_monetization_subscriptions_create_builder(
            &self.http_client,
            &args.packageName,
            &args.productId,
            &args.regionsVersion.version,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_monetization_subscriptions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher monetization subscriptions delete.
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
    pub fn androidpublisher_monetization_subscriptions_delete(
        &self,
        args: &AndroidpublisherMonetizationSubscriptionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_monetization_subscriptions_delete_builder(
            &self.http_client,
            &args.packageName,
            &args.productId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_monetization_subscriptions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher monetization subscriptions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Subscription result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidpublisher_monetization_subscriptions_get(
        &self,
        args: &AndroidpublisherMonetizationSubscriptionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Subscription, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_monetization_subscriptions_get_builder(
            &self.http_client,
            &args.packageName,
            &args.productId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_monetization_subscriptions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher monetization subscriptions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListSubscriptionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidpublisher_monetization_subscriptions_list(
        &self,
        args: &AndroidpublisherMonetizationSubscriptionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSubscriptionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_monetization_subscriptions_list_builder(
            &self.http_client,
            &args.packageName,
            &args.pageSize,
            &args.pageToken,
            &args.showArchived,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_monetization_subscriptions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher monetization subscriptions patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Subscription result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_monetization_subscriptions_patch(
        &self,
        args: &AndroidpublisherMonetizationSubscriptionsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Subscription, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_monetization_subscriptions_patch_builder(
            &self.http_client,
            &args.packageName,
            &args.productId,
            &args.allowMissing,
            &args.latencyTolerance,
            &args.regionsVersion.version,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_monetization_subscriptions_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher monetization subscriptions base plans activate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Subscription result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_monetization_subscriptions_base_plans_activate(
        &self,
        args: &AndroidpublisherMonetizationSubscriptionsBasePlansActivateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Subscription, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_monetization_subscriptions_base_plans_activate_builder(
            &self.http_client,
            &args.packageName,
            &args.productId,
            &args.basePlanId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_monetization_subscriptions_base_plans_activate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher monetization subscriptions base plans batch migrate prices.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchMigrateBasePlanPricesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_monetization_subscriptions_base_plans_batch_migrate_prices(
        &self,
        args: &AndroidpublisherMonetizationSubscriptionsBasePlansBatchMigratePricesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchMigrateBasePlanPricesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_monetization_subscriptions_base_plans_batch_migrate_prices_builder(
            &self.http_client,
            &args.packageName,
            &args.productId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_monetization_subscriptions_base_plans_batch_migrate_prices_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher monetization subscriptions base plans batch update states.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchUpdateBasePlanStatesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_monetization_subscriptions_base_plans_batch_update_states(
        &self,
        args: &AndroidpublisherMonetizationSubscriptionsBasePlansBatchUpdateStatesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchUpdateBasePlanStatesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_monetization_subscriptions_base_plans_batch_update_states_builder(
            &self.http_client,
            &args.packageName,
            &args.productId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_monetization_subscriptions_base_plans_batch_update_states_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher monetization subscriptions base plans deactivate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Subscription result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_monetization_subscriptions_base_plans_deactivate(
        &self,
        args: &AndroidpublisherMonetizationSubscriptionsBasePlansDeactivateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Subscription, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_monetization_subscriptions_base_plans_deactivate_builder(
            &self.http_client,
            &args.packageName,
            &args.productId,
            &args.basePlanId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_monetization_subscriptions_base_plans_deactivate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher monetization subscriptions base plans delete.
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
    pub fn androidpublisher_monetization_subscriptions_base_plans_delete(
        &self,
        args: &AndroidpublisherMonetizationSubscriptionsBasePlansDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_monetization_subscriptions_base_plans_delete_builder(
            &self.http_client,
            &args.packageName,
            &args.productId,
            &args.basePlanId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_monetization_subscriptions_base_plans_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher monetization subscriptions base plans migrate prices.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the MigrateBasePlanPricesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_monetization_subscriptions_base_plans_migrate_prices(
        &self,
        args: &AndroidpublisherMonetizationSubscriptionsBasePlansMigratePricesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<MigrateBasePlanPricesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_monetization_subscriptions_base_plans_migrate_prices_builder(
            &self.http_client,
            &args.packageName,
            &args.productId,
            &args.basePlanId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_monetization_subscriptions_base_plans_migrate_prices_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher monetization subscriptions base plans offers activate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SubscriptionOffer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_monetization_subscriptions_base_plans_offers_activate(
        &self,
        args: &AndroidpublisherMonetizationSubscriptionsBasePlansOffersActivateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SubscriptionOffer, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_monetization_subscriptions_base_plans_offers_activate_builder(
            &self.http_client,
            &args.packageName,
            &args.productId,
            &args.basePlanId,
            &args.offerId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_monetization_subscriptions_base_plans_offers_activate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher monetization subscriptions base plans offers batch get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchGetSubscriptionOffersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidpublisher_monetization_subscriptions_base_plans_offers_batch_get(
        &self,
        args: &AndroidpublisherMonetizationSubscriptionsBasePlansOffersBatchGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchGetSubscriptionOffersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_monetization_subscriptions_base_plans_offers_batch_get_builder(
            &self.http_client,
            &args.packageName,
            &args.productId,
            &args.basePlanId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_monetization_subscriptions_base_plans_offers_batch_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher monetization subscriptions base plans offers batch update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchUpdateSubscriptionOffersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_monetization_subscriptions_base_plans_offers_batch_update(
        &self,
        args: &AndroidpublisherMonetizationSubscriptionsBasePlansOffersBatchUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchUpdateSubscriptionOffersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_monetization_subscriptions_base_plans_offers_batch_update_builder(
            &self.http_client,
            &args.packageName,
            &args.productId,
            &args.basePlanId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_monetization_subscriptions_base_plans_offers_batch_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher monetization subscriptions base plans offers batch update states.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchUpdateSubscriptionOfferStatesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_monetization_subscriptions_base_plans_offers_batch_update_states(
        &self,
        args: &AndroidpublisherMonetizationSubscriptionsBasePlansOffersBatchUpdateStatesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchUpdateSubscriptionOfferStatesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_monetization_subscriptions_base_plans_offers_batch_update_states_builder(
            &self.http_client,
            &args.packageName,
            &args.productId,
            &args.basePlanId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_monetization_subscriptions_base_plans_offers_batch_update_states_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher monetization subscriptions base plans offers create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SubscriptionOffer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_monetization_subscriptions_base_plans_offers_create(
        &self,
        args: &AndroidpublisherMonetizationSubscriptionsBasePlansOffersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SubscriptionOffer, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_monetization_subscriptions_base_plans_offers_create_builder(
            &self.http_client,
            &args.packageName,
            &args.productId,
            &args.basePlanId,
            &args.offerId,
            &args.regionsVersion.version,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_monetization_subscriptions_base_plans_offers_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher monetization subscriptions base plans offers deactivate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SubscriptionOffer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_monetization_subscriptions_base_plans_offers_deactivate(
        &self,
        args: &AndroidpublisherMonetizationSubscriptionsBasePlansOffersDeactivateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SubscriptionOffer, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_monetization_subscriptions_base_plans_offers_deactivate_builder(
            &self.http_client,
            &args.packageName,
            &args.productId,
            &args.basePlanId,
            &args.offerId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_monetization_subscriptions_base_plans_offers_deactivate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher monetization subscriptions base plans offers delete.
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
    pub fn androidpublisher_monetization_subscriptions_base_plans_offers_delete(
        &self,
        args: &AndroidpublisherMonetizationSubscriptionsBasePlansOffersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_monetization_subscriptions_base_plans_offers_delete_builder(
            &self.http_client,
            &args.packageName,
            &args.productId,
            &args.basePlanId,
            &args.offerId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_monetization_subscriptions_base_plans_offers_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher monetization subscriptions base plans offers get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SubscriptionOffer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidpublisher_monetization_subscriptions_base_plans_offers_get(
        &self,
        args: &AndroidpublisherMonetizationSubscriptionsBasePlansOffersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SubscriptionOffer, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_monetization_subscriptions_base_plans_offers_get_builder(
            &self.http_client,
            &args.packageName,
            &args.productId,
            &args.basePlanId,
            &args.offerId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_monetization_subscriptions_base_plans_offers_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher monetization subscriptions base plans offers list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListSubscriptionOffersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidpublisher_monetization_subscriptions_base_plans_offers_list(
        &self,
        args: &AndroidpublisherMonetizationSubscriptionsBasePlansOffersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSubscriptionOffersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_monetization_subscriptions_base_plans_offers_list_builder(
            &self.http_client,
            &args.packageName,
            &args.productId,
            &args.basePlanId,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_monetization_subscriptions_base_plans_offers_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher monetization subscriptions base plans offers patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SubscriptionOffer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_monetization_subscriptions_base_plans_offers_patch(
        &self,
        args: &AndroidpublisherMonetizationSubscriptionsBasePlansOffersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SubscriptionOffer, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_monetization_subscriptions_base_plans_offers_patch_builder(
            &self.http_client,
            &args.packageName,
            &args.productId,
            &args.basePlanId,
            &args.offerId,
            &args.allowMissing,
            &args.latencyTolerance,
            &args.regionsVersion.version,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_monetization_subscriptions_base_plans_offers_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher orders batchget.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchGetOrdersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidpublisher_orders_batchget(
        &self,
        args: &AndroidpublisherOrdersBatchgetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchGetOrdersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_orders_batchget_builder(
            &self.http_client,
            &args.packageName,
            &args.orderIds,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_orders_batchget_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher orders get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Order result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidpublisher_orders_get(
        &self,
        args: &AndroidpublisherOrdersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Order, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_orders_get_builder(
            &self.http_client,
            &args.packageName,
            &args.orderId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_orders_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher orders refund.
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
    pub fn androidpublisher_orders_refund(
        &self,
        args: &AndroidpublisherOrdersRefundArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_orders_refund_builder(
            &self.http_client,
            &args.packageName,
            &args.orderId,
            &args.revoke,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_orders_refund_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher purchases products acknowledge.
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
    pub fn androidpublisher_purchases_products_acknowledge(
        &self,
        args: &AndroidpublisherPurchasesProductsAcknowledgeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_purchases_products_acknowledge_builder(
            &self.http_client,
            &args.packageName,
            &args.productId,
            &args.token,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_purchases_products_acknowledge_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher purchases products consume.
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
    pub fn androidpublisher_purchases_products_consume(
        &self,
        args: &AndroidpublisherPurchasesProductsConsumeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_purchases_products_consume_builder(
            &self.http_client,
            &args.packageName,
            &args.productId,
            &args.token,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_purchases_products_consume_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher purchases products get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProductPurchase result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidpublisher_purchases_products_get(
        &self,
        args: &AndroidpublisherPurchasesProductsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProductPurchase, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_purchases_products_get_builder(
            &self.http_client,
            &args.packageName,
            &args.productId,
            &args.token,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_purchases_products_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher purchases productsv2 getproductpurchasev2.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProductPurchaseV2 result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidpublisher_purchases_productsv2_getproductpurchasev2(
        &self,
        args: &AndroidpublisherPurchasesProductsv2Getproductpurchasev2Args,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProductPurchaseV2, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_purchases_productsv2_getproductpurchasev2_builder(
            &self.http_client,
            &args.packageName,
            &args.token,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_purchases_productsv2_getproductpurchasev2_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher purchases subscriptions acknowledge.
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
    pub fn androidpublisher_purchases_subscriptions_acknowledge(
        &self,
        args: &AndroidpublisherPurchasesSubscriptionsAcknowledgeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_purchases_subscriptions_acknowledge_builder(
            &self.http_client,
            &args.packageName,
            &args.subscriptionId,
            &args.token,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_purchases_subscriptions_acknowledge_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher purchases subscriptions cancel.
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
    pub fn androidpublisher_purchases_subscriptions_cancel(
        &self,
        args: &AndroidpublisherPurchasesSubscriptionsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_purchases_subscriptions_cancel_builder(
            &self.http_client,
            &args.packageName,
            &args.subscriptionId,
            &args.token,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_purchases_subscriptions_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher purchases subscriptions defer.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SubscriptionPurchasesDeferResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_purchases_subscriptions_defer(
        &self,
        args: &AndroidpublisherPurchasesSubscriptionsDeferArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SubscriptionPurchasesDeferResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_purchases_subscriptions_defer_builder(
            &self.http_client,
            &args.packageName,
            &args.subscriptionId,
            &args.token,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_purchases_subscriptions_defer_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher purchases subscriptions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SubscriptionPurchase result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidpublisher_purchases_subscriptions_get(
        &self,
        args: &AndroidpublisherPurchasesSubscriptionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SubscriptionPurchase, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_purchases_subscriptions_get_builder(
            &self.http_client,
            &args.packageName,
            &args.subscriptionId,
            &args.token,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_purchases_subscriptions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher purchases subscriptions refund.
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
    pub fn androidpublisher_purchases_subscriptions_refund(
        &self,
        args: &AndroidpublisherPurchasesSubscriptionsRefundArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_purchases_subscriptions_refund_builder(
            &self.http_client,
            &args.packageName,
            &args.subscriptionId,
            &args.token,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_purchases_subscriptions_refund_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher purchases subscriptions revoke.
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
    pub fn androidpublisher_purchases_subscriptions_revoke(
        &self,
        args: &AndroidpublisherPurchasesSubscriptionsRevokeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_purchases_subscriptions_revoke_builder(
            &self.http_client,
            &args.packageName,
            &args.subscriptionId,
            &args.token,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_purchases_subscriptions_revoke_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher purchases subscriptionsv2 cancel.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CancelSubscriptionPurchaseResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_purchases_subscriptionsv2_cancel(
        &self,
        args: &AndroidpublisherPurchasesSubscriptionsv2CancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CancelSubscriptionPurchaseResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_purchases_subscriptionsv2_cancel_builder(
            &self.http_client,
            &args.packageName,
            &args.token,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_purchases_subscriptionsv2_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher purchases subscriptionsv2 defer.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeferSubscriptionPurchaseResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_purchases_subscriptionsv2_defer(
        &self,
        args: &AndroidpublisherPurchasesSubscriptionsv2DeferArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeferSubscriptionPurchaseResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_purchases_subscriptionsv2_defer_builder(
            &self.http_client,
            &args.packageName,
            &args.token,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_purchases_subscriptionsv2_defer_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher purchases subscriptionsv2 get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SubscriptionPurchaseV2 result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidpublisher_purchases_subscriptionsv2_get(
        &self,
        args: &AndroidpublisherPurchasesSubscriptionsv2GetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SubscriptionPurchaseV2, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_purchases_subscriptionsv2_get_builder(
            &self.http_client,
            &args.packageName,
            &args.token,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_purchases_subscriptionsv2_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher purchases subscriptionsv2 revoke.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RevokeSubscriptionPurchaseResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_purchases_subscriptionsv2_revoke(
        &self,
        args: &AndroidpublisherPurchasesSubscriptionsv2RevokeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RevokeSubscriptionPurchaseResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_purchases_subscriptionsv2_revoke_builder(
            &self.http_client,
            &args.packageName,
            &args.token,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_purchases_subscriptionsv2_revoke_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher purchases voidedpurchases list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the VoidedPurchasesListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidpublisher_purchases_voidedpurchases_list(
        &self,
        args: &AndroidpublisherPurchasesVoidedpurchasesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<VoidedPurchasesListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_purchases_voidedpurchases_list_builder(
            &self.http_client,
            &args.packageName,
            &args.endTime,
            &args.includeQuantityBasedPartialRefund,
            &args.maxResults,
            &args.startIndex,
            &args.startTime,
            &args.token,
            &args.type_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_purchases_voidedpurchases_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher reviews get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Review result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidpublisher_reviews_get(
        &self,
        args: &AndroidpublisherReviewsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Review, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_reviews_get_builder(
            &self.http_client,
            &args.packageName,
            &args.reviewId,
            &args.translationLanguage,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_reviews_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher reviews list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ReviewsListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidpublisher_reviews_list(
        &self,
        args: &AndroidpublisherReviewsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ReviewsListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_reviews_list_builder(
            &self.http_client,
            &args.packageName,
            &args.maxResults,
            &args.startIndex,
            &args.token,
            &args.translationLanguage,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_reviews_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher reviews reply.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ReviewsReplyResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_reviews_reply(
        &self,
        args: &AndroidpublisherReviewsReplyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ReviewsReplyResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_reviews_reply_builder(
            &self.http_client,
            &args.packageName,
            &args.reviewId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_reviews_reply_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher systemapks variants create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Variant result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidpublisher_systemapks_variants_create(
        &self,
        args: &AndroidpublisherSystemapksVariantsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Variant, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_systemapks_variants_create_builder(
            &self.http_client,
            &args.packageName,
            &args.versionCode,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_systemapks_variants_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher systemapks variants download.
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
    pub fn androidpublisher_systemapks_variants_download(
        &self,
        args: &AndroidpublisherSystemapksVariantsDownloadArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_systemapks_variants_download_builder(
            &self.http_client,
            &args.packageName,
            &args.versionCode,
            &args.variantId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_systemapks_variants_download_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher systemapks variants get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Variant result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidpublisher_systemapks_variants_get(
        &self,
        args: &AndroidpublisherSystemapksVariantsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Variant, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_systemapks_variants_get_builder(
            &self.http_client,
            &args.packageName,
            &args.versionCode,
            &args.variantId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_systemapks_variants_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher systemapks variants list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SystemApksListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn androidpublisher_systemapks_variants_list(
        &self,
        args: &AndroidpublisherSystemapksVariantsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SystemApksListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_systemapks_variants_list_builder(
            &self.http_client,
            &args.packageName,
            &args.versionCode,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_systemapks_variants_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher users create.
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
    pub fn androidpublisher_users_create(
        &self,
        args: &AndroidpublisherUsersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<User, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_users_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_users_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher users delete.
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
    pub fn androidpublisher_users_delete(
        &self,
        args: &AndroidpublisherUsersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_users_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_users_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher users list.
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
    pub fn androidpublisher_users_list(
        &self,
        args: &AndroidpublisherUsersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListUsersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_users_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_users_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidpublisher users patch.
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
    pub fn androidpublisher_users_patch(
        &self,
        args: &AndroidpublisherUsersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<User, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidpublisher_users_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = androidpublisher_users_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
