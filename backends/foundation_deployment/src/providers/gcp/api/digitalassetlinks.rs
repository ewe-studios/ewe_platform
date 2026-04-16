//! DigitalassetlinksProvider - State-aware digitalassetlinks API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       digitalassetlinks API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::digitalassetlinks::{
    digitalassetlinks_assetlinks_bulk_check_builder, digitalassetlinks_assetlinks_bulk_check_task,
    digitalassetlinks_assetlinks_check_builder, digitalassetlinks_assetlinks_check_task,
    digitalassetlinks_statements_list_builder, digitalassetlinks_statements_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::digitalassetlinks::BulkCheckResponse;
use crate::providers::gcp::clients::digitalassetlinks::CheckResponse;
use crate::providers::gcp::clients::digitalassetlinks::ListResponse;
use crate::providers::gcp::clients::digitalassetlinks::DigitalassetlinksAssetlinksCheckArgs;
use crate::providers::gcp::clients::digitalassetlinks::DigitalassetlinksStatementsListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// DigitalassetlinksProvider with automatic state tracking.
///
/// # Type Parameters
///
/// * `S` - StateStore implementation (FileStateStore, SqliteStateStore, etc.)
/// * `R` - DNS resolver type for HTTP client
///
/// # Example
///
/// ```rust
/// let state_store = FileStateStore::new("/path", "my-project", "dev");
/// let http_client = SimpleHttpClient::with_resolver(StaticSocketAddr::new(addr));
/// let client = ProviderClient::new("my-project", "dev", state_store, http_client);
/// let provider = DigitalassetlinksProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct DigitalassetlinksProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> DigitalassetlinksProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new DigitalassetlinksProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new DigitalassetlinksProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Digitalassetlinks assetlinks bulk check.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BulkCheckResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn digitalassetlinks_assetlinks_bulk_check(
        &self,
        args: &DigitalassetlinksAssetlinksBulkCheckArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BulkCheckResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = digitalassetlinks_assetlinks_bulk_check_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = digitalassetlinks_assetlinks_bulk_check_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Digitalassetlinks assetlinks check.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CheckResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn digitalassetlinks_assetlinks_check(
        &self,
        args: &DigitalassetlinksAssetlinksCheckArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CheckResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = digitalassetlinks_assetlinks_check_builder(
            &self.http_client,
            &args.relation,
            &args.returnRelationExtensions,
            &args.source_androidApp_certificate_sha256Fingerprint,
            &args.source_androidApp_packageName,
            &args.source_web_site,
            &args.target_androidApp_certificate_sha256Fingerprint,
            &args.target_androidApp_packageName,
            &args.target_web_site,
        )
        .map_err(ProviderError::Api)?;

        let task = digitalassetlinks_assetlinks_check_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Digitalassetlinks statements list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn digitalassetlinks_statements_list(
        &self,
        args: &DigitalassetlinksStatementsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = digitalassetlinks_statements_list_builder(
            &self.http_client,
            &args.relation,
            &args.returnRelationExtensions,
            &args.source_androidApp_certificate_sha256Fingerprint,
            &args.source_androidApp_packageName,
            &args.source_web_site,
        )
        .map_err(ProviderError::Api)?;

        let task = digitalassetlinks_statements_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
