//! WalletobjectsProvider - State-aware walletobjects API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       walletobjects API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::walletobjects::{
    walletobjects_eventticketclass_addmessage_builder, walletobjects_eventticketclass_addmessage_task,
    walletobjects_eventticketclass_get_builder, walletobjects_eventticketclass_get_task,
    walletobjects_eventticketclass_insert_builder, walletobjects_eventticketclass_insert_task,
    walletobjects_eventticketclass_list_builder, walletobjects_eventticketclass_list_task,
    walletobjects_eventticketclass_patch_builder, walletobjects_eventticketclass_patch_task,
    walletobjects_eventticketclass_update_builder, walletobjects_eventticketclass_update_task,
    walletobjects_eventticketobject_addmessage_builder, walletobjects_eventticketobject_addmessage_task,
    walletobjects_eventticketobject_get_builder, walletobjects_eventticketobject_get_task,
    walletobjects_eventticketobject_insert_builder, walletobjects_eventticketobject_insert_task,
    walletobjects_eventticketobject_list_builder, walletobjects_eventticketobject_list_task,
    walletobjects_eventticketobject_modifylinkedofferobjects_builder, walletobjects_eventticketobject_modifylinkedofferobjects_task,
    walletobjects_eventticketobject_patch_builder, walletobjects_eventticketobject_patch_task,
    walletobjects_eventticketobject_update_builder, walletobjects_eventticketobject_update_task,
    walletobjects_flightclass_addmessage_builder, walletobjects_flightclass_addmessage_task,
    walletobjects_flightclass_get_builder, walletobjects_flightclass_get_task,
    walletobjects_flightclass_insert_builder, walletobjects_flightclass_insert_task,
    walletobjects_flightclass_list_builder, walletobjects_flightclass_list_task,
    walletobjects_flightclass_patch_builder, walletobjects_flightclass_patch_task,
    walletobjects_flightclass_update_builder, walletobjects_flightclass_update_task,
    walletobjects_flightobject_addmessage_builder, walletobjects_flightobject_addmessage_task,
    walletobjects_flightobject_get_builder, walletobjects_flightobject_get_task,
    walletobjects_flightobject_insert_builder, walletobjects_flightobject_insert_task,
    walletobjects_flightobject_list_builder, walletobjects_flightobject_list_task,
    walletobjects_flightobject_patch_builder, walletobjects_flightobject_patch_task,
    walletobjects_flightobject_update_builder, walletobjects_flightobject_update_task,
    walletobjects_genericclass_addmessage_builder, walletobjects_genericclass_addmessage_task,
    walletobjects_genericclass_get_builder, walletobjects_genericclass_get_task,
    walletobjects_genericclass_insert_builder, walletobjects_genericclass_insert_task,
    walletobjects_genericclass_list_builder, walletobjects_genericclass_list_task,
    walletobjects_genericclass_patch_builder, walletobjects_genericclass_patch_task,
    walletobjects_genericclass_update_builder, walletobjects_genericclass_update_task,
    walletobjects_genericobject_addmessage_builder, walletobjects_genericobject_addmessage_task,
    walletobjects_genericobject_get_builder, walletobjects_genericobject_get_task,
    walletobjects_genericobject_insert_builder, walletobjects_genericobject_insert_task,
    walletobjects_genericobject_list_builder, walletobjects_genericobject_list_task,
    walletobjects_genericobject_patch_builder, walletobjects_genericobject_patch_task,
    walletobjects_genericobject_update_builder, walletobjects_genericobject_update_task,
    walletobjects_giftcardclass_addmessage_builder, walletobjects_giftcardclass_addmessage_task,
    walletobjects_giftcardclass_get_builder, walletobjects_giftcardclass_get_task,
    walletobjects_giftcardclass_insert_builder, walletobjects_giftcardclass_insert_task,
    walletobjects_giftcardclass_list_builder, walletobjects_giftcardclass_list_task,
    walletobjects_giftcardclass_patch_builder, walletobjects_giftcardclass_patch_task,
    walletobjects_giftcardclass_update_builder, walletobjects_giftcardclass_update_task,
    walletobjects_giftcardobject_addmessage_builder, walletobjects_giftcardobject_addmessage_task,
    walletobjects_giftcardobject_get_builder, walletobjects_giftcardobject_get_task,
    walletobjects_giftcardobject_insert_builder, walletobjects_giftcardobject_insert_task,
    walletobjects_giftcardobject_list_builder, walletobjects_giftcardobject_list_task,
    walletobjects_giftcardobject_patch_builder, walletobjects_giftcardobject_patch_task,
    walletobjects_giftcardobject_update_builder, walletobjects_giftcardobject_update_task,
    walletobjects_issuer_get_builder, walletobjects_issuer_get_task,
    walletobjects_issuer_insert_builder, walletobjects_issuer_insert_task,
    walletobjects_issuer_list_builder, walletobjects_issuer_list_task,
    walletobjects_issuer_patch_builder, walletobjects_issuer_patch_task,
    walletobjects_issuer_update_builder, walletobjects_issuer_update_task,
    walletobjects_jwt_insert_builder, walletobjects_jwt_insert_task,
    walletobjects_loyaltyclass_addmessage_builder, walletobjects_loyaltyclass_addmessage_task,
    walletobjects_loyaltyclass_get_builder, walletobjects_loyaltyclass_get_task,
    walletobjects_loyaltyclass_insert_builder, walletobjects_loyaltyclass_insert_task,
    walletobjects_loyaltyclass_list_builder, walletobjects_loyaltyclass_list_task,
    walletobjects_loyaltyclass_patch_builder, walletobjects_loyaltyclass_patch_task,
    walletobjects_loyaltyclass_update_builder, walletobjects_loyaltyclass_update_task,
    walletobjects_loyaltyobject_addmessage_builder, walletobjects_loyaltyobject_addmessage_task,
    walletobjects_loyaltyobject_get_builder, walletobjects_loyaltyobject_get_task,
    walletobjects_loyaltyobject_insert_builder, walletobjects_loyaltyobject_insert_task,
    walletobjects_loyaltyobject_list_builder, walletobjects_loyaltyobject_list_task,
    walletobjects_loyaltyobject_modifylinkedofferobjects_builder, walletobjects_loyaltyobject_modifylinkedofferobjects_task,
    walletobjects_loyaltyobject_patch_builder, walletobjects_loyaltyobject_patch_task,
    walletobjects_loyaltyobject_update_builder, walletobjects_loyaltyobject_update_task,
    walletobjects_media_download_builder, walletobjects_media_download_task,
    walletobjects_media_upload_builder, walletobjects_media_upload_task,
    walletobjects_offerclass_addmessage_builder, walletobjects_offerclass_addmessage_task,
    walletobjects_offerclass_get_builder, walletobjects_offerclass_get_task,
    walletobjects_offerclass_insert_builder, walletobjects_offerclass_insert_task,
    walletobjects_offerclass_list_builder, walletobjects_offerclass_list_task,
    walletobjects_offerclass_patch_builder, walletobjects_offerclass_patch_task,
    walletobjects_offerclass_update_builder, walletobjects_offerclass_update_task,
    walletobjects_offerobject_addmessage_builder, walletobjects_offerobject_addmessage_task,
    walletobjects_offerobject_get_builder, walletobjects_offerobject_get_task,
    walletobjects_offerobject_insert_builder, walletobjects_offerobject_insert_task,
    walletobjects_offerobject_list_builder, walletobjects_offerobject_list_task,
    walletobjects_offerobject_patch_builder, walletobjects_offerobject_patch_task,
    walletobjects_offerobject_update_builder, walletobjects_offerobject_update_task,
    walletobjects_permissions_get_builder, walletobjects_permissions_get_task,
    walletobjects_permissions_update_builder, walletobjects_permissions_update_task,
    walletobjects_smarttap_insert_builder, walletobjects_smarttap_insert_task,
    walletobjects_transitclass_addmessage_builder, walletobjects_transitclass_addmessage_task,
    walletobjects_transitclass_get_builder, walletobjects_transitclass_get_task,
    walletobjects_transitclass_insert_builder, walletobjects_transitclass_insert_task,
    walletobjects_transitclass_list_builder, walletobjects_transitclass_list_task,
    walletobjects_transitclass_patch_builder, walletobjects_transitclass_patch_task,
    walletobjects_transitclass_update_builder, walletobjects_transitclass_update_task,
    walletobjects_transitobject_addmessage_builder, walletobjects_transitobject_addmessage_task,
    walletobjects_transitobject_get_builder, walletobjects_transitobject_get_task,
    walletobjects_transitobject_insert_builder, walletobjects_transitobject_insert_task,
    walletobjects_transitobject_list_builder, walletobjects_transitobject_list_task,
    walletobjects_transitobject_patch_builder, walletobjects_transitobject_patch_task,
    walletobjects_transitobject_update_builder, walletobjects_transitobject_update_task,
    walletobjects_walletobjects_v1_private_content_set_pass_update_notice_builder, walletobjects_walletobjects_v1_private_content_set_pass_update_notice_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::walletobjects::EventTicketClass;
use crate::providers::gcp::clients::walletobjects::EventTicketClassAddMessageResponse;
use crate::providers::gcp::clients::walletobjects::EventTicketClassListResponse;
use crate::providers::gcp::clients::walletobjects::EventTicketObject;
use crate::providers::gcp::clients::walletobjects::EventTicketObjectAddMessageResponse;
use crate::providers::gcp::clients::walletobjects::EventTicketObjectListResponse;
use crate::providers::gcp::clients::walletobjects::FlightClass;
use crate::providers::gcp::clients::walletobjects::FlightClassAddMessageResponse;
use crate::providers::gcp::clients::walletobjects::FlightClassListResponse;
use crate::providers::gcp::clients::walletobjects::FlightObject;
use crate::providers::gcp::clients::walletobjects::FlightObjectAddMessageResponse;
use crate::providers::gcp::clients::walletobjects::FlightObjectListResponse;
use crate::providers::gcp::clients::walletobjects::GenericClass;
use crate::providers::gcp::clients::walletobjects::GenericClassAddMessageResponse;
use crate::providers::gcp::clients::walletobjects::GenericClassListResponse;
use crate::providers::gcp::clients::walletobjects::GenericObject;
use crate::providers::gcp::clients::walletobjects::GenericObjectAddMessageResponse;
use crate::providers::gcp::clients::walletobjects::GenericObjectListResponse;
use crate::providers::gcp::clients::walletobjects::GiftCardClass;
use crate::providers::gcp::clients::walletobjects::GiftCardClassAddMessageResponse;
use crate::providers::gcp::clients::walletobjects::GiftCardClassListResponse;
use crate::providers::gcp::clients::walletobjects::GiftCardObject;
use crate::providers::gcp::clients::walletobjects::GiftCardObjectAddMessageResponse;
use crate::providers::gcp::clients::walletobjects::GiftCardObjectListResponse;
use crate::providers::gcp::clients::walletobjects::Issuer;
use crate::providers::gcp::clients::walletobjects::IssuerListResponse;
use crate::providers::gcp::clients::walletobjects::JwtInsertResponse;
use crate::providers::gcp::clients::walletobjects::LoyaltyClass;
use crate::providers::gcp::clients::walletobjects::LoyaltyClassAddMessageResponse;
use crate::providers::gcp::clients::walletobjects::LoyaltyClassListResponse;
use crate::providers::gcp::clients::walletobjects::LoyaltyObject;
use crate::providers::gcp::clients::walletobjects::LoyaltyObjectAddMessageResponse;
use crate::providers::gcp::clients::walletobjects::LoyaltyObjectListResponse;
use crate::providers::gcp::clients::walletobjects::Media;
use crate::providers::gcp::clients::walletobjects::OfferClass;
use crate::providers::gcp::clients::walletobjects::OfferClassAddMessageResponse;
use crate::providers::gcp::clients::walletobjects::OfferClassListResponse;
use crate::providers::gcp::clients::walletobjects::OfferObject;
use crate::providers::gcp::clients::walletobjects::OfferObjectAddMessageResponse;
use crate::providers::gcp::clients::walletobjects::OfferObjectListResponse;
use crate::providers::gcp::clients::walletobjects::Permissions;
use crate::providers::gcp::clients::walletobjects::SetPassUpdateNoticeResponse;
use crate::providers::gcp::clients::walletobjects::SmartTap;
use crate::providers::gcp::clients::walletobjects::TransitClass;
use crate::providers::gcp::clients::walletobjects::TransitClassAddMessageResponse;
use crate::providers::gcp::clients::walletobjects::TransitClassListResponse;
use crate::providers::gcp::clients::walletobjects::TransitObject;
use crate::providers::gcp::clients::walletobjects::TransitObjectAddMessageResponse;
use crate::providers::gcp::clients::walletobjects::TransitObjectListResponse;
use crate::providers::gcp::clients::walletobjects::TransitObjectUploadRotatingBarcodeValuesResponse;
use crate::providers::gcp::clients::walletobjects::WalletobjectsEventticketclassAddmessageArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsEventticketclassGetArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsEventticketclassInsertArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsEventticketclassListArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsEventticketclassPatchArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsEventticketclassUpdateArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsEventticketobjectAddmessageArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsEventticketobjectGetArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsEventticketobjectInsertArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsEventticketobjectListArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsEventticketobjectModifylinkedofferobjectsArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsEventticketobjectPatchArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsEventticketobjectUpdateArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsFlightclassAddmessageArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsFlightclassGetArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsFlightclassInsertArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsFlightclassListArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsFlightclassPatchArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsFlightclassUpdateArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsFlightobjectAddmessageArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsFlightobjectGetArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsFlightobjectInsertArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsFlightobjectListArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsFlightobjectPatchArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsFlightobjectUpdateArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsGenericclassAddmessageArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsGenericclassGetArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsGenericclassInsertArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsGenericclassListArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsGenericclassPatchArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsGenericclassUpdateArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsGenericobjectAddmessageArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsGenericobjectGetArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsGenericobjectInsertArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsGenericobjectListArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsGenericobjectPatchArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsGenericobjectUpdateArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsGiftcardclassAddmessageArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsGiftcardclassGetArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsGiftcardclassInsertArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsGiftcardclassListArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsGiftcardclassPatchArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsGiftcardclassUpdateArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsGiftcardobjectAddmessageArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsGiftcardobjectGetArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsGiftcardobjectInsertArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsGiftcardobjectListArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsGiftcardobjectPatchArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsGiftcardobjectUpdateArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsIssuerGetArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsIssuerInsertArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsIssuerListArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsIssuerPatchArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsIssuerUpdateArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsJwtInsertArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsLoyaltyclassAddmessageArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsLoyaltyclassGetArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsLoyaltyclassInsertArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsLoyaltyclassListArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsLoyaltyclassPatchArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsLoyaltyclassUpdateArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsLoyaltyobjectAddmessageArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsLoyaltyobjectGetArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsLoyaltyobjectInsertArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsLoyaltyobjectListArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsLoyaltyobjectModifylinkedofferobjectsArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsLoyaltyobjectPatchArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsLoyaltyobjectUpdateArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsMediaDownloadArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsMediaUploadArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsOfferclassAddmessageArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsOfferclassGetArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsOfferclassInsertArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsOfferclassListArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsOfferclassPatchArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsOfferclassUpdateArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsOfferobjectAddmessageArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsOfferobjectGetArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsOfferobjectInsertArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsOfferobjectListArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsOfferobjectPatchArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsOfferobjectUpdateArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsPermissionsGetArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsPermissionsUpdateArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsSmarttapInsertArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsTransitclassAddmessageArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsTransitclassGetArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsTransitclassInsertArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsTransitclassListArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsTransitclassPatchArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsTransitclassUpdateArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsTransitobjectAddmessageArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsTransitobjectGetArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsTransitobjectInsertArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsTransitobjectListArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsTransitobjectPatchArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsTransitobjectUpdateArgs;
use crate::providers::gcp::clients::walletobjects::WalletobjectsWalletobjectsV1PrivateContentSetPassUpdateNoticeArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// WalletobjectsProvider with automatic state tracking.
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
/// let provider = WalletobjectsProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct WalletobjectsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> WalletobjectsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new WalletobjectsProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Walletobjects eventticketclass addmessage.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EventTicketClassAddMessageResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_eventticketclass_addmessage(
        &self,
        args: &WalletobjectsEventticketclassAddmessageArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EventTicketClassAddMessageResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_eventticketclass_addmessage_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_eventticketclass_addmessage_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects eventticketclass get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EventTicketClass result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn walletobjects_eventticketclass_get(
        &self,
        args: &WalletobjectsEventticketclassGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EventTicketClass, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_eventticketclass_get_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_eventticketclass_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects eventticketclass insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EventTicketClass result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_eventticketclass_insert(
        &self,
        args: &WalletobjectsEventticketclassInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EventTicketClass, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_eventticketclass_insert_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_eventticketclass_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects eventticketclass list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EventTicketClassListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn walletobjects_eventticketclass_list(
        &self,
        args: &WalletobjectsEventticketclassListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EventTicketClassListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_eventticketclass_list_builder(
            &self.http_client,
            &args.issuerId,
            &args.maxResults,
            &args.token,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_eventticketclass_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects eventticketclass patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EventTicketClass result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_eventticketclass_patch(
        &self,
        args: &WalletobjectsEventticketclassPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EventTicketClass, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_eventticketclass_patch_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_eventticketclass_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects eventticketclass update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EventTicketClass result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_eventticketclass_update(
        &self,
        args: &WalletobjectsEventticketclassUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EventTicketClass, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_eventticketclass_update_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_eventticketclass_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects eventticketobject addmessage.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EventTicketObjectAddMessageResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_eventticketobject_addmessage(
        &self,
        args: &WalletobjectsEventticketobjectAddmessageArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EventTicketObjectAddMessageResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_eventticketobject_addmessage_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_eventticketobject_addmessage_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects eventticketobject get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EventTicketObject result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn walletobjects_eventticketobject_get(
        &self,
        args: &WalletobjectsEventticketobjectGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EventTicketObject, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_eventticketobject_get_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_eventticketobject_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects eventticketobject insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EventTicketObject result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_eventticketobject_insert(
        &self,
        args: &WalletobjectsEventticketobjectInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EventTicketObject, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_eventticketobject_insert_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_eventticketobject_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects eventticketobject list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EventTicketObjectListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn walletobjects_eventticketobject_list(
        &self,
        args: &WalletobjectsEventticketobjectListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EventTicketObjectListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_eventticketobject_list_builder(
            &self.http_client,
            &args.classId,
            &args.maxResults,
            &args.token,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_eventticketobject_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects eventticketobject modifylinkedofferobjects.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EventTicketObject result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_eventticketobject_modifylinkedofferobjects(
        &self,
        args: &WalletobjectsEventticketobjectModifylinkedofferobjectsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EventTicketObject, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_eventticketobject_modifylinkedofferobjects_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_eventticketobject_modifylinkedofferobjects_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects eventticketobject patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EventTicketObject result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_eventticketobject_patch(
        &self,
        args: &WalletobjectsEventticketobjectPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EventTicketObject, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_eventticketobject_patch_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_eventticketobject_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects eventticketobject update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EventTicketObject result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_eventticketobject_update(
        &self,
        args: &WalletobjectsEventticketobjectUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EventTicketObject, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_eventticketobject_update_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_eventticketobject_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects flightclass addmessage.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FlightClassAddMessageResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_flightclass_addmessage(
        &self,
        args: &WalletobjectsFlightclassAddmessageArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FlightClassAddMessageResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_flightclass_addmessage_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_flightclass_addmessage_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects flightclass get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FlightClass result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn walletobjects_flightclass_get(
        &self,
        args: &WalletobjectsFlightclassGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FlightClass, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_flightclass_get_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_flightclass_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects flightclass insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FlightClass result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_flightclass_insert(
        &self,
        args: &WalletobjectsFlightclassInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FlightClass, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_flightclass_insert_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_flightclass_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects flightclass list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FlightClassListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn walletobjects_flightclass_list(
        &self,
        args: &WalletobjectsFlightclassListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FlightClassListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_flightclass_list_builder(
            &self.http_client,
            &args.issuerId,
            &args.maxResults,
            &args.token,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_flightclass_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects flightclass patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FlightClass result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_flightclass_patch(
        &self,
        args: &WalletobjectsFlightclassPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FlightClass, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_flightclass_patch_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_flightclass_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects flightclass update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FlightClass result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_flightclass_update(
        &self,
        args: &WalletobjectsFlightclassUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FlightClass, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_flightclass_update_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_flightclass_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects flightobject addmessage.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FlightObjectAddMessageResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_flightobject_addmessage(
        &self,
        args: &WalletobjectsFlightobjectAddmessageArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FlightObjectAddMessageResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_flightobject_addmessage_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_flightobject_addmessage_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects flightobject get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FlightObject result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn walletobjects_flightobject_get(
        &self,
        args: &WalletobjectsFlightobjectGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FlightObject, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_flightobject_get_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_flightobject_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects flightobject insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FlightObject result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_flightobject_insert(
        &self,
        args: &WalletobjectsFlightobjectInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FlightObject, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_flightobject_insert_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_flightobject_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects flightobject list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FlightObjectListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn walletobjects_flightobject_list(
        &self,
        args: &WalletobjectsFlightobjectListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FlightObjectListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_flightobject_list_builder(
            &self.http_client,
            &args.classId,
            &args.maxResults,
            &args.token,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_flightobject_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects flightobject patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FlightObject result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_flightobject_patch(
        &self,
        args: &WalletobjectsFlightobjectPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FlightObject, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_flightobject_patch_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_flightobject_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects flightobject update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FlightObject result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_flightobject_update(
        &self,
        args: &WalletobjectsFlightobjectUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FlightObject, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_flightobject_update_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_flightobject_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects genericclass addmessage.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GenericClassAddMessageResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_genericclass_addmessage(
        &self,
        args: &WalletobjectsGenericclassAddmessageArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GenericClassAddMessageResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_genericclass_addmessage_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_genericclass_addmessage_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects genericclass get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GenericClass result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn walletobjects_genericclass_get(
        &self,
        args: &WalletobjectsGenericclassGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GenericClass, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_genericclass_get_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_genericclass_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects genericclass insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GenericClass result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_genericclass_insert(
        &self,
        args: &WalletobjectsGenericclassInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GenericClass, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_genericclass_insert_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_genericclass_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects genericclass list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GenericClassListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn walletobjects_genericclass_list(
        &self,
        args: &WalletobjectsGenericclassListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GenericClassListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_genericclass_list_builder(
            &self.http_client,
            &args.issuerId,
            &args.maxResults,
            &args.token,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_genericclass_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects genericclass patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GenericClass result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_genericclass_patch(
        &self,
        args: &WalletobjectsGenericclassPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GenericClass, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_genericclass_patch_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_genericclass_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects genericclass update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GenericClass result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_genericclass_update(
        &self,
        args: &WalletobjectsGenericclassUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GenericClass, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_genericclass_update_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_genericclass_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects genericobject addmessage.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GenericObjectAddMessageResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_genericobject_addmessage(
        &self,
        args: &WalletobjectsGenericobjectAddmessageArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GenericObjectAddMessageResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_genericobject_addmessage_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_genericobject_addmessage_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects genericobject get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GenericObject result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn walletobjects_genericobject_get(
        &self,
        args: &WalletobjectsGenericobjectGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GenericObject, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_genericobject_get_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_genericobject_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects genericobject insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GenericObject result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_genericobject_insert(
        &self,
        args: &WalletobjectsGenericobjectInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GenericObject, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_genericobject_insert_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_genericobject_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects genericobject list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GenericObjectListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn walletobjects_genericobject_list(
        &self,
        args: &WalletobjectsGenericobjectListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GenericObjectListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_genericobject_list_builder(
            &self.http_client,
            &args.classId,
            &args.maxResults,
            &args.token,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_genericobject_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects genericobject patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GenericObject result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_genericobject_patch(
        &self,
        args: &WalletobjectsGenericobjectPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GenericObject, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_genericobject_patch_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_genericobject_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects genericobject update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GenericObject result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_genericobject_update(
        &self,
        args: &WalletobjectsGenericobjectUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GenericObject, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_genericobject_update_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_genericobject_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects giftcardclass addmessage.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GiftCardClassAddMessageResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_giftcardclass_addmessage(
        &self,
        args: &WalletobjectsGiftcardclassAddmessageArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GiftCardClassAddMessageResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_giftcardclass_addmessage_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_giftcardclass_addmessage_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects giftcardclass get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GiftCardClass result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn walletobjects_giftcardclass_get(
        &self,
        args: &WalletobjectsGiftcardclassGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GiftCardClass, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_giftcardclass_get_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_giftcardclass_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects giftcardclass insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GiftCardClass result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_giftcardclass_insert(
        &self,
        args: &WalletobjectsGiftcardclassInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GiftCardClass, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_giftcardclass_insert_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_giftcardclass_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects giftcardclass list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GiftCardClassListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn walletobjects_giftcardclass_list(
        &self,
        args: &WalletobjectsGiftcardclassListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GiftCardClassListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_giftcardclass_list_builder(
            &self.http_client,
            &args.issuerId,
            &args.maxResults,
            &args.token,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_giftcardclass_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects giftcardclass patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GiftCardClass result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_giftcardclass_patch(
        &self,
        args: &WalletobjectsGiftcardclassPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GiftCardClass, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_giftcardclass_patch_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_giftcardclass_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects giftcardclass update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GiftCardClass result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_giftcardclass_update(
        &self,
        args: &WalletobjectsGiftcardclassUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GiftCardClass, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_giftcardclass_update_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_giftcardclass_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects giftcardobject addmessage.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GiftCardObjectAddMessageResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_giftcardobject_addmessage(
        &self,
        args: &WalletobjectsGiftcardobjectAddmessageArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GiftCardObjectAddMessageResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_giftcardobject_addmessage_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_giftcardobject_addmessage_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects giftcardobject get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GiftCardObject result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn walletobjects_giftcardobject_get(
        &self,
        args: &WalletobjectsGiftcardobjectGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GiftCardObject, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_giftcardobject_get_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_giftcardobject_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects giftcardobject insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GiftCardObject result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_giftcardobject_insert(
        &self,
        args: &WalletobjectsGiftcardobjectInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GiftCardObject, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_giftcardobject_insert_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_giftcardobject_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects giftcardobject list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GiftCardObjectListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn walletobjects_giftcardobject_list(
        &self,
        args: &WalletobjectsGiftcardobjectListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GiftCardObjectListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_giftcardobject_list_builder(
            &self.http_client,
            &args.classId,
            &args.maxResults,
            &args.token,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_giftcardobject_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects giftcardobject patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GiftCardObject result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_giftcardobject_patch(
        &self,
        args: &WalletobjectsGiftcardobjectPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GiftCardObject, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_giftcardobject_patch_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_giftcardobject_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects giftcardobject update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GiftCardObject result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_giftcardobject_update(
        &self,
        args: &WalletobjectsGiftcardobjectUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GiftCardObject, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_giftcardobject_update_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_giftcardobject_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects issuer get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Issuer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn walletobjects_issuer_get(
        &self,
        args: &WalletobjectsIssuerGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Issuer, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_issuer_get_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_issuer_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects issuer insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Issuer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_issuer_insert(
        &self,
        args: &WalletobjectsIssuerInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Issuer, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_issuer_insert_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_issuer_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects issuer list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IssuerListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn walletobjects_issuer_list(
        &self,
        args: &WalletobjectsIssuerListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IssuerListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_issuer_list_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_issuer_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects issuer patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Issuer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_issuer_patch(
        &self,
        args: &WalletobjectsIssuerPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Issuer, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_issuer_patch_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_issuer_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects issuer update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Issuer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_issuer_update(
        &self,
        args: &WalletobjectsIssuerUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Issuer, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_issuer_update_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_issuer_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects jwt insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the JwtInsertResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_jwt_insert(
        &self,
        args: &WalletobjectsJwtInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<JwtInsertResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_jwt_insert_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_jwt_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects loyaltyclass addmessage.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LoyaltyClassAddMessageResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_loyaltyclass_addmessage(
        &self,
        args: &WalletobjectsLoyaltyclassAddmessageArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LoyaltyClassAddMessageResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_loyaltyclass_addmessage_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_loyaltyclass_addmessage_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects loyaltyclass get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LoyaltyClass result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn walletobjects_loyaltyclass_get(
        &self,
        args: &WalletobjectsLoyaltyclassGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LoyaltyClass, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_loyaltyclass_get_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_loyaltyclass_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects loyaltyclass insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LoyaltyClass result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_loyaltyclass_insert(
        &self,
        args: &WalletobjectsLoyaltyclassInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LoyaltyClass, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_loyaltyclass_insert_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_loyaltyclass_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects loyaltyclass list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LoyaltyClassListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn walletobjects_loyaltyclass_list(
        &self,
        args: &WalletobjectsLoyaltyclassListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LoyaltyClassListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_loyaltyclass_list_builder(
            &self.http_client,
            &args.issuerId,
            &args.maxResults,
            &args.token,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_loyaltyclass_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects loyaltyclass patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LoyaltyClass result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_loyaltyclass_patch(
        &self,
        args: &WalletobjectsLoyaltyclassPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LoyaltyClass, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_loyaltyclass_patch_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_loyaltyclass_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects loyaltyclass update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LoyaltyClass result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_loyaltyclass_update(
        &self,
        args: &WalletobjectsLoyaltyclassUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LoyaltyClass, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_loyaltyclass_update_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_loyaltyclass_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects loyaltyobject addmessage.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LoyaltyObjectAddMessageResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_loyaltyobject_addmessage(
        &self,
        args: &WalletobjectsLoyaltyobjectAddmessageArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LoyaltyObjectAddMessageResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_loyaltyobject_addmessage_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_loyaltyobject_addmessage_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects loyaltyobject get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LoyaltyObject result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn walletobjects_loyaltyobject_get(
        &self,
        args: &WalletobjectsLoyaltyobjectGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LoyaltyObject, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_loyaltyobject_get_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_loyaltyobject_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects loyaltyobject insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LoyaltyObject result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_loyaltyobject_insert(
        &self,
        args: &WalletobjectsLoyaltyobjectInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LoyaltyObject, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_loyaltyobject_insert_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_loyaltyobject_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects loyaltyobject list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LoyaltyObjectListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn walletobjects_loyaltyobject_list(
        &self,
        args: &WalletobjectsLoyaltyobjectListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LoyaltyObjectListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_loyaltyobject_list_builder(
            &self.http_client,
            &args.classId,
            &args.maxResults,
            &args.token,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_loyaltyobject_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects loyaltyobject modifylinkedofferobjects.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LoyaltyObject result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_loyaltyobject_modifylinkedofferobjects(
        &self,
        args: &WalletobjectsLoyaltyobjectModifylinkedofferobjectsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LoyaltyObject, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_loyaltyobject_modifylinkedofferobjects_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_loyaltyobject_modifylinkedofferobjects_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects loyaltyobject patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LoyaltyObject result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_loyaltyobject_patch(
        &self,
        args: &WalletobjectsLoyaltyobjectPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LoyaltyObject, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_loyaltyobject_patch_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_loyaltyobject_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects loyaltyobject update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LoyaltyObject result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_loyaltyobject_update(
        &self,
        args: &WalletobjectsLoyaltyobjectUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LoyaltyObject, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_loyaltyobject_update_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_loyaltyobject_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects media download.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Media result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn walletobjects_media_download(
        &self,
        args: &WalletobjectsMediaDownloadArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Media, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_media_download_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_media_download_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects media upload.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TransitObjectUploadRotatingBarcodeValuesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_media_upload(
        &self,
        args: &WalletobjectsMediaUploadArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TransitObjectUploadRotatingBarcodeValuesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_media_upload_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_media_upload_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects offerclass addmessage.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the OfferClassAddMessageResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_offerclass_addmessage(
        &self,
        args: &WalletobjectsOfferclassAddmessageArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<OfferClassAddMessageResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_offerclass_addmessage_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_offerclass_addmessage_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects offerclass get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the OfferClass result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn walletobjects_offerclass_get(
        &self,
        args: &WalletobjectsOfferclassGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<OfferClass, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_offerclass_get_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_offerclass_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects offerclass insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the OfferClass result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_offerclass_insert(
        &self,
        args: &WalletobjectsOfferclassInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<OfferClass, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_offerclass_insert_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_offerclass_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects offerclass list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the OfferClassListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn walletobjects_offerclass_list(
        &self,
        args: &WalletobjectsOfferclassListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<OfferClassListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_offerclass_list_builder(
            &self.http_client,
            &args.issuerId,
            &args.maxResults,
            &args.token,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_offerclass_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects offerclass patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the OfferClass result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_offerclass_patch(
        &self,
        args: &WalletobjectsOfferclassPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<OfferClass, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_offerclass_patch_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_offerclass_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects offerclass update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the OfferClass result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_offerclass_update(
        &self,
        args: &WalletobjectsOfferclassUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<OfferClass, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_offerclass_update_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_offerclass_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects offerobject addmessage.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the OfferObjectAddMessageResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_offerobject_addmessage(
        &self,
        args: &WalletobjectsOfferobjectAddmessageArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<OfferObjectAddMessageResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_offerobject_addmessage_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_offerobject_addmessage_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects offerobject get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the OfferObject result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn walletobjects_offerobject_get(
        &self,
        args: &WalletobjectsOfferobjectGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<OfferObject, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_offerobject_get_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_offerobject_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects offerobject insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the OfferObject result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_offerobject_insert(
        &self,
        args: &WalletobjectsOfferobjectInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<OfferObject, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_offerobject_insert_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_offerobject_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects offerobject list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the OfferObjectListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn walletobjects_offerobject_list(
        &self,
        args: &WalletobjectsOfferobjectListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<OfferObjectListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_offerobject_list_builder(
            &self.http_client,
            &args.classId,
            &args.maxResults,
            &args.token,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_offerobject_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects offerobject patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the OfferObject result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_offerobject_patch(
        &self,
        args: &WalletobjectsOfferobjectPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<OfferObject, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_offerobject_patch_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_offerobject_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects offerobject update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the OfferObject result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_offerobject_update(
        &self,
        args: &WalletobjectsOfferobjectUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<OfferObject, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_offerobject_update_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_offerobject_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects permissions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Permissions result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn walletobjects_permissions_get(
        &self,
        args: &WalletobjectsPermissionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Permissions, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_permissions_get_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_permissions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects permissions update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Permissions result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_permissions_update(
        &self,
        args: &WalletobjectsPermissionsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Permissions, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_permissions_update_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_permissions_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects smarttap insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SmartTap result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_smarttap_insert(
        &self,
        args: &WalletobjectsSmarttapInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SmartTap, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_smarttap_insert_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_smarttap_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects transitclass addmessage.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TransitClassAddMessageResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_transitclass_addmessage(
        &self,
        args: &WalletobjectsTransitclassAddmessageArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TransitClassAddMessageResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_transitclass_addmessage_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_transitclass_addmessage_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects transitclass get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TransitClass result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn walletobjects_transitclass_get(
        &self,
        args: &WalletobjectsTransitclassGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TransitClass, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_transitclass_get_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_transitclass_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects transitclass insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TransitClass result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_transitclass_insert(
        &self,
        args: &WalletobjectsTransitclassInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TransitClass, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_transitclass_insert_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_transitclass_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects transitclass list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TransitClassListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn walletobjects_transitclass_list(
        &self,
        args: &WalletobjectsTransitclassListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TransitClassListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_transitclass_list_builder(
            &self.http_client,
            &args.issuerId,
            &args.maxResults,
            &args.token,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_transitclass_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects transitclass patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TransitClass result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_transitclass_patch(
        &self,
        args: &WalletobjectsTransitclassPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TransitClass, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_transitclass_patch_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_transitclass_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects transitclass update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TransitClass result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_transitclass_update(
        &self,
        args: &WalletobjectsTransitclassUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TransitClass, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_transitclass_update_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_transitclass_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects transitobject addmessage.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TransitObjectAddMessageResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_transitobject_addmessage(
        &self,
        args: &WalletobjectsTransitobjectAddmessageArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TransitObjectAddMessageResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_transitobject_addmessage_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_transitobject_addmessage_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects transitobject get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TransitObject result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn walletobjects_transitobject_get(
        &self,
        args: &WalletobjectsTransitobjectGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TransitObject, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_transitobject_get_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_transitobject_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects transitobject insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TransitObject result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_transitobject_insert(
        &self,
        args: &WalletobjectsTransitobjectInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TransitObject, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_transitobject_insert_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_transitobject_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects transitobject list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TransitObjectListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn walletobjects_transitobject_list(
        &self,
        args: &WalletobjectsTransitobjectListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TransitObjectListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_transitobject_list_builder(
            &self.http_client,
            &args.classId,
            &args.maxResults,
            &args.token,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_transitobject_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects transitobject patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TransitObject result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_transitobject_patch(
        &self,
        args: &WalletobjectsTransitobjectPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TransitObject, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_transitobject_patch_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_transitobject_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects transitobject update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TransitObject result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_transitobject_update(
        &self,
        args: &WalletobjectsTransitobjectUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TransitObject, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_transitobject_update_builder(
            &self.http_client,
            &args.resourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_transitobject_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Walletobjects walletobjects v1 private content set pass update notice.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SetPassUpdateNoticeResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn walletobjects_walletobjects_v1_private_content_set_pass_update_notice(
        &self,
        args: &WalletobjectsWalletobjectsV1PrivateContentSetPassUpdateNoticeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SetPassUpdateNoticeResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = walletobjects_walletobjects_v1_private_content_set_pass_update_notice_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = walletobjects_walletobjects_v1_private_content_set_pass_update_notice_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
