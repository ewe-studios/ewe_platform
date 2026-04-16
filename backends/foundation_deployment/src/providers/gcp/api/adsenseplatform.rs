//! AdsenseplatformProvider - State-aware adsenseplatform API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       adsenseplatform API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::adsenseplatform::{
    adsenseplatform_platforms_accounts_close_builder, adsenseplatform_platforms_accounts_close_task,
    adsenseplatform_platforms_accounts_create_builder, adsenseplatform_platforms_accounts_create_task,
    adsenseplatform_platforms_accounts_get_builder, adsenseplatform_platforms_accounts_get_task,
    adsenseplatform_platforms_accounts_list_builder, adsenseplatform_platforms_accounts_list_task,
    adsenseplatform_platforms_accounts_lookup_builder, adsenseplatform_platforms_accounts_lookup_task,
    adsenseplatform_platforms_accounts_events_create_builder, adsenseplatform_platforms_accounts_events_create_task,
    adsenseplatform_platforms_accounts_sites_create_builder, adsenseplatform_platforms_accounts_sites_create_task,
    adsenseplatform_platforms_accounts_sites_delete_builder, adsenseplatform_platforms_accounts_sites_delete_task,
    adsenseplatform_platforms_accounts_sites_get_builder, adsenseplatform_platforms_accounts_sites_get_task,
    adsenseplatform_platforms_accounts_sites_list_builder, adsenseplatform_platforms_accounts_sites_list_task,
    adsenseplatform_platforms_accounts_sites_request_review_builder, adsenseplatform_platforms_accounts_sites_request_review_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::adsenseplatform::Account;
use crate::providers::gcp::clients::adsenseplatform::CloseAccountResponse;
use crate::providers::gcp::clients::adsenseplatform::Empty;
use crate::providers::gcp::clients::adsenseplatform::Event;
use crate::providers::gcp::clients::adsenseplatform::ListAccountsResponse;
use crate::providers::gcp::clients::adsenseplatform::ListSitesResponse;
use crate::providers::gcp::clients::adsenseplatform::LookupAccountResponse;
use crate::providers::gcp::clients::adsenseplatform::RequestSiteReviewResponse;
use crate::providers::gcp::clients::adsenseplatform::Site;
use crate::providers::gcp::clients::adsenseplatform::AdsenseplatformPlatformsAccountsCloseArgs;
use crate::providers::gcp::clients::adsenseplatform::AdsenseplatformPlatformsAccountsCreateArgs;
use crate::providers::gcp::clients::adsenseplatform::AdsenseplatformPlatformsAccountsEventsCreateArgs;
use crate::providers::gcp::clients::adsenseplatform::AdsenseplatformPlatformsAccountsGetArgs;
use crate::providers::gcp::clients::adsenseplatform::AdsenseplatformPlatformsAccountsListArgs;
use crate::providers::gcp::clients::adsenseplatform::AdsenseplatformPlatformsAccountsLookupArgs;
use crate::providers::gcp::clients::adsenseplatform::AdsenseplatformPlatformsAccountsSitesCreateArgs;
use crate::providers::gcp::clients::adsenseplatform::AdsenseplatformPlatformsAccountsSitesDeleteArgs;
use crate::providers::gcp::clients::adsenseplatform::AdsenseplatformPlatformsAccountsSitesGetArgs;
use crate::providers::gcp::clients::adsenseplatform::AdsenseplatformPlatformsAccountsSitesListArgs;
use crate::providers::gcp::clients::adsenseplatform::AdsenseplatformPlatformsAccountsSitesRequestReviewArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// AdsenseplatformProvider with automatic state tracking.
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
/// let provider = AdsenseplatformProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct AdsenseplatformProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> AdsenseplatformProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new AdsenseplatformProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new AdsenseplatformProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Adsenseplatform platforms accounts close.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CloseAccountResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn adsenseplatform_platforms_accounts_close(
        &self,
        args: &AdsenseplatformPlatformsAccountsCloseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CloseAccountResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsenseplatform_platforms_accounts_close_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = adsenseplatform_platforms_accounts_close_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adsenseplatform platforms accounts create.
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
    pub fn adsenseplatform_platforms_accounts_create(
        &self,
        args: &AdsenseplatformPlatformsAccountsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Account, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsenseplatform_platforms_accounts_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = adsenseplatform_platforms_accounts_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adsenseplatform platforms accounts get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn adsenseplatform_platforms_accounts_get(
        &self,
        args: &AdsenseplatformPlatformsAccountsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Account, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsenseplatform_platforms_accounts_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = adsenseplatform_platforms_accounts_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adsenseplatform platforms accounts list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAccountsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adsenseplatform_platforms_accounts_list(
        &self,
        args: &AdsenseplatformPlatformsAccountsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAccountsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsenseplatform_platforms_accounts_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adsenseplatform_platforms_accounts_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adsenseplatform platforms accounts lookup.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LookupAccountResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adsenseplatform_platforms_accounts_lookup(
        &self,
        args: &AdsenseplatformPlatformsAccountsLookupArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LookupAccountResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsenseplatform_platforms_accounts_lookup_builder(
            &self.http_client,
            &args.parent,
            &args.creationRequestId,
        )
        .map_err(ProviderError::Api)?;

        let task = adsenseplatform_platforms_accounts_lookup_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adsenseplatform platforms accounts events create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Event result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn adsenseplatform_platforms_accounts_events_create(
        &self,
        args: &AdsenseplatformPlatformsAccountsEventsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Event, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsenseplatform_platforms_accounts_events_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = adsenseplatform_platforms_accounts_events_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adsenseplatform platforms accounts sites create.
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
    pub fn adsenseplatform_platforms_accounts_sites_create(
        &self,
        args: &AdsenseplatformPlatformsAccountsSitesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Site, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsenseplatform_platforms_accounts_sites_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = adsenseplatform_platforms_accounts_sites_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adsenseplatform platforms accounts sites delete.
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
    pub fn adsenseplatform_platforms_accounts_sites_delete(
        &self,
        args: &AdsenseplatformPlatformsAccountsSitesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsenseplatform_platforms_accounts_sites_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = adsenseplatform_platforms_accounts_sites_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adsenseplatform platforms accounts sites get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn adsenseplatform_platforms_accounts_sites_get(
        &self,
        args: &AdsenseplatformPlatformsAccountsSitesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Site, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsenseplatform_platforms_accounts_sites_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = adsenseplatform_platforms_accounts_sites_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adsenseplatform platforms accounts sites list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListSitesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adsenseplatform_platforms_accounts_sites_list(
        &self,
        args: &AdsenseplatformPlatformsAccountsSitesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSitesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsenseplatform_platforms_accounts_sites_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adsenseplatform_platforms_accounts_sites_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adsenseplatform platforms accounts sites request review.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RequestSiteReviewResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn adsenseplatform_platforms_accounts_sites_request_review(
        &self,
        args: &AdsenseplatformPlatformsAccountsSitesRequestReviewArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RequestSiteReviewResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsenseplatform_platforms_accounts_sites_request_review_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = adsenseplatform_platforms_accounts_sites_request_review_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
