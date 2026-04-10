//! SiteVerificationProvider - State-aware siteVerification API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       siteVerification API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::siteVerification::{
    site_verification_web_resource_delete_builder, site_verification_web_resource_delete_task,
    site_verification_web_resource_get_token_builder, site_verification_web_resource_get_token_task,
    site_verification_web_resource_insert_builder, site_verification_web_resource_insert_task,
    site_verification_web_resource_patch_builder, site_verification_web_resource_patch_task,
    site_verification_web_resource_update_builder, site_verification_web_resource_update_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::siteVerification::SiteVerificationWebResourceGettokenResponse;
use crate::providers::gcp::clients::siteVerification::SiteVerificationWebResourceResource;
use crate::providers::gcp::clients::siteVerification::SiteVerificationWebResourceDeleteArgs;
use crate::providers::gcp::clients::siteVerification::SiteVerificationWebResourceGetTokenArgs;
use crate::providers::gcp::clients::siteVerification::SiteVerificationWebResourceInsertArgs;
use crate::providers::gcp::clients::siteVerification::SiteVerificationWebResourcePatchArgs;
use crate::providers::gcp::clients::siteVerification::SiteVerificationWebResourceUpdateArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// SiteVerificationProvider with automatic state tracking.
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
/// let provider = SiteVerificationProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct SiteVerificationProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> SiteVerificationProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new SiteVerificationProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Site verification web resource delete.
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
    pub fn site_verification_web_resource_delete(
        &self,
        args: &SiteVerificationWebResourceDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = site_verification_web_resource_delete_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = site_verification_web_resource_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Site verification web resource get token.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SiteVerificationWebResourceGettokenResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn site_verification_web_resource_get_token(
        &self,
        args: &SiteVerificationWebResourceGetTokenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SiteVerificationWebResourceGettokenResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = site_verification_web_resource_get_token_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = site_verification_web_resource_get_token_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Site verification web resource insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SiteVerificationWebResourceResource result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn site_verification_web_resource_insert(
        &self,
        args: &SiteVerificationWebResourceInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SiteVerificationWebResourceResource, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = site_verification_web_resource_insert_builder(
            &self.http_client,
            &args.verificationMethod,
            &args.verificationMethod,
        )
        .map_err(ProviderError::Api)?;

        let task = site_verification_web_resource_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Site verification web resource patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SiteVerificationWebResourceResource result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn site_verification_web_resource_patch(
        &self,
        args: &SiteVerificationWebResourcePatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SiteVerificationWebResourceResource, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = site_verification_web_resource_patch_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = site_verification_web_resource_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Site verification web resource update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SiteVerificationWebResourceResource result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn site_verification_web_resource_update(
        &self,
        args: &SiteVerificationWebResourceUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SiteVerificationWebResourceResource, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = site_verification_web_resource_update_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = site_verification_web_resource_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
