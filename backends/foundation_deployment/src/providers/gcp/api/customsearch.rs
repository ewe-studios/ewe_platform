//! CustomsearchProvider - State-aware customsearch API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       customsearch API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::customsearch::{
    search_cse_list_builder, search_cse_list_task,
    search_cse_siterestrict_list_builder, search_cse_siterestrict_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::customsearch::Search;
use crate::providers::gcp::clients::customsearch::SearchCseListArgs;
use crate::providers::gcp::clients::customsearch::SearchCseSiterestrictListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// CustomsearchProvider with automatic state tracking.
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
/// let provider = CustomsearchProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct CustomsearchProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> CustomsearchProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new CustomsearchProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Search cse list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Search result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn search_cse_list(
        &self,
        args: &SearchCseListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Search, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = search_cse_list_builder(
            &self.http_client,
            &args.c2coff,
            &args.cr,
            &args.cx,
            &args.dateRestrict,
            &args.enableAlternateSearchHandler,
            &args.exactTerms,
            &args.excludeTerms,
            &args.fileType,
            &args.filter,
            &args.gl,
            &args.googlehost,
            &args.highRange,
            &args.hl,
            &args.hq,
            &args.imgColorType,
            &args.imgDominantColor,
            &args.imgSize,
            &args.imgType,
            &args.linkSite,
            &args.lowRange,
            &args.lr,
            &args.num,
            &args.orTerms,
            &args.q,
            &args.relatedSite,
            &args.rights,
            &args.safe,
            &args.searchType,
            &args.siteSearch,
            &args.siteSearchFilter,
            &args.snippetLength,
            &args.sort,
            &args.start,
        )
        .map_err(ProviderError::Api)?;

        let task = search_cse_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Search cse siterestrict list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Search result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn search_cse_siterestrict_list(
        &self,
        args: &SearchCseSiterestrictListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Search, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = search_cse_siterestrict_list_builder(
            &self.http_client,
            &args.c2coff,
            &args.cr,
            &args.cx,
            &args.dateRestrict,
            &args.enableAlternateSearchHandler,
            &args.exactTerms,
            &args.excludeTerms,
            &args.fileType,
            &args.filter,
            &args.gl,
            &args.googlehost,
            &args.highRange,
            &args.hl,
            &args.hq,
            &args.imgColorType,
            &args.imgDominantColor,
            &args.imgSize,
            &args.imgType,
            &args.linkSite,
            &args.lowRange,
            &args.lr,
            &args.num,
            &args.orTerms,
            &args.q,
            &args.relatedSite,
            &args.rights,
            &args.safe,
            &args.searchType,
            &args.siteSearch,
            &args.siteSearchFilter,
            &args.snippetLength,
            &args.sort,
            &args.start,
        )
        .map_err(ProviderError::Api)?;

        let task = search_cse_siterestrict_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
