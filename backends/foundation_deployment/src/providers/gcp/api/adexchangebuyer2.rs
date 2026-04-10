//! Adexchangebuyer2Provider - State-aware adexchangebuyer2 API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       adexchangebuyer2 API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::adexchangebuyer2::{
    adexchangebuyer2_accounts_clients_create_builder, adexchangebuyer2_accounts_clients_create_task,
    adexchangebuyer2_accounts_clients_get_builder, adexchangebuyer2_accounts_clients_get_task,
    adexchangebuyer2_accounts_clients_list_builder, adexchangebuyer2_accounts_clients_list_task,
    adexchangebuyer2_accounts_clients_update_builder, adexchangebuyer2_accounts_clients_update_task,
    adexchangebuyer2_accounts_clients_invitations_create_builder, adexchangebuyer2_accounts_clients_invitations_create_task,
    adexchangebuyer2_accounts_clients_invitations_get_builder, adexchangebuyer2_accounts_clients_invitations_get_task,
    adexchangebuyer2_accounts_clients_invitations_list_builder, adexchangebuyer2_accounts_clients_invitations_list_task,
    adexchangebuyer2_accounts_clients_users_get_builder, adexchangebuyer2_accounts_clients_users_get_task,
    adexchangebuyer2_accounts_clients_users_list_builder, adexchangebuyer2_accounts_clients_users_list_task,
    adexchangebuyer2_accounts_clients_users_update_builder, adexchangebuyer2_accounts_clients_users_update_task,
    adexchangebuyer2_accounts_creatives_create_builder, adexchangebuyer2_accounts_creatives_create_task,
    adexchangebuyer2_accounts_creatives_get_builder, adexchangebuyer2_accounts_creatives_get_task,
    adexchangebuyer2_accounts_creatives_list_builder, adexchangebuyer2_accounts_creatives_list_task,
    adexchangebuyer2_accounts_creatives_stop_watching_builder, adexchangebuyer2_accounts_creatives_stop_watching_task,
    adexchangebuyer2_accounts_creatives_update_builder, adexchangebuyer2_accounts_creatives_update_task,
    adexchangebuyer2_accounts_creatives_watch_builder, adexchangebuyer2_accounts_creatives_watch_task,
    adexchangebuyer2_accounts_creatives_deal_associations_add_builder, adexchangebuyer2_accounts_creatives_deal_associations_add_task,
    adexchangebuyer2_accounts_creatives_deal_associations_list_builder, adexchangebuyer2_accounts_creatives_deal_associations_list_task,
    adexchangebuyer2_accounts_creatives_deal_associations_remove_builder, adexchangebuyer2_accounts_creatives_deal_associations_remove_task,
    adexchangebuyer2_accounts_finalized_proposals_list_builder, adexchangebuyer2_accounts_finalized_proposals_list_task,
    adexchangebuyer2_accounts_finalized_proposals_pause_builder, adexchangebuyer2_accounts_finalized_proposals_pause_task,
    adexchangebuyer2_accounts_finalized_proposals_resume_builder, adexchangebuyer2_accounts_finalized_proposals_resume_task,
    adexchangebuyer2_accounts_products_get_builder, adexchangebuyer2_accounts_products_get_task,
    adexchangebuyer2_accounts_products_list_builder, adexchangebuyer2_accounts_products_list_task,
    adexchangebuyer2_accounts_proposals_accept_builder, adexchangebuyer2_accounts_proposals_accept_task,
    adexchangebuyer2_accounts_proposals_add_note_builder, adexchangebuyer2_accounts_proposals_add_note_task,
    adexchangebuyer2_accounts_proposals_cancel_negotiation_builder, adexchangebuyer2_accounts_proposals_cancel_negotiation_task,
    adexchangebuyer2_accounts_proposals_complete_setup_builder, adexchangebuyer2_accounts_proposals_complete_setup_task,
    adexchangebuyer2_accounts_proposals_create_builder, adexchangebuyer2_accounts_proposals_create_task,
    adexchangebuyer2_accounts_proposals_get_builder, adexchangebuyer2_accounts_proposals_get_task,
    adexchangebuyer2_accounts_proposals_list_builder, adexchangebuyer2_accounts_proposals_list_task,
    adexchangebuyer2_accounts_proposals_pause_builder, adexchangebuyer2_accounts_proposals_pause_task,
    adexchangebuyer2_accounts_proposals_resume_builder, adexchangebuyer2_accounts_proposals_resume_task,
    adexchangebuyer2_accounts_proposals_update_builder, adexchangebuyer2_accounts_proposals_update_task,
    adexchangebuyer2_accounts_publisher_profiles_get_builder, adexchangebuyer2_accounts_publisher_profiles_get_task,
    adexchangebuyer2_accounts_publisher_profiles_list_builder, adexchangebuyer2_accounts_publisher_profiles_list_task,
    adexchangebuyer2_bidders_accounts_filter_sets_create_builder, adexchangebuyer2_bidders_accounts_filter_sets_create_task,
    adexchangebuyer2_bidders_accounts_filter_sets_delete_builder, adexchangebuyer2_bidders_accounts_filter_sets_delete_task,
    adexchangebuyer2_bidders_accounts_filter_sets_get_builder, adexchangebuyer2_bidders_accounts_filter_sets_get_task,
    adexchangebuyer2_bidders_accounts_filter_sets_list_builder, adexchangebuyer2_bidders_accounts_filter_sets_list_task,
    adexchangebuyer2_bidders_accounts_filter_sets_bid_metrics_list_builder, adexchangebuyer2_bidders_accounts_filter_sets_bid_metrics_list_task,
    adexchangebuyer2_bidders_accounts_filter_sets_bid_response_errors_list_builder, adexchangebuyer2_bidders_accounts_filter_sets_bid_response_errors_list_task,
    adexchangebuyer2_bidders_accounts_filter_sets_bid_responses_without_bids_list_builder, adexchangebuyer2_bidders_accounts_filter_sets_bid_responses_without_bids_list_task,
    adexchangebuyer2_bidders_accounts_filter_sets_filtered_bid_requests_list_builder, adexchangebuyer2_bidders_accounts_filter_sets_filtered_bid_requests_list_task,
    adexchangebuyer2_bidders_accounts_filter_sets_filtered_bids_list_builder, adexchangebuyer2_bidders_accounts_filter_sets_filtered_bids_list_task,
    adexchangebuyer2_bidders_accounts_filter_sets_filtered_bids_creatives_list_builder, adexchangebuyer2_bidders_accounts_filter_sets_filtered_bids_creatives_list_task,
    adexchangebuyer2_bidders_accounts_filter_sets_filtered_bids_details_list_builder, adexchangebuyer2_bidders_accounts_filter_sets_filtered_bids_details_list_task,
    adexchangebuyer2_bidders_accounts_filter_sets_impression_metrics_list_builder, adexchangebuyer2_bidders_accounts_filter_sets_impression_metrics_list_task,
    adexchangebuyer2_bidders_accounts_filter_sets_losing_bids_list_builder, adexchangebuyer2_bidders_accounts_filter_sets_losing_bids_list_task,
    adexchangebuyer2_bidders_accounts_filter_sets_non_billable_winning_bids_list_builder, adexchangebuyer2_bidders_accounts_filter_sets_non_billable_winning_bids_list_task,
    adexchangebuyer2_bidders_filter_sets_create_builder, adexchangebuyer2_bidders_filter_sets_create_task,
    adexchangebuyer2_bidders_filter_sets_delete_builder, adexchangebuyer2_bidders_filter_sets_delete_task,
    adexchangebuyer2_bidders_filter_sets_get_builder, adexchangebuyer2_bidders_filter_sets_get_task,
    adexchangebuyer2_bidders_filter_sets_list_builder, adexchangebuyer2_bidders_filter_sets_list_task,
    adexchangebuyer2_bidders_filter_sets_bid_metrics_list_builder, adexchangebuyer2_bidders_filter_sets_bid_metrics_list_task,
    adexchangebuyer2_bidders_filter_sets_bid_response_errors_list_builder, adexchangebuyer2_bidders_filter_sets_bid_response_errors_list_task,
    adexchangebuyer2_bidders_filter_sets_bid_responses_without_bids_list_builder, adexchangebuyer2_bidders_filter_sets_bid_responses_without_bids_list_task,
    adexchangebuyer2_bidders_filter_sets_filtered_bid_requests_list_builder, adexchangebuyer2_bidders_filter_sets_filtered_bid_requests_list_task,
    adexchangebuyer2_bidders_filter_sets_filtered_bids_list_builder, adexchangebuyer2_bidders_filter_sets_filtered_bids_list_task,
    adexchangebuyer2_bidders_filter_sets_filtered_bids_creatives_list_builder, adexchangebuyer2_bidders_filter_sets_filtered_bids_creatives_list_task,
    adexchangebuyer2_bidders_filter_sets_filtered_bids_details_list_builder, adexchangebuyer2_bidders_filter_sets_filtered_bids_details_list_task,
    adexchangebuyer2_bidders_filter_sets_impression_metrics_list_builder, adexchangebuyer2_bidders_filter_sets_impression_metrics_list_task,
    adexchangebuyer2_bidders_filter_sets_losing_bids_list_builder, adexchangebuyer2_bidders_filter_sets_losing_bids_list_task,
    adexchangebuyer2_bidders_filter_sets_non_billable_winning_bids_list_builder, adexchangebuyer2_bidders_filter_sets_non_billable_winning_bids_list_task,
    adexchangebuyer2_buyers_filter_sets_create_builder, adexchangebuyer2_buyers_filter_sets_create_task,
    adexchangebuyer2_buyers_filter_sets_delete_builder, adexchangebuyer2_buyers_filter_sets_delete_task,
    adexchangebuyer2_buyers_filter_sets_get_builder, adexchangebuyer2_buyers_filter_sets_get_task,
    adexchangebuyer2_buyers_filter_sets_list_builder, adexchangebuyer2_buyers_filter_sets_list_task,
    adexchangebuyer2_buyers_filter_sets_bid_metrics_list_builder, adexchangebuyer2_buyers_filter_sets_bid_metrics_list_task,
    adexchangebuyer2_buyers_filter_sets_bid_response_errors_list_builder, adexchangebuyer2_buyers_filter_sets_bid_response_errors_list_task,
    adexchangebuyer2_buyers_filter_sets_bid_responses_without_bids_list_builder, adexchangebuyer2_buyers_filter_sets_bid_responses_without_bids_list_task,
    adexchangebuyer2_buyers_filter_sets_filtered_bid_requests_list_builder, adexchangebuyer2_buyers_filter_sets_filtered_bid_requests_list_task,
    adexchangebuyer2_buyers_filter_sets_filtered_bids_list_builder, adexchangebuyer2_buyers_filter_sets_filtered_bids_list_task,
    adexchangebuyer2_buyers_filter_sets_filtered_bids_creatives_list_builder, adexchangebuyer2_buyers_filter_sets_filtered_bids_creatives_list_task,
    adexchangebuyer2_buyers_filter_sets_filtered_bids_details_list_builder, adexchangebuyer2_buyers_filter_sets_filtered_bids_details_list_task,
    adexchangebuyer2_buyers_filter_sets_impression_metrics_list_builder, adexchangebuyer2_buyers_filter_sets_impression_metrics_list_task,
    adexchangebuyer2_buyers_filter_sets_losing_bids_list_builder, adexchangebuyer2_buyers_filter_sets_losing_bids_list_task,
    adexchangebuyer2_buyers_filter_sets_non_billable_winning_bids_list_builder, adexchangebuyer2_buyers_filter_sets_non_billable_winning_bids_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::adexchangebuyer2::Client;
use crate::providers::gcp::clients::adexchangebuyer2::ClientUser;
use crate::providers::gcp::clients::adexchangebuyer2::ClientUserInvitation;
use crate::providers::gcp::clients::adexchangebuyer2::Creative;
use crate::providers::gcp::clients::adexchangebuyer2::Empty;
use crate::providers::gcp::clients::adexchangebuyer2::FilterSet;
use crate::providers::gcp::clients::adexchangebuyer2::ListBidMetricsResponse;
use crate::providers::gcp::clients::adexchangebuyer2::ListBidResponseErrorsResponse;
use crate::providers::gcp::clients::adexchangebuyer2::ListBidResponsesWithoutBidsResponse;
use crate::providers::gcp::clients::adexchangebuyer2::ListClientUserInvitationsResponse;
use crate::providers::gcp::clients::adexchangebuyer2::ListClientUsersResponse;
use crate::providers::gcp::clients::adexchangebuyer2::ListClientsResponse;
use crate::providers::gcp::clients::adexchangebuyer2::ListCreativeStatusBreakdownByCreativeResponse;
use crate::providers::gcp::clients::adexchangebuyer2::ListCreativeStatusBreakdownByDetailResponse;
use crate::providers::gcp::clients::adexchangebuyer2::ListCreativesResponse;
use crate::providers::gcp::clients::adexchangebuyer2::ListDealAssociationsResponse;
use crate::providers::gcp::clients::adexchangebuyer2::ListFilterSetsResponse;
use crate::providers::gcp::clients::adexchangebuyer2::ListFilteredBidRequestsResponse;
use crate::providers::gcp::clients::adexchangebuyer2::ListFilteredBidsResponse;
use crate::providers::gcp::clients::adexchangebuyer2::ListImpressionMetricsResponse;
use crate::providers::gcp::clients::adexchangebuyer2::ListLosingBidsResponse;
use crate::providers::gcp::clients::adexchangebuyer2::ListNonBillableWinningBidsResponse;
use crate::providers::gcp::clients::adexchangebuyer2::ListProductsResponse;
use crate::providers::gcp::clients::adexchangebuyer2::ListProposalsResponse;
use crate::providers::gcp::clients::adexchangebuyer2::ListPublisherProfilesResponse;
use crate::providers::gcp::clients::adexchangebuyer2::Note;
use crate::providers::gcp::clients::adexchangebuyer2::Product;
use crate::providers::gcp::clients::adexchangebuyer2::Proposal;
use crate::providers::gcp::clients::adexchangebuyer2::PublisherProfile;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsClientsCreateArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsClientsGetArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsClientsInvitationsCreateArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsClientsInvitationsGetArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsClientsInvitationsListArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsClientsListArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsClientsUpdateArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsClientsUsersGetArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsClientsUsersListArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsClientsUsersUpdateArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsCreativesCreateArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsCreativesDealAssociationsAddArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsCreativesDealAssociationsListArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsCreativesDealAssociationsRemoveArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsCreativesGetArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsCreativesListArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsCreativesStopWatchingArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsCreativesUpdateArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsCreativesWatchArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsFinalizedProposalsListArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsFinalizedProposalsPauseArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsFinalizedProposalsResumeArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsProductsGetArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsProductsListArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsProposalsAcceptArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsProposalsAddNoteArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsProposalsCancelNegotiationArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsProposalsCompleteSetupArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsProposalsCreateArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsProposalsGetArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsProposalsListArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsProposalsPauseArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsProposalsResumeArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsProposalsUpdateArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsPublisherProfilesGetArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsPublisherProfilesListArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2BiddersAccountsFilterSetsBidMetricsListArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2BiddersAccountsFilterSetsBidResponseErrorsListArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2BiddersAccountsFilterSetsBidResponsesWithoutBidsListArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2BiddersAccountsFilterSetsCreateArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2BiddersAccountsFilterSetsDeleteArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2BiddersAccountsFilterSetsFilteredBidRequestsListArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2BiddersAccountsFilterSetsFilteredBidsCreativesListArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2BiddersAccountsFilterSetsFilteredBidsDetailsListArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2BiddersAccountsFilterSetsFilteredBidsListArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2BiddersAccountsFilterSetsGetArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2BiddersAccountsFilterSetsImpressionMetricsListArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2BiddersAccountsFilterSetsListArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2BiddersAccountsFilterSetsLosingBidsListArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2BiddersAccountsFilterSetsNonBillableWinningBidsListArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2BiddersFilterSetsBidMetricsListArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2BiddersFilterSetsBidResponseErrorsListArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2BiddersFilterSetsBidResponsesWithoutBidsListArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2BiddersFilterSetsCreateArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2BiddersFilterSetsDeleteArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2BiddersFilterSetsFilteredBidRequestsListArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2BiddersFilterSetsFilteredBidsCreativesListArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2BiddersFilterSetsFilteredBidsDetailsListArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2BiddersFilterSetsFilteredBidsListArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2BiddersFilterSetsGetArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2BiddersFilterSetsImpressionMetricsListArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2BiddersFilterSetsListArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2BiddersFilterSetsLosingBidsListArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2BiddersFilterSetsNonBillableWinningBidsListArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2BuyersFilterSetsBidMetricsListArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2BuyersFilterSetsBidResponseErrorsListArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2BuyersFilterSetsBidResponsesWithoutBidsListArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2BuyersFilterSetsCreateArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2BuyersFilterSetsDeleteArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2BuyersFilterSetsFilteredBidRequestsListArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2BuyersFilterSetsFilteredBidsCreativesListArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2BuyersFilterSetsFilteredBidsDetailsListArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2BuyersFilterSetsFilteredBidsListArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2BuyersFilterSetsGetArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2BuyersFilterSetsImpressionMetricsListArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2BuyersFilterSetsListArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2BuyersFilterSetsLosingBidsListArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2BuyersFilterSetsNonBillableWinningBidsListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// Adexchangebuyer2Provider with automatic state tracking.
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
/// let provider = Adexchangebuyer2Provider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct Adexchangebuyer2Provider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> Adexchangebuyer2Provider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new Adexchangebuyer2Provider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Adexchangebuyer2 accounts clients create.
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
    pub fn adexchangebuyer2_accounts_clients_create(
        &self,
        args: &Adexchangebuyer2AccountsClientsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Client, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_accounts_clients_create_builder(
            &self.http_client,
            &args.accountId,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_accounts_clients_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 accounts clients get.
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
    pub fn adexchangebuyer2_accounts_clients_get(
        &self,
        args: &Adexchangebuyer2AccountsClientsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Client, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_accounts_clients_get_builder(
            &self.http_client,
            &args.accountId,
            &args.clientAccountId,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_accounts_clients_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 accounts clients list.
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
    pub fn adexchangebuyer2_accounts_clients_list(
        &self,
        args: &Adexchangebuyer2AccountsClientsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListClientsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_accounts_clients_list_builder(
            &self.http_client,
            &args.accountId,
            &args.pageSize,
            &args.pageToken,
            &args.partnerClientId,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_accounts_clients_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 accounts clients update.
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
    pub fn adexchangebuyer2_accounts_clients_update(
        &self,
        args: &Adexchangebuyer2AccountsClientsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Client, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_accounts_clients_update_builder(
            &self.http_client,
            &args.accountId,
            &args.clientAccountId,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_accounts_clients_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 accounts clients invitations create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ClientUserInvitation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn adexchangebuyer2_accounts_clients_invitations_create(
        &self,
        args: &Adexchangebuyer2AccountsClientsInvitationsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ClientUserInvitation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_accounts_clients_invitations_create_builder(
            &self.http_client,
            &args.accountId,
            &args.clientAccountId,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_accounts_clients_invitations_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 accounts clients invitations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ClientUserInvitation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adexchangebuyer2_accounts_clients_invitations_get(
        &self,
        args: &Adexchangebuyer2AccountsClientsInvitationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ClientUserInvitation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_accounts_clients_invitations_get_builder(
            &self.http_client,
            &args.accountId,
            &args.clientAccountId,
            &args.invitationId,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_accounts_clients_invitations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 accounts clients invitations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListClientUserInvitationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adexchangebuyer2_accounts_clients_invitations_list(
        &self,
        args: &Adexchangebuyer2AccountsClientsInvitationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListClientUserInvitationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_accounts_clients_invitations_list_builder(
            &self.http_client,
            &args.accountId,
            &args.clientAccountId,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_accounts_clients_invitations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 accounts clients users get.
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
    pub fn adexchangebuyer2_accounts_clients_users_get(
        &self,
        args: &Adexchangebuyer2AccountsClientsUsersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ClientUser, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_accounts_clients_users_get_builder(
            &self.http_client,
            &args.accountId,
            &args.clientAccountId,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_accounts_clients_users_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 accounts clients users list.
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
    pub fn adexchangebuyer2_accounts_clients_users_list(
        &self,
        args: &Adexchangebuyer2AccountsClientsUsersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListClientUsersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_accounts_clients_users_list_builder(
            &self.http_client,
            &args.accountId,
            &args.clientAccountId,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_accounts_clients_users_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 accounts clients users update.
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
    pub fn adexchangebuyer2_accounts_clients_users_update(
        &self,
        args: &Adexchangebuyer2AccountsClientsUsersUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ClientUser, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_accounts_clients_users_update_builder(
            &self.http_client,
            &args.accountId,
            &args.clientAccountId,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_accounts_clients_users_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 accounts creatives create.
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
    pub fn adexchangebuyer2_accounts_creatives_create(
        &self,
        args: &Adexchangebuyer2AccountsCreativesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Creative, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_accounts_creatives_create_builder(
            &self.http_client,
            &args.accountId,
            &args.duplicateIdMode,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_accounts_creatives_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 accounts creatives get.
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
    pub fn adexchangebuyer2_accounts_creatives_get(
        &self,
        args: &Adexchangebuyer2AccountsCreativesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Creative, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_accounts_creatives_get_builder(
            &self.http_client,
            &args.accountId,
            &args.creativeId,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_accounts_creatives_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 accounts creatives list.
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
    pub fn adexchangebuyer2_accounts_creatives_list(
        &self,
        args: &Adexchangebuyer2AccountsCreativesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCreativesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_accounts_creatives_list_builder(
            &self.http_client,
            &args.accountId,
            &args.pageSize,
            &args.pageToken,
            &args.query,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_accounts_creatives_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 accounts creatives stop watching.
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
    pub fn adexchangebuyer2_accounts_creatives_stop_watching(
        &self,
        args: &Adexchangebuyer2AccountsCreativesStopWatchingArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_accounts_creatives_stop_watching_builder(
            &self.http_client,
            &args.accountId,
            &args.creativeId,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_accounts_creatives_stop_watching_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 accounts creatives update.
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
    pub fn adexchangebuyer2_accounts_creatives_update(
        &self,
        args: &Adexchangebuyer2AccountsCreativesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Creative, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_accounts_creatives_update_builder(
            &self.http_client,
            &args.accountId,
            &args.creativeId,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_accounts_creatives_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 accounts creatives watch.
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
    pub fn adexchangebuyer2_accounts_creatives_watch(
        &self,
        args: &Adexchangebuyer2AccountsCreativesWatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_accounts_creatives_watch_builder(
            &self.http_client,
            &args.accountId,
            &args.creativeId,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_accounts_creatives_watch_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 accounts creatives deal associations add.
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
    pub fn adexchangebuyer2_accounts_creatives_deal_associations_add(
        &self,
        args: &Adexchangebuyer2AccountsCreativesDealAssociationsAddArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_accounts_creatives_deal_associations_add_builder(
            &self.http_client,
            &args.accountId,
            &args.creativeId,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_accounts_creatives_deal_associations_add_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 accounts creatives deal associations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDealAssociationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adexchangebuyer2_accounts_creatives_deal_associations_list(
        &self,
        args: &Adexchangebuyer2AccountsCreativesDealAssociationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDealAssociationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_accounts_creatives_deal_associations_list_builder(
            &self.http_client,
            &args.accountId,
            &args.creativeId,
            &args.pageSize,
            &args.pageToken,
            &args.query,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_accounts_creatives_deal_associations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 accounts creatives deal associations remove.
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
    pub fn adexchangebuyer2_accounts_creatives_deal_associations_remove(
        &self,
        args: &Adexchangebuyer2AccountsCreativesDealAssociationsRemoveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_accounts_creatives_deal_associations_remove_builder(
            &self.http_client,
            &args.accountId,
            &args.creativeId,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_accounts_creatives_deal_associations_remove_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 accounts finalized proposals list.
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
    pub fn adexchangebuyer2_accounts_finalized_proposals_list(
        &self,
        args: &Adexchangebuyer2AccountsFinalizedProposalsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListProposalsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_accounts_finalized_proposals_list_builder(
            &self.http_client,
            &args.accountId,
            &args.filter,
            &args.filterSyntax,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_accounts_finalized_proposals_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 accounts finalized proposals pause.
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
    pub fn adexchangebuyer2_accounts_finalized_proposals_pause(
        &self,
        args: &Adexchangebuyer2AccountsFinalizedProposalsPauseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Proposal, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_accounts_finalized_proposals_pause_builder(
            &self.http_client,
            &args.accountId,
            &args.proposalId,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_accounts_finalized_proposals_pause_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 accounts finalized proposals resume.
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
    pub fn adexchangebuyer2_accounts_finalized_proposals_resume(
        &self,
        args: &Adexchangebuyer2AccountsFinalizedProposalsResumeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Proposal, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_accounts_finalized_proposals_resume_builder(
            &self.http_client,
            &args.accountId,
            &args.proposalId,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_accounts_finalized_proposals_resume_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 accounts products get.
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
    pub fn adexchangebuyer2_accounts_products_get(
        &self,
        args: &Adexchangebuyer2AccountsProductsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Product, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_accounts_products_get_builder(
            &self.http_client,
            &args.accountId,
            &args.productId,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_accounts_products_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 accounts products list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListProductsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adexchangebuyer2_accounts_products_list(
        &self,
        args: &Adexchangebuyer2AccountsProductsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListProductsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_accounts_products_list_builder(
            &self.http_client,
            &args.accountId,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_accounts_products_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 accounts proposals accept.
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
    pub fn adexchangebuyer2_accounts_proposals_accept(
        &self,
        args: &Adexchangebuyer2AccountsProposalsAcceptArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Proposal, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_accounts_proposals_accept_builder(
            &self.http_client,
            &args.accountId,
            &args.proposalId,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_accounts_proposals_accept_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 accounts proposals add note.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Note result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn adexchangebuyer2_accounts_proposals_add_note(
        &self,
        args: &Adexchangebuyer2AccountsProposalsAddNoteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Note, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_accounts_proposals_add_note_builder(
            &self.http_client,
            &args.accountId,
            &args.proposalId,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_accounts_proposals_add_note_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 accounts proposals cancel negotiation.
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
    pub fn adexchangebuyer2_accounts_proposals_cancel_negotiation(
        &self,
        args: &Adexchangebuyer2AccountsProposalsCancelNegotiationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Proposal, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_accounts_proposals_cancel_negotiation_builder(
            &self.http_client,
            &args.accountId,
            &args.proposalId,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_accounts_proposals_cancel_negotiation_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 accounts proposals complete setup.
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
    pub fn adexchangebuyer2_accounts_proposals_complete_setup(
        &self,
        args: &Adexchangebuyer2AccountsProposalsCompleteSetupArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Proposal, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_accounts_proposals_complete_setup_builder(
            &self.http_client,
            &args.accountId,
            &args.proposalId,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_accounts_proposals_complete_setup_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 accounts proposals create.
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
    pub fn adexchangebuyer2_accounts_proposals_create(
        &self,
        args: &Adexchangebuyer2AccountsProposalsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Proposal, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_accounts_proposals_create_builder(
            &self.http_client,
            &args.accountId,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_accounts_proposals_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 accounts proposals get.
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
    pub fn adexchangebuyer2_accounts_proposals_get(
        &self,
        args: &Adexchangebuyer2AccountsProposalsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Proposal, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_accounts_proposals_get_builder(
            &self.http_client,
            &args.accountId,
            &args.proposalId,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_accounts_proposals_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 accounts proposals list.
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
    pub fn adexchangebuyer2_accounts_proposals_list(
        &self,
        args: &Adexchangebuyer2AccountsProposalsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListProposalsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_accounts_proposals_list_builder(
            &self.http_client,
            &args.accountId,
            &args.filter,
            &args.filterSyntax,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_accounts_proposals_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 accounts proposals pause.
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
    pub fn adexchangebuyer2_accounts_proposals_pause(
        &self,
        args: &Adexchangebuyer2AccountsProposalsPauseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Proposal, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_accounts_proposals_pause_builder(
            &self.http_client,
            &args.accountId,
            &args.proposalId,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_accounts_proposals_pause_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 accounts proposals resume.
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
    pub fn adexchangebuyer2_accounts_proposals_resume(
        &self,
        args: &Adexchangebuyer2AccountsProposalsResumeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Proposal, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_accounts_proposals_resume_builder(
            &self.http_client,
            &args.accountId,
            &args.proposalId,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_accounts_proposals_resume_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 accounts proposals update.
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
    pub fn adexchangebuyer2_accounts_proposals_update(
        &self,
        args: &Adexchangebuyer2AccountsProposalsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Proposal, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_accounts_proposals_update_builder(
            &self.http_client,
            &args.accountId,
            &args.proposalId,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_accounts_proposals_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 accounts publisher profiles get.
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
    pub fn adexchangebuyer2_accounts_publisher_profiles_get(
        &self,
        args: &Adexchangebuyer2AccountsPublisherProfilesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PublisherProfile, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_accounts_publisher_profiles_get_builder(
            &self.http_client,
            &args.accountId,
            &args.publisherProfileId,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_accounts_publisher_profiles_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 accounts publisher profiles list.
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
    pub fn adexchangebuyer2_accounts_publisher_profiles_list(
        &self,
        args: &Adexchangebuyer2AccountsPublisherProfilesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListPublisherProfilesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_accounts_publisher_profiles_list_builder(
            &self.http_client,
            &args.accountId,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_accounts_publisher_profiles_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 bidders accounts filter sets create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FilterSet result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn adexchangebuyer2_bidders_accounts_filter_sets_create(
        &self,
        args: &Adexchangebuyer2BiddersAccountsFilterSetsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FilterSet, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_bidders_accounts_filter_sets_create_builder(
            &self.http_client,
            &args.ownerName,
            &args.isTransient,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_bidders_accounts_filter_sets_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 bidders accounts filter sets delete.
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
    pub fn adexchangebuyer2_bidders_accounts_filter_sets_delete(
        &self,
        args: &Adexchangebuyer2BiddersAccountsFilterSetsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_bidders_accounts_filter_sets_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_bidders_accounts_filter_sets_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 bidders accounts filter sets get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FilterSet result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adexchangebuyer2_bidders_accounts_filter_sets_get(
        &self,
        args: &Adexchangebuyer2BiddersAccountsFilterSetsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FilterSet, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_bidders_accounts_filter_sets_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_bidders_accounts_filter_sets_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 bidders accounts filter sets list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListFilterSetsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adexchangebuyer2_bidders_accounts_filter_sets_list(
        &self,
        args: &Adexchangebuyer2BiddersAccountsFilterSetsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListFilterSetsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_bidders_accounts_filter_sets_list_builder(
            &self.http_client,
            &args.ownerName,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_bidders_accounts_filter_sets_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 bidders accounts filter sets bid metrics list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListBidMetricsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adexchangebuyer2_bidders_accounts_filter_sets_bid_metrics_list(
        &self,
        args: &Adexchangebuyer2BiddersAccountsFilterSetsBidMetricsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBidMetricsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_bidders_accounts_filter_sets_bid_metrics_list_builder(
            &self.http_client,
            &args.filterSetName,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_bidders_accounts_filter_sets_bid_metrics_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 bidders accounts filter sets bid response errors list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListBidResponseErrorsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adexchangebuyer2_bidders_accounts_filter_sets_bid_response_errors_list(
        &self,
        args: &Adexchangebuyer2BiddersAccountsFilterSetsBidResponseErrorsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBidResponseErrorsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_bidders_accounts_filter_sets_bid_response_errors_list_builder(
            &self.http_client,
            &args.filterSetName,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_bidders_accounts_filter_sets_bid_response_errors_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 bidders accounts filter sets bid responses without bids list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListBidResponsesWithoutBidsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adexchangebuyer2_bidders_accounts_filter_sets_bid_responses_without_bids_list(
        &self,
        args: &Adexchangebuyer2BiddersAccountsFilterSetsBidResponsesWithoutBidsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBidResponsesWithoutBidsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_bidders_accounts_filter_sets_bid_responses_without_bids_list_builder(
            &self.http_client,
            &args.filterSetName,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_bidders_accounts_filter_sets_bid_responses_without_bids_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 bidders accounts filter sets filtered bid requests list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListFilteredBidRequestsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adexchangebuyer2_bidders_accounts_filter_sets_filtered_bid_requests_list(
        &self,
        args: &Adexchangebuyer2BiddersAccountsFilterSetsFilteredBidRequestsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListFilteredBidRequestsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_bidders_accounts_filter_sets_filtered_bid_requests_list_builder(
            &self.http_client,
            &args.filterSetName,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_bidders_accounts_filter_sets_filtered_bid_requests_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 bidders accounts filter sets filtered bids list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListFilteredBidsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adexchangebuyer2_bidders_accounts_filter_sets_filtered_bids_list(
        &self,
        args: &Adexchangebuyer2BiddersAccountsFilterSetsFilteredBidsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListFilteredBidsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_bidders_accounts_filter_sets_filtered_bids_list_builder(
            &self.http_client,
            &args.filterSetName,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_bidders_accounts_filter_sets_filtered_bids_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 bidders accounts filter sets filtered bids creatives list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCreativeStatusBreakdownByCreativeResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adexchangebuyer2_bidders_accounts_filter_sets_filtered_bids_creatives_list(
        &self,
        args: &Adexchangebuyer2BiddersAccountsFilterSetsFilteredBidsCreativesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCreativeStatusBreakdownByCreativeResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_bidders_accounts_filter_sets_filtered_bids_creatives_list_builder(
            &self.http_client,
            &args.filterSetName,
            &args.creativeStatusId,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_bidders_accounts_filter_sets_filtered_bids_creatives_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 bidders accounts filter sets filtered bids details list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCreativeStatusBreakdownByDetailResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adexchangebuyer2_bidders_accounts_filter_sets_filtered_bids_details_list(
        &self,
        args: &Adexchangebuyer2BiddersAccountsFilterSetsFilteredBidsDetailsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCreativeStatusBreakdownByDetailResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_bidders_accounts_filter_sets_filtered_bids_details_list_builder(
            &self.http_client,
            &args.filterSetName,
            &args.creativeStatusId,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_bidders_accounts_filter_sets_filtered_bids_details_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 bidders accounts filter sets impression metrics list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListImpressionMetricsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adexchangebuyer2_bidders_accounts_filter_sets_impression_metrics_list(
        &self,
        args: &Adexchangebuyer2BiddersAccountsFilterSetsImpressionMetricsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListImpressionMetricsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_bidders_accounts_filter_sets_impression_metrics_list_builder(
            &self.http_client,
            &args.filterSetName,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_bidders_accounts_filter_sets_impression_metrics_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 bidders accounts filter sets losing bids list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListLosingBidsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adexchangebuyer2_bidders_accounts_filter_sets_losing_bids_list(
        &self,
        args: &Adexchangebuyer2BiddersAccountsFilterSetsLosingBidsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLosingBidsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_bidders_accounts_filter_sets_losing_bids_list_builder(
            &self.http_client,
            &args.filterSetName,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_bidders_accounts_filter_sets_losing_bids_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 bidders accounts filter sets non billable winning bids list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListNonBillableWinningBidsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adexchangebuyer2_bidders_accounts_filter_sets_non_billable_winning_bids_list(
        &self,
        args: &Adexchangebuyer2BiddersAccountsFilterSetsNonBillableWinningBidsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListNonBillableWinningBidsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_bidders_accounts_filter_sets_non_billable_winning_bids_list_builder(
            &self.http_client,
            &args.filterSetName,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_bidders_accounts_filter_sets_non_billable_winning_bids_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 bidders filter sets create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FilterSet result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn adexchangebuyer2_bidders_filter_sets_create(
        &self,
        args: &Adexchangebuyer2BiddersFilterSetsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FilterSet, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_bidders_filter_sets_create_builder(
            &self.http_client,
            &args.ownerName,
            &args.isTransient,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_bidders_filter_sets_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 bidders filter sets delete.
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
    pub fn adexchangebuyer2_bidders_filter_sets_delete(
        &self,
        args: &Adexchangebuyer2BiddersFilterSetsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_bidders_filter_sets_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_bidders_filter_sets_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 bidders filter sets get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FilterSet result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adexchangebuyer2_bidders_filter_sets_get(
        &self,
        args: &Adexchangebuyer2BiddersFilterSetsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FilterSet, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_bidders_filter_sets_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_bidders_filter_sets_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 bidders filter sets list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListFilterSetsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adexchangebuyer2_bidders_filter_sets_list(
        &self,
        args: &Adexchangebuyer2BiddersFilterSetsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListFilterSetsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_bidders_filter_sets_list_builder(
            &self.http_client,
            &args.ownerName,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_bidders_filter_sets_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 bidders filter sets bid metrics list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListBidMetricsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adexchangebuyer2_bidders_filter_sets_bid_metrics_list(
        &self,
        args: &Adexchangebuyer2BiddersFilterSetsBidMetricsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBidMetricsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_bidders_filter_sets_bid_metrics_list_builder(
            &self.http_client,
            &args.filterSetName,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_bidders_filter_sets_bid_metrics_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 bidders filter sets bid response errors list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListBidResponseErrorsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adexchangebuyer2_bidders_filter_sets_bid_response_errors_list(
        &self,
        args: &Adexchangebuyer2BiddersFilterSetsBidResponseErrorsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBidResponseErrorsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_bidders_filter_sets_bid_response_errors_list_builder(
            &self.http_client,
            &args.filterSetName,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_bidders_filter_sets_bid_response_errors_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 bidders filter sets bid responses without bids list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListBidResponsesWithoutBidsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adexchangebuyer2_bidders_filter_sets_bid_responses_without_bids_list(
        &self,
        args: &Adexchangebuyer2BiddersFilterSetsBidResponsesWithoutBidsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBidResponsesWithoutBidsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_bidders_filter_sets_bid_responses_without_bids_list_builder(
            &self.http_client,
            &args.filterSetName,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_bidders_filter_sets_bid_responses_without_bids_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 bidders filter sets filtered bid requests list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListFilteredBidRequestsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adexchangebuyer2_bidders_filter_sets_filtered_bid_requests_list(
        &self,
        args: &Adexchangebuyer2BiddersFilterSetsFilteredBidRequestsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListFilteredBidRequestsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_bidders_filter_sets_filtered_bid_requests_list_builder(
            &self.http_client,
            &args.filterSetName,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_bidders_filter_sets_filtered_bid_requests_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 bidders filter sets filtered bids list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListFilteredBidsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adexchangebuyer2_bidders_filter_sets_filtered_bids_list(
        &self,
        args: &Adexchangebuyer2BiddersFilterSetsFilteredBidsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListFilteredBidsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_bidders_filter_sets_filtered_bids_list_builder(
            &self.http_client,
            &args.filterSetName,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_bidders_filter_sets_filtered_bids_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 bidders filter sets filtered bids creatives list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCreativeStatusBreakdownByCreativeResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adexchangebuyer2_bidders_filter_sets_filtered_bids_creatives_list(
        &self,
        args: &Adexchangebuyer2BiddersFilterSetsFilteredBidsCreativesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCreativeStatusBreakdownByCreativeResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_bidders_filter_sets_filtered_bids_creatives_list_builder(
            &self.http_client,
            &args.filterSetName,
            &args.creativeStatusId,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_bidders_filter_sets_filtered_bids_creatives_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 bidders filter sets filtered bids details list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCreativeStatusBreakdownByDetailResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adexchangebuyer2_bidders_filter_sets_filtered_bids_details_list(
        &self,
        args: &Adexchangebuyer2BiddersFilterSetsFilteredBidsDetailsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCreativeStatusBreakdownByDetailResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_bidders_filter_sets_filtered_bids_details_list_builder(
            &self.http_client,
            &args.filterSetName,
            &args.creativeStatusId,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_bidders_filter_sets_filtered_bids_details_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 bidders filter sets impression metrics list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListImpressionMetricsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adexchangebuyer2_bidders_filter_sets_impression_metrics_list(
        &self,
        args: &Adexchangebuyer2BiddersFilterSetsImpressionMetricsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListImpressionMetricsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_bidders_filter_sets_impression_metrics_list_builder(
            &self.http_client,
            &args.filterSetName,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_bidders_filter_sets_impression_metrics_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 bidders filter sets losing bids list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListLosingBidsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adexchangebuyer2_bidders_filter_sets_losing_bids_list(
        &self,
        args: &Adexchangebuyer2BiddersFilterSetsLosingBidsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLosingBidsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_bidders_filter_sets_losing_bids_list_builder(
            &self.http_client,
            &args.filterSetName,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_bidders_filter_sets_losing_bids_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 bidders filter sets non billable winning bids list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListNonBillableWinningBidsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adexchangebuyer2_bidders_filter_sets_non_billable_winning_bids_list(
        &self,
        args: &Adexchangebuyer2BiddersFilterSetsNonBillableWinningBidsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListNonBillableWinningBidsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_bidders_filter_sets_non_billable_winning_bids_list_builder(
            &self.http_client,
            &args.filterSetName,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_bidders_filter_sets_non_billable_winning_bids_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 buyers filter sets create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FilterSet result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn adexchangebuyer2_buyers_filter_sets_create(
        &self,
        args: &Adexchangebuyer2BuyersFilterSetsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FilterSet, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_buyers_filter_sets_create_builder(
            &self.http_client,
            &args.ownerName,
            &args.isTransient,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_buyers_filter_sets_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 buyers filter sets delete.
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
    pub fn adexchangebuyer2_buyers_filter_sets_delete(
        &self,
        args: &Adexchangebuyer2BuyersFilterSetsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_buyers_filter_sets_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_buyers_filter_sets_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 buyers filter sets get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FilterSet result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adexchangebuyer2_buyers_filter_sets_get(
        &self,
        args: &Adexchangebuyer2BuyersFilterSetsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FilterSet, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_buyers_filter_sets_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_buyers_filter_sets_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 buyers filter sets list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListFilterSetsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adexchangebuyer2_buyers_filter_sets_list(
        &self,
        args: &Adexchangebuyer2BuyersFilterSetsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListFilterSetsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_buyers_filter_sets_list_builder(
            &self.http_client,
            &args.ownerName,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_buyers_filter_sets_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 buyers filter sets bid metrics list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListBidMetricsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adexchangebuyer2_buyers_filter_sets_bid_metrics_list(
        &self,
        args: &Adexchangebuyer2BuyersFilterSetsBidMetricsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBidMetricsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_buyers_filter_sets_bid_metrics_list_builder(
            &self.http_client,
            &args.filterSetName,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_buyers_filter_sets_bid_metrics_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 buyers filter sets bid response errors list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListBidResponseErrorsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adexchangebuyer2_buyers_filter_sets_bid_response_errors_list(
        &self,
        args: &Adexchangebuyer2BuyersFilterSetsBidResponseErrorsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBidResponseErrorsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_buyers_filter_sets_bid_response_errors_list_builder(
            &self.http_client,
            &args.filterSetName,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_buyers_filter_sets_bid_response_errors_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 buyers filter sets bid responses without bids list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListBidResponsesWithoutBidsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adexchangebuyer2_buyers_filter_sets_bid_responses_without_bids_list(
        &self,
        args: &Adexchangebuyer2BuyersFilterSetsBidResponsesWithoutBidsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBidResponsesWithoutBidsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_buyers_filter_sets_bid_responses_without_bids_list_builder(
            &self.http_client,
            &args.filterSetName,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_buyers_filter_sets_bid_responses_without_bids_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 buyers filter sets filtered bid requests list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListFilteredBidRequestsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adexchangebuyer2_buyers_filter_sets_filtered_bid_requests_list(
        &self,
        args: &Adexchangebuyer2BuyersFilterSetsFilteredBidRequestsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListFilteredBidRequestsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_buyers_filter_sets_filtered_bid_requests_list_builder(
            &self.http_client,
            &args.filterSetName,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_buyers_filter_sets_filtered_bid_requests_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 buyers filter sets filtered bids list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListFilteredBidsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adexchangebuyer2_buyers_filter_sets_filtered_bids_list(
        &self,
        args: &Adexchangebuyer2BuyersFilterSetsFilteredBidsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListFilteredBidsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_buyers_filter_sets_filtered_bids_list_builder(
            &self.http_client,
            &args.filterSetName,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_buyers_filter_sets_filtered_bids_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 buyers filter sets filtered bids creatives list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCreativeStatusBreakdownByCreativeResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adexchangebuyer2_buyers_filter_sets_filtered_bids_creatives_list(
        &self,
        args: &Adexchangebuyer2BuyersFilterSetsFilteredBidsCreativesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCreativeStatusBreakdownByCreativeResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_buyers_filter_sets_filtered_bids_creatives_list_builder(
            &self.http_client,
            &args.filterSetName,
            &args.creativeStatusId,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_buyers_filter_sets_filtered_bids_creatives_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 buyers filter sets filtered bids details list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCreativeStatusBreakdownByDetailResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adexchangebuyer2_buyers_filter_sets_filtered_bids_details_list(
        &self,
        args: &Adexchangebuyer2BuyersFilterSetsFilteredBidsDetailsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCreativeStatusBreakdownByDetailResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_buyers_filter_sets_filtered_bids_details_list_builder(
            &self.http_client,
            &args.filterSetName,
            &args.creativeStatusId,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_buyers_filter_sets_filtered_bids_details_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 buyers filter sets impression metrics list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListImpressionMetricsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adexchangebuyer2_buyers_filter_sets_impression_metrics_list(
        &self,
        args: &Adexchangebuyer2BuyersFilterSetsImpressionMetricsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListImpressionMetricsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_buyers_filter_sets_impression_metrics_list_builder(
            &self.http_client,
            &args.filterSetName,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_buyers_filter_sets_impression_metrics_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 buyers filter sets losing bids list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListLosingBidsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adexchangebuyer2_buyers_filter_sets_losing_bids_list(
        &self,
        args: &Adexchangebuyer2BuyersFilterSetsLosingBidsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLosingBidsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_buyers_filter_sets_losing_bids_list_builder(
            &self.http_client,
            &args.filterSetName,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_buyers_filter_sets_losing_bids_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adexchangebuyer2 buyers filter sets non billable winning bids list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListNonBillableWinningBidsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adexchangebuyer2_buyers_filter_sets_non_billable_winning_bids_list(
        &self,
        args: &Adexchangebuyer2BuyersFilterSetsNonBillableWinningBidsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListNonBillableWinningBidsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adexchangebuyer2_buyers_filter_sets_non_billable_winning_bids_list_builder(
            &self.http_client,
            &args.filterSetName,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adexchangebuyer2_buyers_filter_sets_non_billable_winning_bids_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
