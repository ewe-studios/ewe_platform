//! StreetviewpublishProvider - State-aware streetviewpublish API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       streetviewpublish API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::streetviewpublish::{
    streetviewpublish_photo_create_builder, streetviewpublish_photo_create_task,
    streetviewpublish_photo_delete_builder, streetviewpublish_photo_delete_task,
    streetviewpublish_photo_get_builder, streetviewpublish_photo_get_task,
    streetviewpublish_photo_start_upload_builder, streetviewpublish_photo_start_upload_task,
    streetviewpublish_photo_update_builder, streetviewpublish_photo_update_task,
    streetviewpublish_photo_sequence_create_builder, streetviewpublish_photo_sequence_create_task,
    streetviewpublish_photo_sequence_delete_builder, streetviewpublish_photo_sequence_delete_task,
    streetviewpublish_photo_sequence_get_builder, streetviewpublish_photo_sequence_get_task,
    streetviewpublish_photo_sequence_start_upload_builder, streetviewpublish_photo_sequence_start_upload_task,
    streetviewpublish_photo_sequences_list_builder, streetviewpublish_photo_sequences_list_task,
    streetviewpublish_photos_batch_delete_builder, streetviewpublish_photos_batch_delete_task,
    streetviewpublish_photos_batch_get_builder, streetviewpublish_photos_batch_get_task,
    streetviewpublish_photos_batch_update_builder, streetviewpublish_photos_batch_update_task,
    streetviewpublish_photos_list_builder, streetviewpublish_photos_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::streetviewpublish::BatchDeletePhotosResponse;
use crate::providers::gcp::clients::streetviewpublish::BatchGetPhotosResponse;
use crate::providers::gcp::clients::streetviewpublish::BatchUpdatePhotosResponse;
use crate::providers::gcp::clients::streetviewpublish::Empty;
use crate::providers::gcp::clients::streetviewpublish::ListPhotoSequencesResponse;
use crate::providers::gcp::clients::streetviewpublish::ListPhotosResponse;
use crate::providers::gcp::clients::streetviewpublish::Operation;
use crate::providers::gcp::clients::streetviewpublish::Photo;
use crate::providers::gcp::clients::streetviewpublish::UploadRef;
use crate::providers::gcp::clients::streetviewpublish::StreetviewpublishPhotoCreateArgs;
use crate::providers::gcp::clients::streetviewpublish::StreetviewpublishPhotoDeleteArgs;
use crate::providers::gcp::clients::streetviewpublish::StreetviewpublishPhotoGetArgs;
use crate::providers::gcp::clients::streetviewpublish::StreetviewpublishPhotoSequenceCreateArgs;
use crate::providers::gcp::clients::streetviewpublish::StreetviewpublishPhotoSequenceDeleteArgs;
use crate::providers::gcp::clients::streetviewpublish::StreetviewpublishPhotoSequenceGetArgs;
use crate::providers::gcp::clients::streetviewpublish::StreetviewpublishPhotoSequenceStartUploadArgs;
use crate::providers::gcp::clients::streetviewpublish::StreetviewpublishPhotoSequencesListArgs;
use crate::providers::gcp::clients::streetviewpublish::StreetviewpublishPhotoStartUploadArgs;
use crate::providers::gcp::clients::streetviewpublish::StreetviewpublishPhotoUpdateArgs;
use crate::providers::gcp::clients::streetviewpublish::StreetviewpublishPhotosBatchDeleteArgs;
use crate::providers::gcp::clients::streetviewpublish::StreetviewpublishPhotosBatchGetArgs;
use crate::providers::gcp::clients::streetviewpublish::StreetviewpublishPhotosBatchUpdateArgs;
use crate::providers::gcp::clients::streetviewpublish::StreetviewpublishPhotosListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// StreetviewpublishProvider with automatic state tracking.
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
/// let provider = StreetviewpublishProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct StreetviewpublishProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> StreetviewpublishProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new StreetviewpublishProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Streetviewpublish photo create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Photo result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn streetviewpublish_photo_create(
        &self,
        args: &StreetviewpublishPhotoCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Photo, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = streetviewpublish_photo_create_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = streetviewpublish_photo_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Streetviewpublish photo delete.
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
    pub fn streetviewpublish_photo_delete(
        &self,
        args: &StreetviewpublishPhotoDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = streetviewpublish_photo_delete_builder(
            &self.http_client,
            &args.photoId,
        )
        .map_err(ProviderError::Api)?;

        let task = streetviewpublish_photo_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Streetviewpublish photo get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Photo result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn streetviewpublish_photo_get(
        &self,
        args: &StreetviewpublishPhotoGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Photo, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = streetviewpublish_photo_get_builder(
            &self.http_client,
            &args.photoId,
            &args.languageCode,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = streetviewpublish_photo_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Streetviewpublish photo start upload.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UploadRef result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn streetviewpublish_photo_start_upload(
        &self,
        args: &StreetviewpublishPhotoStartUploadArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UploadRef, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = streetviewpublish_photo_start_upload_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = streetviewpublish_photo_start_upload_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Streetviewpublish photo update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Photo result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn streetviewpublish_photo_update(
        &self,
        args: &StreetviewpublishPhotoUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Photo, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = streetviewpublish_photo_update_builder(
            &self.http_client,
            &args.id,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = streetviewpublish_photo_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Streetviewpublish photo sequence create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn streetviewpublish_photo_sequence_create(
        &self,
        args: &StreetviewpublishPhotoSequenceCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = streetviewpublish_photo_sequence_create_builder(
            &self.http_client,
            &args.inputType,
        )
        .map_err(ProviderError::Api)?;

        let task = streetviewpublish_photo_sequence_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Streetviewpublish photo sequence delete.
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
    pub fn streetviewpublish_photo_sequence_delete(
        &self,
        args: &StreetviewpublishPhotoSequenceDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = streetviewpublish_photo_sequence_delete_builder(
            &self.http_client,
            &args.sequenceId,
        )
        .map_err(ProviderError::Api)?;

        let task = streetviewpublish_photo_sequence_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Streetviewpublish photo sequence get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn streetviewpublish_photo_sequence_get(
        &self,
        args: &StreetviewpublishPhotoSequenceGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = streetviewpublish_photo_sequence_get_builder(
            &self.http_client,
            &args.sequenceId,
            &args.filter,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = streetviewpublish_photo_sequence_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Streetviewpublish photo sequence start upload.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UploadRef result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn streetviewpublish_photo_sequence_start_upload(
        &self,
        args: &StreetviewpublishPhotoSequenceStartUploadArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UploadRef, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = streetviewpublish_photo_sequence_start_upload_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = streetviewpublish_photo_sequence_start_upload_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Streetviewpublish photo sequences list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListPhotoSequencesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn streetviewpublish_photo_sequences_list(
        &self,
        args: &StreetviewpublishPhotoSequencesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListPhotoSequencesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = streetviewpublish_photo_sequences_list_builder(
            &self.http_client,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = streetviewpublish_photo_sequences_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Streetviewpublish photos batch delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchDeletePhotosResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn streetviewpublish_photos_batch_delete(
        &self,
        args: &StreetviewpublishPhotosBatchDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchDeletePhotosResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = streetviewpublish_photos_batch_delete_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = streetviewpublish_photos_batch_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Streetviewpublish photos batch get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchGetPhotosResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn streetviewpublish_photos_batch_get(
        &self,
        args: &StreetviewpublishPhotosBatchGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchGetPhotosResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = streetviewpublish_photos_batch_get_builder(
            &self.http_client,
            &args.languageCode,
            &args.photoIds,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = streetviewpublish_photos_batch_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Streetviewpublish photos batch update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchUpdatePhotosResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn streetviewpublish_photos_batch_update(
        &self,
        args: &StreetviewpublishPhotosBatchUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchUpdatePhotosResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = streetviewpublish_photos_batch_update_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = streetviewpublish_photos_batch_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Streetviewpublish photos list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListPhotosResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn streetviewpublish_photos_list(
        &self,
        args: &StreetviewpublishPhotosListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListPhotosResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = streetviewpublish_photos_list_builder(
            &self.http_client,
            &args.filter,
            &args.languageCode,
            &args.pageSize,
            &args.pageToken,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = streetviewpublish_photos_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
