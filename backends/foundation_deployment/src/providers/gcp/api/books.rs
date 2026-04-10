//! BooksProvider - State-aware books API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       books API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::books::{
    books_cloudloading_add_book_builder, books_cloudloading_add_book_task,
    books_cloudloading_delete_book_builder, books_cloudloading_delete_book_task,
    books_cloudloading_update_book_builder, books_cloudloading_update_book_task,
    books_familysharing_share_builder, books_familysharing_share_task,
    books_familysharing_unshare_builder, books_familysharing_unshare_task,
    books_myconfig_release_download_access_builder, books_myconfig_release_download_access_task,
    books_myconfig_request_access_builder, books_myconfig_request_access_task,
    books_myconfig_sync_volume_licenses_builder, books_myconfig_sync_volume_licenses_task,
    books_myconfig_update_user_settings_builder, books_myconfig_update_user_settings_task,
    books_mylibrary_annotations_delete_builder, books_mylibrary_annotations_delete_task,
    books_mylibrary_annotations_insert_builder, books_mylibrary_annotations_insert_task,
    books_mylibrary_annotations_summary_builder, books_mylibrary_annotations_summary_task,
    books_mylibrary_annotations_update_builder, books_mylibrary_annotations_update_task,
    books_mylibrary_bookshelves_add_volume_builder, books_mylibrary_bookshelves_add_volume_task,
    books_mylibrary_bookshelves_clear_volumes_builder, books_mylibrary_bookshelves_clear_volumes_task,
    books_mylibrary_bookshelves_move_volume_builder, books_mylibrary_bookshelves_move_volume_task,
    books_mylibrary_bookshelves_remove_volume_builder, books_mylibrary_bookshelves_remove_volume_task,
    books_mylibrary_readingpositions_set_position_builder, books_mylibrary_readingpositions_set_position_task,
    books_promooffer_accept_builder, books_promooffer_accept_task,
    books_promooffer_dismiss_builder, books_promooffer_dismiss_task,
    books_volumes_recommended_rate_builder, books_volumes_recommended_rate_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::books::Annotation;
use crate::providers::gcp::clients::books::AnnotationsSummary;
use crate::providers::gcp::clients::books::BooksCloudloadingResource;
use crate::providers::gcp::clients::books::BooksVolumesRecommendedRateResponse;
use crate::providers::gcp::clients::books::DownloadAccesses;
use crate::providers::gcp::clients::books::Empty;
use crate::providers::gcp::clients::books::RequestAccessData;
use crate::providers::gcp::clients::books::Usersettings;
use crate::providers::gcp::clients::books::Volumes;
use crate::providers::gcp::clients::books::BooksCloudloadingAddBookArgs;
use crate::providers::gcp::clients::books::BooksCloudloadingDeleteBookArgs;
use crate::providers::gcp::clients::books::BooksCloudloadingUpdateBookArgs;
use crate::providers::gcp::clients::books::BooksFamilysharingShareArgs;
use crate::providers::gcp::clients::books::BooksFamilysharingUnshareArgs;
use crate::providers::gcp::clients::books::BooksMyconfigReleaseDownloadAccessArgs;
use crate::providers::gcp::clients::books::BooksMyconfigRequestAccessArgs;
use crate::providers::gcp::clients::books::BooksMyconfigSyncVolumeLicensesArgs;
use crate::providers::gcp::clients::books::BooksMyconfigUpdateUserSettingsArgs;
use crate::providers::gcp::clients::books::BooksMylibraryAnnotationsDeleteArgs;
use crate::providers::gcp::clients::books::BooksMylibraryAnnotationsInsertArgs;
use crate::providers::gcp::clients::books::BooksMylibraryAnnotationsSummaryArgs;
use crate::providers::gcp::clients::books::BooksMylibraryAnnotationsUpdateArgs;
use crate::providers::gcp::clients::books::BooksMylibraryBookshelvesAddVolumeArgs;
use crate::providers::gcp::clients::books::BooksMylibraryBookshelvesClearVolumesArgs;
use crate::providers::gcp::clients::books::BooksMylibraryBookshelvesMoveVolumeArgs;
use crate::providers::gcp::clients::books::BooksMylibraryBookshelvesRemoveVolumeArgs;
use crate::providers::gcp::clients::books::BooksMylibraryReadingpositionsSetPositionArgs;
use crate::providers::gcp::clients::books::BooksPromoofferAcceptArgs;
use crate::providers::gcp::clients::books::BooksPromoofferDismissArgs;
use crate::providers::gcp::clients::books::BooksVolumesRecommendedRateArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// BooksProvider with automatic state tracking.
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
/// let provider = BooksProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct BooksProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> BooksProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new BooksProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Books cloudloading add book.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BooksCloudloadingResource result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn books_cloudloading_add_book(
        &self,
        args: &BooksCloudloadingAddBookArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BooksCloudloadingResource, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_cloudloading_add_book_builder(
            &self.http_client,
            &args.drive_document_id,
            &args.mime_type,
            &args.name,
            &args.upload_client_token,
        )
        .map_err(ProviderError::Api)?;

        let task = books_cloudloading_add_book_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Books cloudloading delete book.
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
    pub fn books_cloudloading_delete_book(
        &self,
        args: &BooksCloudloadingDeleteBookArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_cloudloading_delete_book_builder(
            &self.http_client,
            &args.volumeId,
            &args.volumeId,
        )
        .map_err(ProviderError::Api)?;

        let task = books_cloudloading_delete_book_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Books cloudloading update book.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BooksCloudloadingResource result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn books_cloudloading_update_book(
        &self,
        args: &BooksCloudloadingUpdateBookArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BooksCloudloadingResource, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_cloudloading_update_book_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = books_cloudloading_update_book_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Books familysharing share.
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
    pub fn books_familysharing_share(
        &self,
        args: &BooksFamilysharingShareArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_familysharing_share_builder(
            &self.http_client,
            &args.docId,
            &args.source,
            &args.volumeId,
        )
        .map_err(ProviderError::Api)?;

        let task = books_familysharing_share_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Books familysharing unshare.
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
    pub fn books_familysharing_unshare(
        &self,
        args: &BooksFamilysharingUnshareArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_familysharing_unshare_builder(
            &self.http_client,
            &args.docId,
            &args.source,
            &args.volumeId,
        )
        .map_err(ProviderError::Api)?;

        let task = books_familysharing_unshare_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Books myconfig release download access.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DownloadAccesses result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn books_myconfig_release_download_access(
        &self,
        args: &BooksMyconfigReleaseDownloadAccessArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DownloadAccesses, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_myconfig_release_download_access_builder(
            &self.http_client,
            &args.cpksver,
            &args.volumeIds,
            &args.cpksver,
            &args.locale,
            &args.source,
            &args.volumeIds,
        )
        .map_err(ProviderError::Api)?;

        let task = books_myconfig_release_download_access_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Books myconfig request access.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RequestAccessData result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn books_myconfig_request_access(
        &self,
        args: &BooksMyconfigRequestAccessArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RequestAccessData, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_myconfig_request_access_builder(
            &self.http_client,
            &args.cpksver,
            &args.nonce,
            &args.source,
            &args.volumeId,
            &args.cpksver,
            &args.licenseTypes,
            &args.locale,
            &args.nonce,
            &args.source,
            &args.volumeId,
        )
        .map_err(ProviderError::Api)?;

        let task = books_myconfig_request_access_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Books myconfig sync volume licenses.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Volumes result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn books_myconfig_sync_volume_licenses(
        &self,
        args: &BooksMyconfigSyncVolumeLicensesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Volumes, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_myconfig_sync_volume_licenses_builder(
            &self.http_client,
            &args.cpksver,
            &args.nonce,
            &args.source,
            &args.cpksver,
            &args.features,
            &args.includeNonComicsSeries,
            &args.locale,
            &args.nonce,
            &args.showPreorders,
            &args.source,
            &args.volumeIds,
        )
        .map_err(ProviderError::Api)?;

        let task = books_myconfig_sync_volume_licenses_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Books myconfig update user settings.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Usersettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn books_myconfig_update_user_settings(
        &self,
        args: &BooksMyconfigUpdateUserSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Usersettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_myconfig_update_user_settings_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = books_myconfig_update_user_settings_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Books mylibrary annotations delete.
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
    pub fn books_mylibrary_annotations_delete(
        &self,
        args: &BooksMylibraryAnnotationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_mylibrary_annotations_delete_builder(
            &self.http_client,
            &args.annotationId,
            &args.source,
        )
        .map_err(ProviderError::Api)?;

        let task = books_mylibrary_annotations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Books mylibrary annotations insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Annotation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn books_mylibrary_annotations_insert(
        &self,
        args: &BooksMylibraryAnnotationsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Annotation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_mylibrary_annotations_insert_builder(
            &self.http_client,
            &args.annotationId,
            &args.country,
            &args.showOnlySummaryInResponse,
            &args.source,
        )
        .map_err(ProviderError::Api)?;

        let task = books_mylibrary_annotations_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Books mylibrary annotations summary.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AnnotationsSummary result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn books_mylibrary_annotations_summary(
        &self,
        args: &BooksMylibraryAnnotationsSummaryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AnnotationsSummary, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_mylibrary_annotations_summary_builder(
            &self.http_client,
            &args.layerIds,
            &args.volumeId,
            &args.layerIds,
            &args.source,
            &args.volumeId,
        )
        .map_err(ProviderError::Api)?;

        let task = books_mylibrary_annotations_summary_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Books mylibrary annotations update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Annotation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn books_mylibrary_annotations_update(
        &self,
        args: &BooksMylibraryAnnotationsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Annotation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_mylibrary_annotations_update_builder(
            &self.http_client,
            &args.annotationId,
            &args.source,
        )
        .map_err(ProviderError::Api)?;

        let task = books_mylibrary_annotations_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Books mylibrary bookshelves add volume.
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
    pub fn books_mylibrary_bookshelves_add_volume(
        &self,
        args: &BooksMylibraryBookshelvesAddVolumeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_mylibrary_bookshelves_add_volume_builder(
            &self.http_client,
            &args.shelf,
            &args.volumeId,
            &args.reason,
            &args.source,
            &args.volumeId,
        )
        .map_err(ProviderError::Api)?;

        let task = books_mylibrary_bookshelves_add_volume_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Books mylibrary bookshelves clear volumes.
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
    pub fn books_mylibrary_bookshelves_clear_volumes(
        &self,
        args: &BooksMylibraryBookshelvesClearVolumesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_mylibrary_bookshelves_clear_volumes_builder(
            &self.http_client,
            &args.shelf,
            &args.source,
        )
        .map_err(ProviderError::Api)?;

        let task = books_mylibrary_bookshelves_clear_volumes_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Books mylibrary bookshelves move volume.
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
    pub fn books_mylibrary_bookshelves_move_volume(
        &self,
        args: &BooksMylibraryBookshelvesMoveVolumeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_mylibrary_bookshelves_move_volume_builder(
            &self.http_client,
            &args.shelf,
            &args.volumeId,
            &args.volumePosition,
            &args.source,
            &args.volumeId,
            &args.volumePosition,
        )
        .map_err(ProviderError::Api)?;

        let task = books_mylibrary_bookshelves_move_volume_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Books mylibrary bookshelves remove volume.
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
    pub fn books_mylibrary_bookshelves_remove_volume(
        &self,
        args: &BooksMylibraryBookshelvesRemoveVolumeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_mylibrary_bookshelves_remove_volume_builder(
            &self.http_client,
            &args.shelf,
            &args.volumeId,
            &args.reason,
            &args.source,
            &args.volumeId,
        )
        .map_err(ProviderError::Api)?;

        let task = books_mylibrary_bookshelves_remove_volume_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Books mylibrary readingpositions set position.
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
    pub fn books_mylibrary_readingpositions_set_position(
        &self,
        args: &BooksMylibraryReadingpositionsSetPositionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_mylibrary_readingpositions_set_position_builder(
            &self.http_client,
            &args.volumeId,
            &args.position,
            &args.timestamp,
            &args.action,
            &args.contentVersion,
            &args.deviceCookie,
            &args.position,
            &args.source,
            &args.timestamp,
        )
        .map_err(ProviderError::Api)?;

        let task = books_mylibrary_readingpositions_set_position_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Books promooffer accept.
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
    pub fn books_promooffer_accept(
        &self,
        args: &BooksPromoofferAcceptArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_promooffer_accept_builder(
            &self.http_client,
            &args.androidId,
            &args.device,
            &args.manufacturer,
            &args.model,
            &args.offerId,
            &args.product,
            &args.serial,
            &args.volumeId,
        )
        .map_err(ProviderError::Api)?;

        let task = books_promooffer_accept_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Books promooffer dismiss.
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
    pub fn books_promooffer_dismiss(
        &self,
        args: &BooksPromoofferDismissArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_promooffer_dismiss_builder(
            &self.http_client,
            &args.androidId,
            &args.device,
            &args.manufacturer,
            &args.model,
            &args.offerId,
            &args.product,
            &args.serial,
        )
        .map_err(ProviderError::Api)?;

        let task = books_promooffer_dismiss_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Books volumes recommended rate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BooksVolumesRecommendedRateResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn books_volumes_recommended_rate(
        &self,
        args: &BooksVolumesRecommendedRateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BooksVolumesRecommendedRateResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_volumes_recommended_rate_builder(
            &self.http_client,
            &args.rating,
            &args.volumeId,
            &args.locale,
            &args.rating,
            &args.source,
            &args.volumeId,
        )
        .map_err(ProviderError::Api)?;

        let task = books_volumes_recommended_rate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
