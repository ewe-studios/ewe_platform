//! AdsenseProvider - State-aware adsense API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       adsense API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::adsense::{
    adsense_accounts_adclients_adunits_create_builder, adsense_accounts_adclients_adunits_create_task,
    adsense_accounts_adclients_adunits_patch_builder, adsense_accounts_adclients_adunits_patch_task,
    adsense_accounts_adclients_customchannels_create_builder, adsense_accounts_adclients_customchannels_create_task,
    adsense_accounts_adclients_customchannels_delete_builder, adsense_accounts_adclients_customchannels_delete_task,
    adsense_accounts_adclients_customchannels_patch_builder, adsense_accounts_adclients_customchannels_patch_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::adsense::AdUnit;
use crate::providers::gcp::clients::adsense::CustomChannel;
use crate::providers::gcp::clients::adsense::Empty;
use crate::providers::gcp::clients::adsense::AdsenseAccountsAdclientsAdunitsCreateArgs;
use crate::providers::gcp::clients::adsense::AdsenseAccountsAdclientsAdunitsPatchArgs;
use crate::providers::gcp::clients::adsense::AdsenseAccountsAdclientsCustomchannelsCreateArgs;
use crate::providers::gcp::clients::adsense::AdsenseAccountsAdclientsCustomchannelsDeleteArgs;
use crate::providers::gcp::clients::adsense::AdsenseAccountsAdclientsCustomchannelsPatchArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// AdsenseProvider with automatic state tracking.
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
/// let provider = AdsenseProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct AdsenseProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> AdsenseProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new AdsenseProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Adsense accounts adclients adunits create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AdUnit result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn adsense_accounts_adclients_adunits_create(
        &self,
        args: &AdsenseAccountsAdclientsAdunitsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AdUnit, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsense_accounts_adclients_adunits_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = adsense_accounts_adclients_adunits_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adsense accounts adclients adunits patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AdUnit result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn adsense_accounts_adclients_adunits_patch(
        &self,
        args: &AdsenseAccountsAdclientsAdunitsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AdUnit, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsense_accounts_adclients_adunits_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = adsense_accounts_adclients_adunits_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adsense accounts adclients customchannels create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CustomChannel result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn adsense_accounts_adclients_customchannels_create(
        &self,
        args: &AdsenseAccountsAdclientsCustomchannelsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CustomChannel, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsense_accounts_adclients_customchannels_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = adsense_accounts_adclients_customchannels_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adsense accounts adclients customchannels delete.
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
    pub fn adsense_accounts_adclients_customchannels_delete(
        &self,
        args: &AdsenseAccountsAdclientsCustomchannelsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsense_accounts_adclients_customchannels_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = adsense_accounts_adclients_customchannels_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adsense accounts adclients customchannels patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CustomChannel result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn adsense_accounts_adclients_customchannels_patch(
        &self,
        args: &AdsenseAccountsAdclientsCustomchannelsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CustomChannel, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsense_accounts_adclients_customchannels_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = adsense_accounts_adclients_customchannels_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
