//! AuthorizedbuyersmarketplaceProvider - State-aware authorizedbuyersmarketplace API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       authorizedbuyersmarketplace API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::authorizedbuyersmarketplace::{
    authorizedbuyersmarketplace_bidders_auction_packages_list_builder, authorizedbuyersmarketplace_bidders_auction_packages_list_task,
    authorizedbuyersmarketplace_bidders_finalized_deals_list_builder, authorizedbuyersmarketplace_bidders_finalized_deals_list_task,
    authorizedbuyersmarketplace_bidders_finalized_deals_set_ready_to_serve_builder, authorizedbuyersmarketplace_bidders_finalized_deals_set_ready_to_serve_task,
    authorizedbuyersmarketplace_buyers_auction_packages_get_builder, authorizedbuyersmarketplace_buyers_auction_packages_get_task,
    authorizedbuyersmarketplace_buyers_auction_packages_list_builder, authorizedbuyersmarketplace_buyers_auction_packages_list_task,
    authorizedbuyersmarketplace_buyers_auction_packages_subscribe_builder, authorizedbuyersmarketplace_buyers_auction_packages_subscribe_task,
    authorizedbuyersmarketplace_buyers_auction_packages_subscribe_clients_builder, authorizedbuyersmarketplace_buyers_auction_packages_subscribe_clients_task,
    authorizedbuyersmarketplace_buyers_auction_packages_unsubscribe_builder, authorizedbuyersmarketplace_buyers_auction_packages_unsubscribe_task,
    authorizedbuyersmarketplace_buyers_auction_packages_unsubscribe_clients_builder, authorizedbuyersmarketplace_buyers_auction_packages_unsubscribe_clients_task,
    authorizedbuyersmarketplace_buyers_clients_activate_builder, authorizedbuyersmarketplace_buyers_clients_activate_task,
    authorizedbuyersmarketplace_buyers_clients_create_builder, authorizedbuyersmarketplace_buyers_clients_create_task,
    authorizedbuyersmarketplace_buyers_clients_deactivate_builder, authorizedbuyersmarketplace_buyers_clients_deactivate_task,
    authorizedbuyersmarketplace_buyers_clients_get_builder, authorizedbuyersmarketplace_buyers_clients_get_task,
    authorizedbuyersmarketplace_buyers_clients_list_builder, authorizedbuyersmarketplace_buyers_clients_list_task,
    authorizedbuyersmarketplace_buyers_clients_patch_builder, authorizedbuyersmarketplace_buyers_clients_patch_task,
    authorizedbuyersmarketplace_buyers_clients_users_activate_builder, authorizedbuyersmarketplace_buyers_clients_users_activate_task,
    authorizedbuyersmarketplace_buyers_clients_users_create_builder, authorizedbuyersmarketplace_buyers_clients_users_create_task,
    authorizedbuyersmarketplace_buyers_clients_users_deactivate_builder, authorizedbuyersmarketplace_buyers_clients_users_deactivate_task,
    authorizedbuyersmarketplace_buyers_clients_users_delete_builder, authorizedbuyersmarketplace_buyers_clients_users_delete_task,
    authorizedbuyersmarketplace_buyers_clients_users_get_builder, authorizedbuyersmarketplace_buyers_clients_users_get_task,
    authorizedbuyersmarketplace_buyers_clients_users_list_builder, authorizedbuyersmarketplace_buyers_clients_users_list_task,
    authorizedbuyersmarketplace_buyers_finalized_deals_add_creative_builder, authorizedbuyersmarketplace_buyers_finalized_deals_add_creative_task,
    authorizedbuyersmarketplace_buyers_finalized_deals_get_builder, authorizedbuyersmarketplace_buyers_finalized_deals_get_task,
    authorizedbuyersmarketplace_buyers_finalized_deals_list_builder, authorizedbuyersmarketplace_buyers_finalized_deals_list_task,
    authorizedbuyersmarketplace_buyers_finalized_deals_pause_builder, authorizedbuyersmarketplace_buyers_finalized_deals_pause_task,
    authorizedbuyersmarketplace_buyers_finalized_deals_resume_builder, authorizedbuyersmarketplace_buyers_finalized_deals_resume_task,
    authorizedbuyersmarketplace_buyers_finalized_deals_set_ready_to_serve_builder, authorizedbuyersmarketplace_buyers_finalized_deals_set_ready_to_serve_task,
    authorizedbuyersmarketplace_buyers_proposals_accept_builder, authorizedbuyersmarketplace_buyers_proposals_accept_task,
    authorizedbuyersmarketplace_buyers_proposals_add_note_builder, authorizedbuyersmarketplace_buyers_proposals_add_note_task,
    authorizedbuyersmarketplace_buyers_proposals_cancel_negotiation_builder, authorizedbuyersmarketplace_buyers_proposals_cancel_negotiation_task,
    authorizedbuyersmarketplace_buyers_proposals_get_builder, authorizedbuyersmarketplace_buyers_proposals_get_task,
    authorizedbuyersmarketplace_buyers_proposals_list_builder, authorizedbuyersmarketplace_buyers_proposals_list_task,
    authorizedbuyersmarketplace_buyers_proposals_patch_builder, authorizedbuyersmarketplace_buyers_proposals_patch_task,
    authorizedbuyersmarketplace_buyers_proposals_send_rfp_builder, authorizedbuyersmarketplace_buyers_proposals_send_rfp_task,
    authorizedbuyersmarketplace_buyers_proposals_deals_batch_update_builder, authorizedbuyersmarketplace_buyers_proposals_deals_batch_update_task,
    authorizedbuyersmarketplace_buyers_proposals_deals_get_builder, authorizedbuyersmarketplace_buyers_proposals_deals_get_task,
    authorizedbuyersmarketplace_buyers_proposals_deals_list_builder, authorizedbuyersmarketplace_buyers_proposals_deals_list_task,
    authorizedbuyersmarketplace_buyers_proposals_deals_patch_builder, authorizedbuyersmarketplace_buyers_proposals_deals_patch_task,
    authorizedbuyersmarketplace_buyers_publisher_profiles_get_builder, authorizedbuyersmarketplace_buyers_publisher_profiles_get_task,
    authorizedbuyersmarketplace_buyers_publisher_profiles_list_builder, authorizedbuyersmarketplace_buyers_publisher_profiles_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::authorizedbuyersmarketplace::AuctionPackage;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::BatchUpdateDealsResponse;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::Client;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::ClientUser;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::Deal;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::Empty;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::FinalizedDeal;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::ListAuctionPackagesResponse;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::ListClientUsersResponse;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::ListClientsResponse;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::ListDealsResponse;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::ListFinalizedDealsResponse;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::ListProposalsResponse;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::ListPublisherProfilesResponse;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::Proposal;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::PublisherProfile;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::AuthorizedbuyersmarketplaceBiddersAuctionPackagesListArgs;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::AuthorizedbuyersmarketplaceBiddersFinalizedDealsListArgs;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::AuthorizedbuyersmarketplaceBiddersFinalizedDealsSetReadyToServeArgs;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::AuthorizedbuyersmarketplaceBuyersAuctionPackagesGetArgs;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::AuthorizedbuyersmarketplaceBuyersAuctionPackagesListArgs;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::AuthorizedbuyersmarketplaceBuyersAuctionPackagesSubscribeArgs;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::AuthorizedbuyersmarketplaceBuyersAuctionPackagesSubscribeClientsArgs;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::AuthorizedbuyersmarketplaceBuyersAuctionPackagesUnsubscribeArgs;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::AuthorizedbuyersmarketplaceBuyersAuctionPackagesUnsubscribeClientsArgs;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::AuthorizedbuyersmarketplaceBuyersClientsActivateArgs;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::AuthorizedbuyersmarketplaceBuyersClientsCreateArgs;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::AuthorizedbuyersmarketplaceBuyersClientsDeactivateArgs;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::AuthorizedbuyersmarketplaceBuyersClientsGetArgs;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::AuthorizedbuyersmarketplaceBuyersClientsListArgs;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::AuthorizedbuyersmarketplaceBuyersClientsPatchArgs;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::AuthorizedbuyersmarketplaceBuyersClientsUsersActivateArgs;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::AuthorizedbuyersmarketplaceBuyersClientsUsersCreateArgs;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::AuthorizedbuyersmarketplaceBuyersClientsUsersDeactivateArgs;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::AuthorizedbuyersmarketplaceBuyersClientsUsersDeleteArgs;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::AuthorizedbuyersmarketplaceBuyersClientsUsersGetArgs;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::AuthorizedbuyersmarketplaceBuyersClientsUsersListArgs;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::AuthorizedbuyersmarketplaceBuyersFinalizedDealsAddCreativeArgs;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::AuthorizedbuyersmarketplaceBuyersFinalizedDealsGetArgs;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::AuthorizedbuyersmarketplaceBuyersFinalizedDealsListArgs;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::AuthorizedbuyersmarketplaceBuyersFinalizedDealsPauseArgs;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::AuthorizedbuyersmarketplaceBuyersFinalizedDealsResumeArgs;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::AuthorizedbuyersmarketplaceBuyersFinalizedDealsSetReadyToServeArgs;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::AuthorizedbuyersmarketplaceBuyersProposalsAcceptArgs;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::AuthorizedbuyersmarketplaceBuyersProposalsAddNoteArgs;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::AuthorizedbuyersmarketplaceBuyersProposalsCancelNegotiationArgs;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::AuthorizedbuyersmarketplaceBuyersProposalsDealsBatchUpdateArgs;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::AuthorizedbuyersmarketplaceBuyersProposalsDealsGetArgs;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::AuthorizedbuyersmarketplaceBuyersProposalsDealsListArgs;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::AuthorizedbuyersmarketplaceBuyersProposalsDealsPatchArgs;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::AuthorizedbuyersmarketplaceBuyersProposalsGetArgs;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::AuthorizedbuyersmarketplaceBuyersProposalsListArgs;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::AuthorizedbuyersmarketplaceBuyersProposalsPatchArgs;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::AuthorizedbuyersmarketplaceBuyersProposalsSendRfpArgs;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::AuthorizedbuyersmarketplaceBuyersPublisherProfilesGetArgs;
use crate::providers::gcp::clients::authorizedbuyersmarketplace::AuthorizedbuyersmarketplaceBuyersPublisherProfilesListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// AuthorizedbuyersmarketplaceProvider with automatic state tracking.
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
/// let provider = AuthorizedbuyersmarketplaceProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct AuthorizedbuyersmarketplaceProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> AuthorizedbuyersmarketplaceProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new AuthorizedbuyersmarketplaceProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Authorizedbuyersmarketplace bidders auction packages list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAuctionPackagesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn authorizedbuyersmarketplace_bidders_auction_packages_list(
        &self,
        args: &AuthorizedbuyersmarketplaceBiddersAuctionPackagesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAuctionPackagesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = authorizedbuyersmarketplace_bidders_auction_packages_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = authorizedbuyersmarketplace_bidders_auction_packages_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Authorizedbuyersmarketplace bidders finalized deals list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListFinalizedDealsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn authorizedbuyersmarketplace_bidders_finalized_deals_list(
        &self,
        args: &AuthorizedbuyersmarketplaceBiddersFinalizedDealsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListFinalizedDealsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = authorizedbuyersmarketplace_bidders_finalized_deals_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = authorizedbuyersmarketplace_bidders_finalized_deals_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Authorizedbuyersmarketplace bidders finalized deals set ready to serve.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FinalizedDeal result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn authorizedbuyersmarketplace_bidders_finalized_deals_set_ready_to_serve(
        &self,
        args: &AuthorizedbuyersmarketplaceBiddersFinalizedDealsSetReadyToServeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FinalizedDeal, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = authorizedbuyersmarketplace_bidders_finalized_deals_set_ready_to_serve_builder(
            &self.http_client,
            &args.deal,
        )
        .map_err(ProviderError::Api)?;

        let task = authorizedbuyersmarketplace_bidders_finalized_deals_set_ready_to_serve_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Authorizedbuyersmarketplace buyers auction packages get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AuctionPackage result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn authorizedbuyersmarketplace_buyers_auction_packages_get(
        &self,
        args: &AuthorizedbuyersmarketplaceBuyersAuctionPackagesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AuctionPackage, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = authorizedbuyersmarketplace_buyers_auction_packages_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = authorizedbuyersmarketplace_buyers_auction_packages_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Authorizedbuyersmarketplace buyers auction packages list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAuctionPackagesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn authorizedbuyersmarketplace_buyers_auction_packages_list(
        &self,
        args: &AuthorizedbuyersmarketplaceBuyersAuctionPackagesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAuctionPackagesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = authorizedbuyersmarketplace_buyers_auction_packages_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = authorizedbuyersmarketplace_buyers_auction_packages_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Authorizedbuyersmarketplace buyers auction packages subscribe.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AuctionPackage result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn authorizedbuyersmarketplace_buyers_auction_packages_subscribe(
        &self,
        args: &AuthorizedbuyersmarketplaceBuyersAuctionPackagesSubscribeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AuctionPackage, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = authorizedbuyersmarketplace_buyers_auction_packages_subscribe_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = authorizedbuyersmarketplace_buyers_auction_packages_subscribe_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Authorizedbuyersmarketplace buyers auction packages subscribe clients.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AuctionPackage result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn authorizedbuyersmarketplace_buyers_auction_packages_subscribe_clients(
        &self,
        args: &AuthorizedbuyersmarketplaceBuyersAuctionPackagesSubscribeClientsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AuctionPackage, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = authorizedbuyersmarketplace_buyers_auction_packages_subscribe_clients_builder(
            &self.http_client,
            &args.auctionPackage,
        )
        .map_err(ProviderError::Api)?;

        let task = authorizedbuyersmarketplace_buyers_auction_packages_subscribe_clients_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Authorizedbuyersmarketplace buyers auction packages unsubscribe.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AuctionPackage result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn authorizedbuyersmarketplace_buyers_auction_packages_unsubscribe(
        &self,
        args: &AuthorizedbuyersmarketplaceBuyersAuctionPackagesUnsubscribeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AuctionPackage, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = authorizedbuyersmarketplace_buyers_auction_packages_unsubscribe_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = authorizedbuyersmarketplace_buyers_auction_packages_unsubscribe_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Authorizedbuyersmarketplace buyers auction packages unsubscribe clients.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AuctionPackage result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn authorizedbuyersmarketplace_buyers_auction_packages_unsubscribe_clients(
        &self,
        args: &AuthorizedbuyersmarketplaceBuyersAuctionPackagesUnsubscribeClientsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AuctionPackage, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = authorizedbuyersmarketplace_buyers_auction_packages_unsubscribe_clients_builder(
            &self.http_client,
            &args.auctionPackage,
        )
        .map_err(ProviderError::Api)?;

        let task = authorizedbuyersmarketplace_buyers_auction_packages_unsubscribe_clients_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Authorizedbuyersmarketplace buyers clients activate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Client result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn authorizedbuyersmarketplace_buyers_clients_activate(
        &self,
        args: &AuthorizedbuyersmarketplaceBuyersClientsActivateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Client, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = authorizedbuyersmarketplace_buyers_clients_activate_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = authorizedbuyersmarketplace_buyers_clients_activate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Authorizedbuyersmarketplace buyers clients create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Client result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn authorizedbuyersmarketplace_buyers_clients_create(
        &self,
        args: &AuthorizedbuyersmarketplaceBuyersClientsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Client, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = authorizedbuyersmarketplace_buyers_clients_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = authorizedbuyersmarketplace_buyers_clients_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Authorizedbuyersmarketplace buyers clients deactivate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Client result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn authorizedbuyersmarketplace_buyers_clients_deactivate(
        &self,
        args: &AuthorizedbuyersmarketplaceBuyersClientsDeactivateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Client, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = authorizedbuyersmarketplace_buyers_clients_deactivate_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = authorizedbuyersmarketplace_buyers_clients_deactivate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Authorizedbuyersmarketplace buyers clients get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Client result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn authorizedbuyersmarketplace_buyers_clients_get(
        &self,
        args: &AuthorizedbuyersmarketplaceBuyersClientsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Client, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = authorizedbuyersmarketplace_buyers_clients_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = authorizedbuyersmarketplace_buyers_clients_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Authorizedbuyersmarketplace buyers clients list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListClientsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn authorizedbuyersmarketplace_buyers_clients_list(
        &self,
        args: &AuthorizedbuyersmarketplaceBuyersClientsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListClientsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = authorizedbuyersmarketplace_buyers_clients_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = authorizedbuyersmarketplace_buyers_clients_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Authorizedbuyersmarketplace buyers clients patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Client result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn authorizedbuyersmarketplace_buyers_clients_patch(
        &self,
        args: &AuthorizedbuyersmarketplaceBuyersClientsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Client, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = authorizedbuyersmarketplace_buyers_clients_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = authorizedbuyersmarketplace_buyers_clients_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Authorizedbuyersmarketplace buyers clients users activate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ClientUser result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn authorizedbuyersmarketplace_buyers_clients_users_activate(
        &self,
        args: &AuthorizedbuyersmarketplaceBuyersClientsUsersActivateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ClientUser, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = authorizedbuyersmarketplace_buyers_clients_users_activate_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = authorizedbuyersmarketplace_buyers_clients_users_activate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Authorizedbuyersmarketplace buyers clients users create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ClientUser result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn authorizedbuyersmarketplace_buyers_clients_users_create(
        &self,
        args: &AuthorizedbuyersmarketplaceBuyersClientsUsersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ClientUser, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = authorizedbuyersmarketplace_buyers_clients_users_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = authorizedbuyersmarketplace_buyers_clients_users_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Authorizedbuyersmarketplace buyers clients users deactivate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ClientUser result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn authorizedbuyersmarketplace_buyers_clients_users_deactivate(
        &self,
        args: &AuthorizedbuyersmarketplaceBuyersClientsUsersDeactivateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ClientUser, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = authorizedbuyersmarketplace_buyers_clients_users_deactivate_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = authorizedbuyersmarketplace_buyers_clients_users_deactivate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Authorizedbuyersmarketplace buyers clients users delete.
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
    pub fn authorizedbuyersmarketplace_buyers_clients_users_delete(
        &self,
        args: &AuthorizedbuyersmarketplaceBuyersClientsUsersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = authorizedbuyersmarketplace_buyers_clients_users_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = authorizedbuyersmarketplace_buyers_clients_users_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Authorizedbuyersmarketplace buyers clients users get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ClientUser result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn authorizedbuyersmarketplace_buyers_clients_users_get(
        &self,
        args: &AuthorizedbuyersmarketplaceBuyersClientsUsersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ClientUser, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = authorizedbuyersmarketplace_buyers_clients_users_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = authorizedbuyersmarketplace_buyers_clients_users_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Authorizedbuyersmarketplace buyers clients users list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListClientUsersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn authorizedbuyersmarketplace_buyers_clients_users_list(
        &self,
        args: &AuthorizedbuyersmarketplaceBuyersClientsUsersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListClientUsersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = authorizedbuyersmarketplace_buyers_clients_users_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = authorizedbuyersmarketplace_buyers_clients_users_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Authorizedbuyersmarketplace buyers finalized deals add creative.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FinalizedDeal result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn authorizedbuyersmarketplace_buyers_finalized_deals_add_creative(
        &self,
        args: &AuthorizedbuyersmarketplaceBuyersFinalizedDealsAddCreativeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FinalizedDeal, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = authorizedbuyersmarketplace_buyers_finalized_deals_add_creative_builder(
            &self.http_client,
            &args.deal,
        )
        .map_err(ProviderError::Api)?;

        let task = authorizedbuyersmarketplace_buyers_finalized_deals_add_creative_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Authorizedbuyersmarketplace buyers finalized deals get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FinalizedDeal result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn authorizedbuyersmarketplace_buyers_finalized_deals_get(
        &self,
        args: &AuthorizedbuyersmarketplaceBuyersFinalizedDealsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FinalizedDeal, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = authorizedbuyersmarketplace_buyers_finalized_deals_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = authorizedbuyersmarketplace_buyers_finalized_deals_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Authorizedbuyersmarketplace buyers finalized deals list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListFinalizedDealsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn authorizedbuyersmarketplace_buyers_finalized_deals_list(
        &self,
        args: &AuthorizedbuyersmarketplaceBuyersFinalizedDealsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListFinalizedDealsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = authorizedbuyersmarketplace_buyers_finalized_deals_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = authorizedbuyersmarketplace_buyers_finalized_deals_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Authorizedbuyersmarketplace buyers finalized deals pause.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FinalizedDeal result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn authorizedbuyersmarketplace_buyers_finalized_deals_pause(
        &self,
        args: &AuthorizedbuyersmarketplaceBuyersFinalizedDealsPauseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FinalizedDeal, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = authorizedbuyersmarketplace_buyers_finalized_deals_pause_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = authorizedbuyersmarketplace_buyers_finalized_deals_pause_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Authorizedbuyersmarketplace buyers finalized deals resume.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FinalizedDeal result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn authorizedbuyersmarketplace_buyers_finalized_deals_resume(
        &self,
        args: &AuthorizedbuyersmarketplaceBuyersFinalizedDealsResumeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FinalizedDeal, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = authorizedbuyersmarketplace_buyers_finalized_deals_resume_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = authorizedbuyersmarketplace_buyers_finalized_deals_resume_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Authorizedbuyersmarketplace buyers finalized deals set ready to serve.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FinalizedDeal result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn authorizedbuyersmarketplace_buyers_finalized_deals_set_ready_to_serve(
        &self,
        args: &AuthorizedbuyersmarketplaceBuyersFinalizedDealsSetReadyToServeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FinalizedDeal, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = authorizedbuyersmarketplace_buyers_finalized_deals_set_ready_to_serve_builder(
            &self.http_client,
            &args.deal,
        )
        .map_err(ProviderError::Api)?;

        let task = authorizedbuyersmarketplace_buyers_finalized_deals_set_ready_to_serve_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Authorizedbuyersmarketplace buyers proposals accept.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Proposal result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn authorizedbuyersmarketplace_buyers_proposals_accept(
        &self,
        args: &AuthorizedbuyersmarketplaceBuyersProposalsAcceptArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Proposal, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = authorizedbuyersmarketplace_buyers_proposals_accept_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = authorizedbuyersmarketplace_buyers_proposals_accept_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Authorizedbuyersmarketplace buyers proposals add note.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Proposal result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn authorizedbuyersmarketplace_buyers_proposals_add_note(
        &self,
        args: &AuthorizedbuyersmarketplaceBuyersProposalsAddNoteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Proposal, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = authorizedbuyersmarketplace_buyers_proposals_add_note_builder(
            &self.http_client,
            &args.proposal,
        )
        .map_err(ProviderError::Api)?;

        let task = authorizedbuyersmarketplace_buyers_proposals_add_note_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Authorizedbuyersmarketplace buyers proposals cancel negotiation.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Proposal result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn authorizedbuyersmarketplace_buyers_proposals_cancel_negotiation(
        &self,
        args: &AuthorizedbuyersmarketplaceBuyersProposalsCancelNegotiationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Proposal, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = authorizedbuyersmarketplace_buyers_proposals_cancel_negotiation_builder(
            &self.http_client,
            &args.proposal,
        )
        .map_err(ProviderError::Api)?;

        let task = authorizedbuyersmarketplace_buyers_proposals_cancel_negotiation_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Authorizedbuyersmarketplace buyers proposals get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Proposal result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn authorizedbuyersmarketplace_buyers_proposals_get(
        &self,
        args: &AuthorizedbuyersmarketplaceBuyersProposalsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Proposal, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = authorizedbuyersmarketplace_buyers_proposals_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = authorizedbuyersmarketplace_buyers_proposals_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Authorizedbuyersmarketplace buyers proposals list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListProposalsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn authorizedbuyersmarketplace_buyers_proposals_list(
        &self,
        args: &AuthorizedbuyersmarketplaceBuyersProposalsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListProposalsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = authorizedbuyersmarketplace_buyers_proposals_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = authorizedbuyersmarketplace_buyers_proposals_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Authorizedbuyersmarketplace buyers proposals patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Proposal result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn authorizedbuyersmarketplace_buyers_proposals_patch(
        &self,
        args: &AuthorizedbuyersmarketplaceBuyersProposalsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Proposal, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = authorizedbuyersmarketplace_buyers_proposals_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = authorizedbuyersmarketplace_buyers_proposals_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Authorizedbuyersmarketplace buyers proposals send rfp.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Proposal result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn authorizedbuyersmarketplace_buyers_proposals_send_rfp(
        &self,
        args: &AuthorizedbuyersmarketplaceBuyersProposalsSendRfpArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Proposal, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = authorizedbuyersmarketplace_buyers_proposals_send_rfp_builder(
            &self.http_client,
            &args.buyer,
        )
        .map_err(ProviderError::Api)?;

        let task = authorizedbuyersmarketplace_buyers_proposals_send_rfp_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Authorizedbuyersmarketplace buyers proposals deals batch update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchUpdateDealsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn authorizedbuyersmarketplace_buyers_proposals_deals_batch_update(
        &self,
        args: &AuthorizedbuyersmarketplaceBuyersProposalsDealsBatchUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchUpdateDealsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = authorizedbuyersmarketplace_buyers_proposals_deals_batch_update_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = authorizedbuyersmarketplace_buyers_proposals_deals_batch_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Authorizedbuyersmarketplace buyers proposals deals get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Deal result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn authorizedbuyersmarketplace_buyers_proposals_deals_get(
        &self,
        args: &AuthorizedbuyersmarketplaceBuyersProposalsDealsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Deal, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = authorizedbuyersmarketplace_buyers_proposals_deals_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = authorizedbuyersmarketplace_buyers_proposals_deals_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Authorizedbuyersmarketplace buyers proposals deals list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDealsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn authorizedbuyersmarketplace_buyers_proposals_deals_list(
        &self,
        args: &AuthorizedbuyersmarketplaceBuyersProposalsDealsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDealsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = authorizedbuyersmarketplace_buyers_proposals_deals_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = authorizedbuyersmarketplace_buyers_proposals_deals_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Authorizedbuyersmarketplace buyers proposals deals patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Deal result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn authorizedbuyersmarketplace_buyers_proposals_deals_patch(
        &self,
        args: &AuthorizedbuyersmarketplaceBuyersProposalsDealsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Deal, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = authorizedbuyersmarketplace_buyers_proposals_deals_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = authorizedbuyersmarketplace_buyers_proposals_deals_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Authorizedbuyersmarketplace buyers publisher profiles get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PublisherProfile result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn authorizedbuyersmarketplace_buyers_publisher_profiles_get(
        &self,
        args: &AuthorizedbuyersmarketplaceBuyersPublisherProfilesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PublisherProfile, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = authorizedbuyersmarketplace_buyers_publisher_profiles_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = authorizedbuyersmarketplace_buyers_publisher_profiles_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Authorizedbuyersmarketplace buyers publisher profiles list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListPublisherProfilesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn authorizedbuyersmarketplace_buyers_publisher_profiles_list(
        &self,
        args: &AuthorizedbuyersmarketplaceBuyersPublisherProfilesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListPublisherProfilesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = authorizedbuyersmarketplace_buyers_publisher_profiles_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = authorizedbuyersmarketplace_buyers_publisher_profiles_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
