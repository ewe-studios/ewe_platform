//! MerchantapiProvider - State-aware merchantapi API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       merchantapi API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::merchantapi::{
    merchantapi_accounts_merchant_reviews_delete_builder, merchantapi_accounts_merchant_reviews_delete_task,
    merchantapi_accounts_merchant_reviews_insert_builder, merchantapi_accounts_merchant_reviews_insert_task,
    merchantapi_accounts_product_reviews_delete_builder, merchantapi_accounts_product_reviews_delete_task,
    merchantapi_accounts_product_reviews_insert_builder, merchantapi_accounts_product_reviews_insert_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::merchantapi::Empty;
use crate::providers::gcp::clients::merchantapi::MerchantReview;
use crate::providers::gcp::clients::merchantapi::ProductReview;
use crate::providers::gcp::clients::merchantapi::MerchantapiAccountsMerchantReviewsDeleteArgs;
use crate::providers::gcp::clients::merchantapi::MerchantapiAccountsMerchantReviewsInsertArgs;
use crate::providers::gcp::clients::merchantapi::MerchantapiAccountsProductReviewsDeleteArgs;
use crate::providers::gcp::clients::merchantapi::MerchantapiAccountsProductReviewsInsertArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// MerchantapiProvider with automatic state tracking.
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
/// let provider = MerchantapiProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct MerchantapiProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> MerchantapiProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new MerchantapiProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Merchantapi accounts merchant reviews delete.
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
    pub fn merchantapi_accounts_merchant_reviews_delete(
        &self,
        args: &MerchantapiAccountsMerchantReviewsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = merchantapi_accounts_merchant_reviews_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = merchantapi_accounts_merchant_reviews_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Merchantapi accounts merchant reviews insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the MerchantReview result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn merchantapi_accounts_merchant_reviews_insert(
        &self,
        args: &MerchantapiAccountsMerchantReviewsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<MerchantReview, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = merchantapi_accounts_merchant_reviews_insert_builder(
            &self.http_client,
            &args.parent,
            &args.dataSource,
        )
        .map_err(ProviderError::Api)?;

        let task = merchantapi_accounts_merchant_reviews_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Merchantapi accounts product reviews delete.
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
    pub fn merchantapi_accounts_product_reviews_delete(
        &self,
        args: &MerchantapiAccountsProductReviewsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = merchantapi_accounts_product_reviews_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = merchantapi_accounts_product_reviews_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Merchantapi accounts product reviews insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProductReview result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn merchantapi_accounts_product_reviews_insert(
        &self,
        args: &MerchantapiAccountsProductReviewsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProductReview, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = merchantapi_accounts_product_reviews_insert_builder(
            &self.http_client,
            &args.parent,
            &args.dataSource,
        )
        .map_err(ProviderError::Api)?;

        let task = merchantapi_accounts_product_reviews_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
