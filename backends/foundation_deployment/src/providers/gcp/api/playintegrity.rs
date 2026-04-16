//! PlayintegrityProvider - State-aware playintegrity API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       playintegrity API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::playintegrity::{
    playintegrity_device_recall_write_builder, playintegrity_device_recall_write_task,
    playintegrity_decode_integrity_token_builder, playintegrity_decode_integrity_token_task,
    playintegrity_decode_pc_integrity_token_builder, playintegrity_decode_pc_integrity_token_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::playintegrity::DecodeIntegrityTokenResponse;
use crate::providers::gcp::clients::playintegrity::DecodePcIntegrityTokenResponse;
use crate::providers::gcp::clients::playintegrity::WriteDeviceRecallResponse;
use crate::providers::gcp::clients::playintegrity::PlayintegrityDecodeIntegrityTokenArgs;
use crate::providers::gcp::clients::playintegrity::PlayintegrityDecodePcIntegrityTokenArgs;
use crate::providers::gcp::clients::playintegrity::PlayintegrityDeviceRecallWriteArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// PlayintegrityProvider with automatic state tracking.
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
/// let provider = PlayintegrityProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct PlayintegrityProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> PlayintegrityProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new PlayintegrityProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new PlayintegrityProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Playintegrity device recall write.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WriteDeviceRecallResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn playintegrity_device_recall_write(
        &self,
        args: &PlayintegrityDeviceRecallWriteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WriteDeviceRecallResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = playintegrity_device_recall_write_builder(
            &self.http_client,
            &args.packageName,
        )
        .map_err(ProviderError::Api)?;

        let task = playintegrity_device_recall_write_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Playintegrity decode integrity token.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DecodeIntegrityTokenResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn playintegrity_decode_integrity_token(
        &self,
        args: &PlayintegrityDecodeIntegrityTokenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DecodeIntegrityTokenResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = playintegrity_decode_integrity_token_builder(
            &self.http_client,
            &args.packageName,
        )
        .map_err(ProviderError::Api)?;

        let task = playintegrity_decode_integrity_token_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Playintegrity decode pc integrity token.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DecodePcIntegrityTokenResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn playintegrity_decode_pc_integrity_token(
        &self,
        args: &PlayintegrityDecodePcIntegrityTokenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DecodePcIntegrityTokenResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = playintegrity_decode_pc_integrity_token_builder(
            &self.http_client,
            &args.packageName,
        )
        .map_err(ProviderError::Api)?;

        let task = playintegrity_decode_pc_integrity_token_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
