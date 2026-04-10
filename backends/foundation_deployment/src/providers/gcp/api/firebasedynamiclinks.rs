//! FirebasedynamiclinksProvider - State-aware firebasedynamiclinks API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       firebasedynamiclinks API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::firebasedynamiclinks::{
    firebasedynamiclinks_managed_short_links_create_builder, firebasedynamiclinks_managed_short_links_create_task,
    firebasedynamiclinks_short_links_create_builder, firebasedynamiclinks_short_links_create_task,
    firebasedynamiclinks_install_attribution_builder, firebasedynamiclinks_install_attribution_task,
    firebasedynamiclinks_reopen_attribution_builder, firebasedynamiclinks_reopen_attribution_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::firebasedynamiclinks::CreateManagedShortLinkResponse;
use crate::providers::gcp::clients::firebasedynamiclinks::CreateShortDynamicLinkResponse;
use crate::providers::gcp::clients::firebasedynamiclinks::GetIosPostInstallAttributionResponse;
use crate::providers::gcp::clients::firebasedynamiclinks::GetIosReopenAttributionResponse;
use crate::providers::gcp::clients::firebasedynamiclinks::FirebasedynamiclinksInstallAttributionArgs;
use crate::providers::gcp::clients::firebasedynamiclinks::FirebasedynamiclinksManagedShortLinksCreateArgs;
use crate::providers::gcp::clients::firebasedynamiclinks::FirebasedynamiclinksReopenAttributionArgs;
use crate::providers::gcp::clients::firebasedynamiclinks::FirebasedynamiclinksShortLinksCreateArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// FirebasedynamiclinksProvider with automatic state tracking.
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
/// let provider = FirebasedynamiclinksProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct FirebasedynamiclinksProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> FirebasedynamiclinksProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new FirebasedynamiclinksProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Firebasedynamiclinks managed short links create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CreateManagedShortLinkResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebasedynamiclinks_managed_short_links_create(
        &self,
        args: &FirebasedynamiclinksManagedShortLinksCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CreateManagedShortLinkResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebasedynamiclinks_managed_short_links_create_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = firebasedynamiclinks_managed_short_links_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebasedynamiclinks short links create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CreateShortDynamicLinkResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebasedynamiclinks_short_links_create(
        &self,
        args: &FirebasedynamiclinksShortLinksCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CreateShortDynamicLinkResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebasedynamiclinks_short_links_create_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = firebasedynamiclinks_short_links_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebasedynamiclinks install attribution.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GetIosPostInstallAttributionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebasedynamiclinks_install_attribution(
        &self,
        args: &FirebasedynamiclinksInstallAttributionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GetIosPostInstallAttributionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebasedynamiclinks_install_attribution_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = firebasedynamiclinks_install_attribution_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebasedynamiclinks reopen attribution.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GetIosReopenAttributionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebasedynamiclinks_reopen_attribution(
        &self,
        args: &FirebasedynamiclinksReopenAttributionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GetIosReopenAttributionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebasedynamiclinks_reopen_attribution_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = firebasedynamiclinks_reopen_attribution_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
