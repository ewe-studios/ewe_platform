//! SlidesProvider - State-aware slides API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       slides API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::slides::{
    slides_presentations_batch_update_builder, slides_presentations_batch_update_task,
    slides_presentations_create_builder, slides_presentations_create_task,
    slides_presentations_get_builder, slides_presentations_get_task,
    slides_presentations_pages_get_builder, slides_presentations_pages_get_task,
    slides_presentations_pages_get_thumbnail_builder, slides_presentations_pages_get_thumbnail_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::slides::BatchUpdatePresentationResponse;
use crate::providers::gcp::clients::slides::Page;
use crate::providers::gcp::clients::slides::Presentation;
use crate::providers::gcp::clients::slides::Thumbnail;
use crate::providers::gcp::clients::slides::SlidesPresentationsBatchUpdateArgs;
use crate::providers::gcp::clients::slides::SlidesPresentationsGetArgs;
use crate::providers::gcp::clients::slides::SlidesPresentationsPagesGetArgs;
use crate::providers::gcp::clients::slides::SlidesPresentationsPagesGetThumbnailArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// SlidesProvider with automatic state tracking.
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
/// let provider = SlidesProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct SlidesProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> SlidesProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new SlidesProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new SlidesProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Slides presentations batch update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchUpdatePresentationResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn slides_presentations_batch_update(
        &self,
        args: &SlidesPresentationsBatchUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchUpdatePresentationResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = slides_presentations_batch_update_builder(
            &self.http_client,
            &args.presentationId,
        )
        .map_err(ProviderError::Api)?;

        let task = slides_presentations_batch_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Slides presentations create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Presentation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn slides_presentations_create(
        &self,
        args: &SlidesPresentationsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Presentation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = slides_presentations_create_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = slides_presentations_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Slides presentations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Presentation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn slides_presentations_get(
        &self,
        args: &SlidesPresentationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Presentation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = slides_presentations_get_builder(
            &self.http_client,
            &args.presentationId,
        )
        .map_err(ProviderError::Api)?;

        let task = slides_presentations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Slides presentations pages get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Page result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn slides_presentations_pages_get(
        &self,
        args: &SlidesPresentationsPagesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Page, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = slides_presentations_pages_get_builder(
            &self.http_client,
            &args.presentationId,
            &args.pageObjectId,
        )
        .map_err(ProviderError::Api)?;

        let task = slides_presentations_pages_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Slides presentations pages get thumbnail.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Thumbnail result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn slides_presentations_pages_get_thumbnail(
        &self,
        args: &SlidesPresentationsPagesGetThumbnailArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Thumbnail, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = slides_presentations_pages_get_thumbnail_builder(
            &self.http_client,
            &args.presentationId,
            &args.pageObjectId,
            &args.thumbnailProperties_mimeType,
            &args.thumbnailProperties_thumbnailSize,
        )
        .map_err(ProviderError::Api)?;

        let task = slides_presentations_pages_get_thumbnail_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
