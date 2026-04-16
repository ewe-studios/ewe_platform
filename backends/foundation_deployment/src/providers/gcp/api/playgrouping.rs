//! PlaygroupingProvider - State-aware playgrouping API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       playgrouping API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::playgrouping::{
    playgrouping_apps_tokens_verify_builder, playgrouping_apps_tokens_verify_task,
    playgrouping_apps_tokens_tags_create_or_update_builder, playgrouping_apps_tokens_tags_create_or_update_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::playgrouping::CreateOrUpdateTagsResponse;
use crate::providers::gcp::clients::playgrouping::VerifyTokenResponse;
use crate::providers::gcp::clients::playgrouping::PlaygroupingAppsTokensTagsCreateOrUpdateArgs;
use crate::providers::gcp::clients::playgrouping::PlaygroupingAppsTokensVerifyArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// PlaygroupingProvider with automatic state tracking.
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
/// let provider = PlaygroupingProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct PlaygroupingProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> PlaygroupingProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new PlaygroupingProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new PlaygroupingProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Playgrouping apps tokens verify.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the VerifyTokenResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn playgrouping_apps_tokens_verify(
        &self,
        args: &PlaygroupingAppsTokensVerifyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<VerifyTokenResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = playgrouping_apps_tokens_verify_builder(
            &self.http_client,
            &args.appPackage,
            &args.token,
        )
        .map_err(ProviderError::Api)?;

        let task = playgrouping_apps_tokens_verify_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Playgrouping apps tokens tags create or update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CreateOrUpdateTagsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn playgrouping_apps_tokens_tags_create_or_update(
        &self,
        args: &PlaygroupingAppsTokensTagsCreateOrUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CreateOrUpdateTagsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = playgrouping_apps_tokens_tags_create_or_update_builder(
            &self.http_client,
            &args.appPackage,
            &args.token,
        )
        .map_err(ProviderError::Api)?;

        let task = playgrouping_apps_tokens_tags_create_or_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
