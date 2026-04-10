//! ChromewebstoreProvider - State-aware chromewebstore API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       chromewebstore API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::chromewebstore::{
    chromewebstore_media_upload_builder, chromewebstore_media_upload_task,
    chromewebstore_publishers_items_cancel_submission_builder, chromewebstore_publishers_items_cancel_submission_task,
    chromewebstore_publishers_items_publish_builder, chromewebstore_publishers_items_publish_task,
    chromewebstore_publishers_items_set_published_deploy_percentage_builder, chromewebstore_publishers_items_set_published_deploy_percentage_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::chromewebstore::CancelSubmissionResponse;
use crate::providers::gcp::clients::chromewebstore::PublishItemResponse;
use crate::providers::gcp::clients::chromewebstore::SetPublishedDeployPercentageResponse;
use crate::providers::gcp::clients::chromewebstore::UploadItemPackageResponse;
use crate::providers::gcp::clients::chromewebstore::ChromewebstoreMediaUploadArgs;
use crate::providers::gcp::clients::chromewebstore::ChromewebstorePublishersItemsCancelSubmissionArgs;
use crate::providers::gcp::clients::chromewebstore::ChromewebstorePublishersItemsPublishArgs;
use crate::providers::gcp::clients::chromewebstore::ChromewebstorePublishersItemsSetPublishedDeployPercentageArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// ChromewebstoreProvider with automatic state tracking.
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
/// let provider = ChromewebstoreProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct ChromewebstoreProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> ChromewebstoreProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new ChromewebstoreProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Chromewebstore media upload.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UploadItemPackageResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chromewebstore_media_upload(
        &self,
        args: &ChromewebstoreMediaUploadArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UploadItemPackageResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromewebstore_media_upload_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = chromewebstore_media_upload_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromewebstore publishers items cancel submission.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CancelSubmissionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chromewebstore_publishers_items_cancel_submission(
        &self,
        args: &ChromewebstorePublishersItemsCancelSubmissionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CancelSubmissionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromewebstore_publishers_items_cancel_submission_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = chromewebstore_publishers_items_cancel_submission_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromewebstore publishers items publish.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PublishItemResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chromewebstore_publishers_items_publish(
        &self,
        args: &ChromewebstorePublishersItemsPublishArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PublishItemResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromewebstore_publishers_items_publish_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = chromewebstore_publishers_items_publish_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromewebstore publishers items set published deploy percentage.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SetPublishedDeployPercentageResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chromewebstore_publishers_items_set_published_deploy_percentage(
        &self,
        args: &ChromewebstorePublishersItemsSetPublishedDeployPercentageArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SetPublishedDeployPercentageResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromewebstore_publishers_items_set_published_deploy_percentage_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = chromewebstore_publishers_items_set_published_deploy_percentage_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
