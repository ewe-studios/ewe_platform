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
    adexchangebuyer2_accounts_clients_update_builder, adexchangebuyer2_accounts_clients_update_task,
    adexchangebuyer2_accounts_clients_invitations_create_builder, adexchangebuyer2_accounts_clients_invitations_create_task,
    adexchangebuyer2_accounts_clients_users_update_builder, adexchangebuyer2_accounts_clients_users_update_task,
    adexchangebuyer2_accounts_creatives_create_builder, adexchangebuyer2_accounts_creatives_create_task,
    adexchangebuyer2_accounts_creatives_stop_watching_builder, adexchangebuyer2_accounts_creatives_stop_watching_task,
    adexchangebuyer2_accounts_creatives_update_builder, adexchangebuyer2_accounts_creatives_update_task,
    adexchangebuyer2_accounts_creatives_watch_builder, adexchangebuyer2_accounts_creatives_watch_task,
    adexchangebuyer2_accounts_creatives_deal_associations_add_builder, adexchangebuyer2_accounts_creatives_deal_associations_add_task,
    adexchangebuyer2_accounts_creatives_deal_associations_remove_builder, adexchangebuyer2_accounts_creatives_deal_associations_remove_task,
    adexchangebuyer2_accounts_finalized_proposals_pause_builder, adexchangebuyer2_accounts_finalized_proposals_pause_task,
    adexchangebuyer2_accounts_finalized_proposals_resume_builder, adexchangebuyer2_accounts_finalized_proposals_resume_task,
    adexchangebuyer2_accounts_proposals_accept_builder, adexchangebuyer2_accounts_proposals_accept_task,
    adexchangebuyer2_accounts_proposals_add_note_builder, adexchangebuyer2_accounts_proposals_add_note_task,
    adexchangebuyer2_accounts_proposals_cancel_negotiation_builder, adexchangebuyer2_accounts_proposals_cancel_negotiation_task,
    adexchangebuyer2_accounts_proposals_complete_setup_builder, adexchangebuyer2_accounts_proposals_complete_setup_task,
    adexchangebuyer2_accounts_proposals_create_builder, adexchangebuyer2_accounts_proposals_create_task,
    adexchangebuyer2_accounts_proposals_pause_builder, adexchangebuyer2_accounts_proposals_pause_task,
    adexchangebuyer2_accounts_proposals_resume_builder, adexchangebuyer2_accounts_proposals_resume_task,
    adexchangebuyer2_accounts_proposals_update_builder, adexchangebuyer2_accounts_proposals_update_task,
    adexchangebuyer2_bidders_accounts_filter_sets_create_builder, adexchangebuyer2_bidders_accounts_filter_sets_create_task,
    adexchangebuyer2_bidders_accounts_filter_sets_delete_builder, adexchangebuyer2_bidders_accounts_filter_sets_delete_task,
    adexchangebuyer2_bidders_filter_sets_create_builder, adexchangebuyer2_bidders_filter_sets_create_task,
    adexchangebuyer2_bidders_filter_sets_delete_builder, adexchangebuyer2_bidders_filter_sets_delete_task,
    adexchangebuyer2_buyers_filter_sets_create_builder, adexchangebuyer2_buyers_filter_sets_create_task,
    adexchangebuyer2_buyers_filter_sets_delete_builder, adexchangebuyer2_buyers_filter_sets_delete_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::adexchangebuyer2::Client;
use crate::providers::gcp::clients::adexchangebuyer2::ClientUser;
use crate::providers::gcp::clients::adexchangebuyer2::ClientUserInvitation;
use crate::providers::gcp::clients::adexchangebuyer2::Creative;
use crate::providers::gcp::clients::adexchangebuyer2::Empty;
use crate::providers::gcp::clients::adexchangebuyer2::FilterSet;
use crate::providers::gcp::clients::adexchangebuyer2::Note;
use crate::providers::gcp::clients::adexchangebuyer2::Proposal;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsClientsCreateArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsClientsInvitationsCreateArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsClientsUpdateArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsClientsUsersUpdateArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsCreativesCreateArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsCreativesDealAssociationsAddArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsCreativesDealAssociationsRemoveArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsCreativesStopWatchingArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsCreativesUpdateArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsCreativesWatchArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsFinalizedProposalsPauseArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsFinalizedProposalsResumeArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsProposalsAcceptArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsProposalsAddNoteArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsProposalsCancelNegotiationArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsProposalsCompleteSetupArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsProposalsCreateArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsProposalsPauseArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsProposalsResumeArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2AccountsProposalsUpdateArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2BiddersAccountsFilterSetsCreateArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2BiddersAccountsFilterSetsDeleteArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2BiddersFilterSetsCreateArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2BiddersFilterSetsDeleteArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2BuyersFilterSetsCreateArgs;
use crate::providers::gcp::clients::adexchangebuyer2::Adexchangebuyer2BuyersFilterSetsDeleteArgs;
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

}
