//! CivicinfoProvider - State-aware civicinfo API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       civicinfo API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::civicinfo::{
    civicinfo_divisions_query_division_by_address_builder, civicinfo_divisions_query_division_by_address_task,
    civicinfo_divisions_search_builder, civicinfo_divisions_search_task,
    civicinfo_elections_election_query_builder, civicinfo_elections_election_query_task,
    civicinfo_elections_voter_info_query_builder, civicinfo_elections_voter_info_query_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::civicinfo::CivicinfoApiprotosV2DivisionByAddressResponse;
use crate::providers::gcp::clients::civicinfo::CivicinfoApiprotosV2DivisionSearchResponse;
use crate::providers::gcp::clients::civicinfo::CivicinfoApiprotosV2ElectionsQueryResponse;
use crate::providers::gcp::clients::civicinfo::CivicinfoApiprotosV2VoterInfoResponse;
use crate::providers::gcp::clients::civicinfo::CivicinfoDivisionsQueryDivisionByAddressArgs;
use crate::providers::gcp::clients::civicinfo::CivicinfoDivisionsSearchArgs;
use crate::providers::gcp::clients::civicinfo::CivicinfoElectionsElectionQueryArgs;
use crate::providers::gcp::clients::civicinfo::CivicinfoElectionsVoterInfoQueryArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// CivicinfoProvider with automatic state tracking.
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
/// let provider = CivicinfoProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct CivicinfoProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> CivicinfoProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new CivicinfoProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Civicinfo divisions query division by address.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CivicinfoApiprotosV2DivisionByAddressResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn civicinfo_divisions_query_division_by_address(
        &self,
        args: &CivicinfoDivisionsQueryDivisionByAddressArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CivicinfoApiprotosV2DivisionByAddressResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = civicinfo_divisions_query_division_by_address_builder(
            &self.http_client,
            &args.address,
        )
        .map_err(ProviderError::Api)?;

        let task = civicinfo_divisions_query_division_by_address_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Civicinfo divisions search.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CivicinfoApiprotosV2DivisionSearchResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn civicinfo_divisions_search(
        &self,
        args: &CivicinfoDivisionsSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CivicinfoApiprotosV2DivisionSearchResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = civicinfo_divisions_search_builder(
            &self.http_client,
            &args.query,
        )
        .map_err(ProviderError::Api)?;

        let task = civicinfo_divisions_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Civicinfo elections election query.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CivicinfoApiprotosV2ElectionsQueryResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn civicinfo_elections_election_query(
        &self,
        args: &CivicinfoElectionsElectionQueryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CivicinfoApiprotosV2ElectionsQueryResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = civicinfo_elections_election_query_builder(
            &self.http_client,
            &args.productionDataOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = civicinfo_elections_election_query_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Civicinfo elections voter info query.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CivicinfoApiprotosV2VoterInfoResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn civicinfo_elections_voter_info_query(
        &self,
        args: &CivicinfoElectionsVoterInfoQueryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CivicinfoApiprotosV2VoterInfoResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = civicinfo_elections_voter_info_query_builder(
            &self.http_client,
            &args.address,
            &args.electionId,
            &args.officialOnly,
            &args.productionDataOnly,
            &args.returnAllAvailableData,
        )
        .map_err(ProviderError::Api)?;

        let task = civicinfo_elections_voter_info_query_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
