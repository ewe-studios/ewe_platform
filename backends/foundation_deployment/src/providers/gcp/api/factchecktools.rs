//! FactchecktoolsProvider - State-aware factchecktools API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       factchecktools API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::factchecktools::{
    factchecktools_claims_image_search_builder, factchecktools_claims_image_search_task,
    factchecktools_claims_search_builder, factchecktools_claims_search_task,
    factchecktools_pages_create_builder, factchecktools_pages_create_task,
    factchecktools_pages_delete_builder, factchecktools_pages_delete_task,
    factchecktools_pages_get_builder, factchecktools_pages_get_task,
    factchecktools_pages_list_builder, factchecktools_pages_list_task,
    factchecktools_pages_update_builder, factchecktools_pages_update_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::factchecktools::GoogleFactcheckingFactchecktoolsV1alpha1ClaimReviewMarkupPage;
use crate::providers::gcp::clients::factchecktools::GoogleFactcheckingFactchecktoolsV1alpha1FactCheckedClaimImageSearchResponse;
use crate::providers::gcp::clients::factchecktools::GoogleFactcheckingFactchecktoolsV1alpha1FactCheckedClaimSearchResponse;
use crate::providers::gcp::clients::factchecktools::GoogleFactcheckingFactchecktoolsV1alpha1ListClaimReviewMarkupPagesResponse;
use crate::providers::gcp::clients::factchecktools::GoogleProtobufEmpty;
use crate::providers::gcp::clients::factchecktools::FactchecktoolsClaimsImageSearchArgs;
use crate::providers::gcp::clients::factchecktools::FactchecktoolsClaimsSearchArgs;
use crate::providers::gcp::clients::factchecktools::FactchecktoolsPagesCreateArgs;
use crate::providers::gcp::clients::factchecktools::FactchecktoolsPagesDeleteArgs;
use crate::providers::gcp::clients::factchecktools::FactchecktoolsPagesGetArgs;
use crate::providers::gcp::clients::factchecktools::FactchecktoolsPagesListArgs;
use crate::providers::gcp::clients::factchecktools::FactchecktoolsPagesUpdateArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// FactchecktoolsProvider with automatic state tracking.
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
/// let provider = FactchecktoolsProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct FactchecktoolsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> FactchecktoolsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new FactchecktoolsProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Factchecktools claims image search.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFactcheckingFactchecktoolsV1alpha1FactCheckedClaimImageSearchResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn factchecktools_claims_image_search(
        &self,
        args: &FactchecktoolsClaimsImageSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFactcheckingFactchecktoolsV1alpha1FactCheckedClaimImageSearchResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = factchecktools_claims_image_search_builder(
            &self.http_client,
            &args.imageUri,
            &args.languageCode,
            &args.offset,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = factchecktools_claims_image_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Factchecktools claims search.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFactcheckingFactchecktoolsV1alpha1FactCheckedClaimSearchResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn factchecktools_claims_search(
        &self,
        args: &FactchecktoolsClaimsSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFactcheckingFactchecktoolsV1alpha1FactCheckedClaimSearchResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = factchecktools_claims_search_builder(
            &self.http_client,
            &args.languageCode,
            &args.maxAgeDays,
            &args.offset,
            &args.pageSize,
            &args.pageToken,
            &args.query,
            &args.reviewPublisherSiteFilter,
        )
        .map_err(ProviderError::Api)?;

        let task = factchecktools_claims_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Factchecktools pages create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFactcheckingFactchecktoolsV1alpha1ClaimReviewMarkupPage result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn factchecktools_pages_create(
        &self,
        args: &FactchecktoolsPagesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFactcheckingFactchecktoolsV1alpha1ClaimReviewMarkupPage, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = factchecktools_pages_create_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = factchecktools_pages_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Factchecktools pages delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn factchecktools_pages_delete(
        &self,
        args: &FactchecktoolsPagesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = factchecktools_pages_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = factchecktools_pages_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Factchecktools pages get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFactcheckingFactchecktoolsV1alpha1ClaimReviewMarkupPage result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn factchecktools_pages_get(
        &self,
        args: &FactchecktoolsPagesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFactcheckingFactchecktoolsV1alpha1ClaimReviewMarkupPage, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = factchecktools_pages_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = factchecktools_pages_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Factchecktools pages list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFactcheckingFactchecktoolsV1alpha1ListClaimReviewMarkupPagesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn factchecktools_pages_list(
        &self,
        args: &FactchecktoolsPagesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFactcheckingFactchecktoolsV1alpha1ListClaimReviewMarkupPagesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = factchecktools_pages_list_builder(
            &self.http_client,
            &args.offset,
            &args.organization,
            &args.pageSize,
            &args.pageToken,
            &args.url,
        )
        .map_err(ProviderError::Api)?;

        let task = factchecktools_pages_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Factchecktools pages update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFactcheckingFactchecktoolsV1alpha1ClaimReviewMarkupPage result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn factchecktools_pages_update(
        &self,
        args: &FactchecktoolsPagesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFactcheckingFactchecktoolsV1alpha1ClaimReviewMarkupPage, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = factchecktools_pages_update_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = factchecktools_pages_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
