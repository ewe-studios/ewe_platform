//! Oauth2Provider - State-aware oauth2 API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       oauth2 API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::oauth2::{
    oauth2_userinfo_get_builder, oauth2_userinfo_get_task,
    oauth2_userinfo_v2_me_get_builder, oauth2_userinfo_v2_me_get_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::oauth2::Userinfo;
use crate::providers::gcp::clients::oauth2::Oauth2UserinfoGetArgs;
use crate::providers::gcp::clients::oauth2::Oauth2UserinfoV2MeGetArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// Oauth2Provider with automatic state tracking.
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
/// let provider = Oauth2Provider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct Oauth2Provider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> Oauth2Provider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new Oauth2Provider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Oauth2 userinfo get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Userinfo result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn oauth2_userinfo_get(
        &self,
        args: &Oauth2UserinfoGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Userinfo, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oauth2_userinfo_get_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = oauth2_userinfo_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oauth2 userinfo v2 me get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Userinfo result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn oauth2_userinfo_v2_me_get(
        &self,
        args: &Oauth2UserinfoV2MeGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Userinfo, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oauth2_userinfo_v2_me_get_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = oauth2_userinfo_v2_me_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
