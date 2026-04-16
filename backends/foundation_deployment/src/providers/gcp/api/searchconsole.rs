//! SearchconsoleProvider - State-aware searchconsole API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       searchconsole API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::searchconsole::{
    webmasters_searchanalytics_query_builder, webmasters_searchanalytics_query_task,
    webmasters_sitemaps_delete_builder, webmasters_sitemaps_delete_task,
    webmasters_sitemaps_get_builder, webmasters_sitemaps_get_task,
    webmasters_sitemaps_list_builder, webmasters_sitemaps_list_task,
    webmasters_sitemaps_submit_builder, webmasters_sitemaps_submit_task,
    webmasters_sites_add_builder, webmasters_sites_add_task,
    webmasters_sites_delete_builder, webmasters_sites_delete_task,
    webmasters_sites_get_builder, webmasters_sites_get_task,
    webmasters_sites_list_builder, webmasters_sites_list_task,
    searchconsole_url_inspection_index_inspect_builder, searchconsole_url_inspection_index_inspect_task,
    searchconsole_url_testing_tools_mobile_friendly_test_run_builder, searchconsole_url_testing_tools_mobile_friendly_test_run_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::searchconsole::InspectUrlIndexResponse;
use crate::providers::gcp::clients::searchconsole::RunMobileFriendlyTestResponse;
use crate::providers::gcp::clients::searchconsole::SearchAnalyticsQueryResponse;
use crate::providers::gcp::clients::searchconsole::SitemapsListResponse;
use crate::providers::gcp::clients::searchconsole::SitesListResponse;
use crate::providers::gcp::clients::searchconsole::WmxSite;
use crate::providers::gcp::clients::searchconsole::WmxSitemap;
use crate::providers::gcp::clients::searchconsole::WebmastersSearchanalyticsQueryArgs;
use crate::providers::gcp::clients::searchconsole::WebmastersSitemapsDeleteArgs;
use crate::providers::gcp::clients::searchconsole::WebmastersSitemapsGetArgs;
use crate::providers::gcp::clients::searchconsole::WebmastersSitemapsListArgs;
use crate::providers::gcp::clients::searchconsole::WebmastersSitemapsSubmitArgs;
use crate::providers::gcp::clients::searchconsole::WebmastersSitesAddArgs;
use crate::providers::gcp::clients::searchconsole::WebmastersSitesDeleteArgs;
use crate::providers::gcp::clients::searchconsole::WebmastersSitesGetArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// SearchconsoleProvider with automatic state tracking.
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
/// let provider = SearchconsoleProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct SearchconsoleProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> SearchconsoleProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new SearchconsoleProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new SearchconsoleProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Webmasters searchanalytics query.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchAnalyticsQueryResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn webmasters_searchanalytics_query(
        &self,
        args: &WebmastersSearchanalyticsQueryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchAnalyticsQueryResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = webmasters_searchanalytics_query_builder(
            &self.http_client,
            &args.siteUrl,
        )
        .map_err(ProviderError::Api)?;

        let task = webmasters_searchanalytics_query_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Webmasters sitemaps delete.
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
    pub fn webmasters_sitemaps_delete(
        &self,
        args: &WebmastersSitemapsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = webmasters_sitemaps_delete_builder(
            &self.http_client,
            &args.siteUrl,
            &args.feedpath,
        )
        .map_err(ProviderError::Api)?;

        let task = webmasters_sitemaps_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Webmasters sitemaps get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WmxSitemap result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn webmasters_sitemaps_get(
        &self,
        args: &WebmastersSitemapsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WmxSitemap, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = webmasters_sitemaps_get_builder(
            &self.http_client,
            &args.siteUrl,
            &args.feedpath,
        )
        .map_err(ProviderError::Api)?;

        let task = webmasters_sitemaps_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Webmasters sitemaps list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SitemapsListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn webmasters_sitemaps_list(
        &self,
        args: &WebmastersSitemapsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SitemapsListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = webmasters_sitemaps_list_builder(
            &self.http_client,
            &args.siteUrl,
            &args.sitemapIndex,
        )
        .map_err(ProviderError::Api)?;

        let task = webmasters_sitemaps_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Webmasters sitemaps submit.
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
    pub fn webmasters_sitemaps_submit(
        &self,
        args: &WebmastersSitemapsSubmitArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = webmasters_sitemaps_submit_builder(
            &self.http_client,
            &args.siteUrl,
            &args.feedpath,
        )
        .map_err(ProviderError::Api)?;

        let task = webmasters_sitemaps_submit_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Webmasters sites add.
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
    pub fn webmasters_sites_add(
        &self,
        args: &WebmastersSitesAddArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = webmasters_sites_add_builder(
            &self.http_client,
            &args.siteUrl,
        )
        .map_err(ProviderError::Api)?;

        let task = webmasters_sites_add_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Webmasters sites delete.
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
    pub fn webmasters_sites_delete(
        &self,
        args: &WebmastersSitesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = webmasters_sites_delete_builder(
            &self.http_client,
            &args.siteUrl,
        )
        .map_err(ProviderError::Api)?;

        let task = webmasters_sites_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Webmasters sites get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WmxSite result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn webmasters_sites_get(
        &self,
        args: &WebmastersSitesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WmxSite, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = webmasters_sites_get_builder(
            &self.http_client,
            &args.siteUrl,
        )
        .map_err(ProviderError::Api)?;

        let task = webmasters_sites_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Webmasters sites list.
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
    pub fn webmasters_sites_list(
        &self,
        args: &WebmastersSitesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SitesListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = webmasters_sites_list_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = webmasters_sites_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Searchconsole url inspection index inspect.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the InspectUrlIndexResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn searchconsole_url_inspection_index_inspect(
        &self,
        args: &SearchconsoleUrlInspectionIndexInspectArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<InspectUrlIndexResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = searchconsole_url_inspection_index_inspect_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = searchconsole_url_inspection_index_inspect_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Searchconsole url testing tools mobile friendly test run.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RunMobileFriendlyTestResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn searchconsole_url_testing_tools_mobile_friendly_test_run(
        &self,
        args: &SearchconsoleUrlTestingToolsMobileFriendlyTestRunArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RunMobileFriendlyTestResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = searchconsole_url_testing_tools_mobile_friendly_test_run_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = searchconsole_url_testing_tools_mobile_friendly_test_run_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
