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
    books_bookshelves_get_builder, books_bookshelves_get_task,
    books_bookshelves_list_builder, books_bookshelves_list_task,
    books_bookshelves_volumes_list_builder, books_bookshelves_volumes_list_task,
    books_cloudloading_add_book_builder, books_cloudloading_add_book_task,
    books_cloudloading_delete_book_builder, books_cloudloading_delete_book_task,
    books_cloudloading_update_book_builder, books_cloudloading_update_book_task,
    books_dictionary_list_offline_metadata_builder, books_dictionary_list_offline_metadata_task,
    books_familysharing_get_family_info_builder, books_familysharing_get_family_info_task,
    books_familysharing_share_builder, books_familysharing_share_task,
    books_familysharing_unshare_builder, books_familysharing_unshare_task,
    books_layers_get_builder, books_layers_get_task,
    books_layers_list_builder, books_layers_list_task,
    books_layers_annotation_data_get_builder, books_layers_annotation_data_get_task,
    books_layers_annotation_data_list_builder, books_layers_annotation_data_list_task,
    books_layers_volume_annotations_get_builder, books_layers_volume_annotations_get_task,
    books_layers_volume_annotations_list_builder, books_layers_volume_annotations_list_task,
    books_myconfig_get_user_settings_builder, books_myconfig_get_user_settings_task,
    books_myconfig_release_download_access_builder, books_myconfig_release_download_access_task,
    books_myconfig_request_access_builder, books_myconfig_request_access_task,
    books_myconfig_sync_volume_licenses_builder, books_myconfig_sync_volume_licenses_task,
    books_myconfig_update_user_settings_builder, books_myconfig_update_user_settings_task,
    books_mylibrary_annotations_delete_builder, books_mylibrary_annotations_delete_task,
    books_mylibrary_annotations_insert_builder, books_mylibrary_annotations_insert_task,
    books_mylibrary_annotations_list_builder, books_mylibrary_annotations_list_task,
    books_mylibrary_annotations_summary_builder, books_mylibrary_annotations_summary_task,
    books_mylibrary_annotations_update_builder, books_mylibrary_annotations_update_task,
    books_mylibrary_bookshelves_add_volume_builder, books_mylibrary_bookshelves_add_volume_task,
    books_mylibrary_bookshelves_clear_volumes_builder, books_mylibrary_bookshelves_clear_volumes_task,
    books_mylibrary_bookshelves_get_builder, books_mylibrary_bookshelves_get_task,
    books_mylibrary_bookshelves_list_builder, books_mylibrary_bookshelves_list_task,
    books_mylibrary_bookshelves_move_volume_builder, books_mylibrary_bookshelves_move_volume_task,
    books_mylibrary_bookshelves_remove_volume_builder, books_mylibrary_bookshelves_remove_volume_task,
    books_mylibrary_bookshelves_volumes_list_builder, books_mylibrary_bookshelves_volumes_list_task,
    books_mylibrary_readingpositions_get_builder, books_mylibrary_readingpositions_get_task,
    books_mylibrary_readingpositions_set_position_builder, books_mylibrary_readingpositions_set_position_task,
    books_notification_get_builder, books_notification_get_task,
    books_onboarding_list_categories_builder, books_onboarding_list_categories_task,
    books_onboarding_list_category_volumes_builder, books_onboarding_list_category_volumes_task,
    books_personalizedstream_get_builder, books_personalizedstream_get_task,
    books_promooffer_accept_builder, books_promooffer_accept_task,
    books_promooffer_dismiss_builder, books_promooffer_dismiss_task,
    books_promooffer_get_builder, books_promooffer_get_task,
    books_series_get_builder, books_series_get_task,
    books_series_membership_get_builder, books_series_membership_get_task,
    books_volumes_get_builder, books_volumes_get_task,
    books_volumes_list_builder, books_volumes_list_task,
    books_volumes_associated_list_builder, books_volumes_associated_list_task,
    books_volumes_mybooks_list_builder, books_volumes_mybooks_list_task,
    books_volumes_recommended_list_builder, books_volumes_recommended_list_task,
    books_volumes_recommended_rate_builder, books_volumes_recommended_rate_task,
    books_volumes_useruploaded_list_builder, books_volumes_useruploaded_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::books::Annotation;
use crate::providers::gcp::clients::books::Annotations;
use crate::providers::gcp::clients::books::AnnotationsSummary;
use crate::providers::gcp::clients::books::Annotationsdata;
use crate::providers::gcp::clients::books::BooksCloudloadingResource;
use crate::providers::gcp::clients::books::BooksVolumesRecommendedRateResponse;
use crate::providers::gcp::clients::books::Bookshelf;
use crate::providers::gcp::clients::books::Bookshelves;
use crate::providers::gcp::clients::books::Category;
use crate::providers::gcp::clients::books::DictionaryAnnotationdata;
use crate::providers::gcp::clients::books::Discoveryclusters;
use crate::providers::gcp::clients::books::DownloadAccesses;
use crate::providers::gcp::clients::books::Empty;
use crate::providers::gcp::clients::books::FamilyInfo;
use crate::providers::gcp::clients::books::Layersummaries;
use crate::providers::gcp::clients::books::Layersummary;
use crate::providers::gcp::clients::books::Metadata;
use crate::providers::gcp::clients::books::Notification;
use crate::providers::gcp::clients::books::Offers;
use crate::providers::gcp::clients::books::ReadingPosition;
use crate::providers::gcp::clients::books::RequestAccessData;
use crate::providers::gcp::clients::books::Series;
use crate::providers::gcp::clients::books::Seriesmembership;
use crate::providers::gcp::clients::books::Usersettings;
use crate::providers::gcp::clients::books::Volume;
use crate::providers::gcp::clients::books::Volume2;
use crate::providers::gcp::clients::books::Volumeannotation;
use crate::providers::gcp::clients::books::Volumeannotations;
use crate::providers::gcp::clients::books::Volumes;
use crate::providers::gcp::clients::books::BooksBookshelvesGetArgs;
use crate::providers::gcp::clients::books::BooksBookshelvesListArgs;
use crate::providers::gcp::clients::books::BooksBookshelvesVolumesListArgs;
use crate::providers::gcp::clients::books::BooksCloudloadingAddBookArgs;
use crate::providers::gcp::clients::books::BooksCloudloadingDeleteBookArgs;
use crate::providers::gcp::clients::books::BooksCloudloadingUpdateBookArgs;
use crate::providers::gcp::clients::books::BooksDictionaryListOfflineMetadataArgs;
use crate::providers::gcp::clients::books::BooksFamilysharingGetFamilyInfoArgs;
use crate::providers::gcp::clients::books::BooksFamilysharingShareArgs;
use crate::providers::gcp::clients::books::BooksFamilysharingUnshareArgs;
use crate::providers::gcp::clients::books::BooksLayersAnnotationDataGetArgs;
use crate::providers::gcp::clients::books::BooksLayersAnnotationDataListArgs;
use crate::providers::gcp::clients::books::BooksLayersGetArgs;
use crate::providers::gcp::clients::books::BooksLayersListArgs;
use crate::providers::gcp::clients::books::BooksLayersVolumeAnnotationsGetArgs;
use crate::providers::gcp::clients::books::BooksLayersVolumeAnnotationsListArgs;
use crate::providers::gcp::clients::books::BooksMyconfigGetUserSettingsArgs;
use crate::providers::gcp::clients::books::BooksMyconfigReleaseDownloadAccessArgs;
use crate::providers::gcp::clients::books::BooksMyconfigRequestAccessArgs;
use crate::providers::gcp::clients::books::BooksMyconfigSyncVolumeLicensesArgs;
use crate::providers::gcp::clients::books::BooksMyconfigUpdateUserSettingsArgs;
use crate::providers::gcp::clients::books::BooksMylibraryAnnotationsDeleteArgs;
use crate::providers::gcp::clients::books::BooksMylibraryAnnotationsInsertArgs;
use crate::providers::gcp::clients::books::BooksMylibraryAnnotationsListArgs;
use crate::providers::gcp::clients::books::BooksMylibraryAnnotationsSummaryArgs;
use crate::providers::gcp::clients::books::BooksMylibraryAnnotationsUpdateArgs;
use crate::providers::gcp::clients::books::BooksMylibraryBookshelvesAddVolumeArgs;
use crate::providers::gcp::clients::books::BooksMylibraryBookshelvesClearVolumesArgs;
use crate::providers::gcp::clients::books::BooksMylibraryBookshelvesGetArgs;
use crate::providers::gcp::clients::books::BooksMylibraryBookshelvesListArgs;
use crate::providers::gcp::clients::books::BooksMylibraryBookshelvesMoveVolumeArgs;
use crate::providers::gcp::clients::books::BooksMylibraryBookshelvesRemoveVolumeArgs;
use crate::providers::gcp::clients::books::BooksMylibraryBookshelvesVolumesListArgs;
use crate::providers::gcp::clients::books::BooksMylibraryReadingpositionsGetArgs;
use crate::providers::gcp::clients::books::BooksMylibraryReadingpositionsSetPositionArgs;
use crate::providers::gcp::clients::books::BooksNotificationGetArgs;
use crate::providers::gcp::clients::books::BooksOnboardingListCategoriesArgs;
use crate::providers::gcp::clients::books::BooksOnboardingListCategoryVolumesArgs;
use crate::providers::gcp::clients::books::BooksPersonalizedstreamGetArgs;
use crate::providers::gcp::clients::books::BooksPromoofferAcceptArgs;
use crate::providers::gcp::clients::books::BooksPromoofferDismissArgs;
use crate::providers::gcp::clients::books::BooksPromoofferGetArgs;
use crate::providers::gcp::clients::books::BooksSeriesGetArgs;
use crate::providers::gcp::clients::books::BooksSeriesMembershipGetArgs;
use crate::providers::gcp::clients::books::BooksVolumesAssociatedListArgs;
use crate::providers::gcp::clients::books::BooksVolumesGetArgs;
use crate::providers::gcp::clients::books::BooksVolumesListArgs;
use crate::providers::gcp::clients::books::BooksVolumesMybooksListArgs;
use crate::providers::gcp::clients::books::BooksVolumesRecommendedListArgs;
use crate::providers::gcp::clients::books::BooksVolumesRecommendedRateArgs;
use crate::providers::gcp::clients::books::BooksVolumesUseruploadedListArgs;
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

    /// Books bookshelves get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Bookshelf result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn books_bookshelves_get(
        &self,
        args: &BooksBookshelvesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Bookshelf, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_bookshelves_get_builder(
            &self.http_client,
            &args.userId,
            &args.shelf,
            &args.source,
        )
        .map_err(ProviderError::Api)?;

        let task = books_bookshelves_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Books bookshelves list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Bookshelves result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn books_bookshelves_list(
        &self,
        args: &BooksBookshelvesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Bookshelves, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_bookshelves_list_builder(
            &self.http_client,
            &args.userId,
            &args.source,
        )
        .map_err(ProviderError::Api)?;

        let task = books_bookshelves_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Books bookshelves volumes list.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn books_bookshelves_volumes_list(
        &self,
        args: &BooksBookshelvesVolumesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Volumes, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_bookshelves_volumes_list_builder(
            &self.http_client,
            &args.userId,
            &args.shelf,
            &args.maxResults,
            &args.showPreorders,
            &args.source,
            &args.startIndex,
        )
        .map_err(ProviderError::Api)?;

        let task = books_bookshelves_volumes_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Books dictionary list offline metadata.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Metadata result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn books_dictionary_list_offline_metadata(
        &self,
        args: &BooksDictionaryListOfflineMetadataArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Metadata, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_dictionary_list_offline_metadata_builder(
            &self.http_client,
            &args.cpksver,
        )
        .map_err(ProviderError::Api)?;

        let task = books_dictionary_list_offline_metadata_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Books familysharing get family info.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FamilyInfo result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn books_familysharing_get_family_info(
        &self,
        args: &BooksFamilysharingGetFamilyInfoArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FamilyInfo, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_familysharing_get_family_info_builder(
            &self.http_client,
            &args.source,
        )
        .map_err(ProviderError::Api)?;

        let task = books_familysharing_get_family_info_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Books layers get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Layersummary result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn books_layers_get(
        &self,
        args: &BooksLayersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Layersummary, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_layers_get_builder(
            &self.http_client,
            &args.volumeId,
            &args.summaryId,
            &args.contentVersion,
            &args.source,
        )
        .map_err(ProviderError::Api)?;

        let task = books_layers_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Books layers list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Layersummaries result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn books_layers_list(
        &self,
        args: &BooksLayersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Layersummaries, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_layers_list_builder(
            &self.http_client,
            &args.volumeId,
            &args.contentVersion,
            &args.maxResults,
            &args.pageToken,
            &args.source,
        )
        .map_err(ProviderError::Api)?;

        let task = books_layers_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Books layers annotation data get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DictionaryAnnotationdata result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn books_layers_annotation_data_get(
        &self,
        args: &BooksLayersAnnotationDataGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DictionaryAnnotationdata, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_layers_annotation_data_get_builder(
            &self.http_client,
            &args.volumeId,
            &args.layerId,
            &args.annotationDataId,
            &args.allowWebDefinitions,
            &args.contentVersion,
            &args.h,
            &args.locale,
            &args.scale,
            &args.source,
            &args.w,
        )
        .map_err(ProviderError::Api)?;

        let task = books_layers_annotation_data_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Books layers annotation data list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Annotationsdata result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn books_layers_annotation_data_list(
        &self,
        args: &BooksLayersAnnotationDataListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Annotationsdata, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_layers_annotation_data_list_builder(
            &self.http_client,
            &args.volumeId,
            &args.layerId,
            &args.annotationDataId,
            &args.contentVersion,
            &args.h,
            &args.locale,
            &args.maxResults,
            &args.pageToken,
            &args.scale,
            &args.source,
            &args.updatedMax,
            &args.updatedMin,
            &args.w,
        )
        .map_err(ProviderError::Api)?;

        let task = books_layers_annotation_data_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Books layers volume annotations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Volumeannotation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn books_layers_volume_annotations_get(
        &self,
        args: &BooksLayersVolumeAnnotationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Volumeannotation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_layers_volume_annotations_get_builder(
            &self.http_client,
            &args.volumeId,
            &args.layerId,
            &args.annotationId,
            &args.locale,
            &args.source,
        )
        .map_err(ProviderError::Api)?;

        let task = books_layers_volume_annotations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Books layers volume annotations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Volumeannotations result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn books_layers_volume_annotations_list(
        &self,
        args: &BooksLayersVolumeAnnotationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Volumeannotations, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_layers_volume_annotations_list_builder(
            &self.http_client,
            &args.volumeId,
            &args.layerId,
            &args.contentVersion,
            &args.endOffset,
            &args.endPosition,
            &args.locale,
            &args.maxResults,
            &args.pageToken,
            &args.showDeleted,
            &args.source,
            &args.startOffset,
            &args.startPosition,
            &args.updatedMax,
            &args.updatedMin,
            &args.volumeAnnotationsVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = books_layers_volume_annotations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Books myconfig get user settings.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn books_myconfig_get_user_settings(
        &self,
        args: &BooksMyconfigGetUserSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Usersettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_myconfig_get_user_settings_builder(
            &self.http_client,
            &args.country,
        )
        .map_err(ProviderError::Api)?;

        let task = books_myconfig_get_user_settings_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Books mylibrary annotations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Annotations result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn books_mylibrary_annotations_list(
        &self,
        args: &BooksMylibraryAnnotationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Annotations, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_mylibrary_annotations_list_builder(
            &self.http_client,
            &args.contentVersion,
            &args.layerId,
            &args.layerIds,
            &args.maxResults,
            &args.pageToken,
            &args.showDeleted,
            &args.source,
            &args.updatedMax,
            &args.updatedMin,
            &args.volumeId,
        )
        .map_err(ProviderError::Api)?;

        let task = books_mylibrary_annotations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Books mylibrary bookshelves get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Bookshelf result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn books_mylibrary_bookshelves_get(
        &self,
        args: &BooksMylibraryBookshelvesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Bookshelf, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_mylibrary_bookshelves_get_builder(
            &self.http_client,
            &args.shelf,
            &args.source,
        )
        .map_err(ProviderError::Api)?;

        let task = books_mylibrary_bookshelves_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Books mylibrary bookshelves list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Bookshelves result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn books_mylibrary_bookshelves_list(
        &self,
        args: &BooksMylibraryBookshelvesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Bookshelves, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_mylibrary_bookshelves_list_builder(
            &self.http_client,
            &args.source,
        )
        .map_err(ProviderError::Api)?;

        let task = books_mylibrary_bookshelves_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Books mylibrary bookshelves volumes list.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn books_mylibrary_bookshelves_volumes_list(
        &self,
        args: &BooksMylibraryBookshelvesVolumesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Volumes, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_mylibrary_bookshelves_volumes_list_builder(
            &self.http_client,
            &args.shelf,
            &args.country,
            &args.maxResults,
            &args.projection,
            &args.q,
            &args.showPreorders,
            &args.source,
            &args.startIndex,
        )
        .map_err(ProviderError::Api)?;

        let task = books_mylibrary_bookshelves_volumes_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Books mylibrary readingpositions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ReadingPosition result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn books_mylibrary_readingpositions_get(
        &self,
        args: &BooksMylibraryReadingpositionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ReadingPosition, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_mylibrary_readingpositions_get_builder(
            &self.http_client,
            &args.volumeId,
            &args.contentVersion,
            &args.source,
        )
        .map_err(ProviderError::Api)?;

        let task = books_mylibrary_readingpositions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Books notification get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Notification result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn books_notification_get(
        &self,
        args: &BooksNotificationGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Notification, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_notification_get_builder(
            &self.http_client,
            &args.locale,
            &args.notification_id,
            &args.source,
        )
        .map_err(ProviderError::Api)?;

        let task = books_notification_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Books onboarding list categories.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Category result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn books_onboarding_list_categories(
        &self,
        args: &BooksOnboardingListCategoriesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Category, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_onboarding_list_categories_builder(
            &self.http_client,
            &args.locale,
        )
        .map_err(ProviderError::Api)?;

        let task = books_onboarding_list_categories_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Books onboarding list category volumes.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Volume2 result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn books_onboarding_list_category_volumes(
        &self,
        args: &BooksOnboardingListCategoryVolumesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Volume2, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_onboarding_list_category_volumes_builder(
            &self.http_client,
            &args.categoryId,
            &args.locale,
            &args.maxAllowedMaturityRating,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = books_onboarding_list_category_volumes_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Books personalizedstream get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Discoveryclusters result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn books_personalizedstream_get(
        &self,
        args: &BooksPersonalizedstreamGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Discoveryclusters, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_personalizedstream_get_builder(
            &self.http_client,
            &args.locale,
            &args.maxAllowedMaturityRating,
            &args.source,
        )
        .map_err(ProviderError::Api)?;

        let task = books_personalizedstream_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Books promooffer get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Offers result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn books_promooffer_get(
        &self,
        args: &BooksPromoofferGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Offers, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_promooffer_get_builder(
            &self.http_client,
            &args.androidId,
            &args.device,
            &args.manufacturer,
            &args.model,
            &args.product,
            &args.serial,
        )
        .map_err(ProviderError::Api)?;

        let task = books_promooffer_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Books series get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Series result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn books_series_get(
        &self,
        args: &BooksSeriesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Series, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_series_get_builder(
            &self.http_client,
            &args.series_id,
        )
        .map_err(ProviderError::Api)?;

        let task = books_series_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Books series membership get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Seriesmembership result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn books_series_membership_get(
        &self,
        args: &BooksSeriesMembershipGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Seriesmembership, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_series_membership_get_builder(
            &self.http_client,
            &args.page_size,
            &args.page_token,
            &args.series_id,
        )
        .map_err(ProviderError::Api)?;

        let task = books_series_membership_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Books volumes get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Volume result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn books_volumes_get(
        &self,
        args: &BooksVolumesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Volume, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_volumes_get_builder(
            &self.http_client,
            &args.volumeId,
            &args.country,
            &args.includeNonComicsSeries,
            &args.partner,
            &args.projection,
            &args.source,
            &args.user_library_consistent_read,
        )
        .map_err(ProviderError::Api)?;

        let task = books_volumes_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Books volumes list.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn books_volumes_list(
        &self,
        args: &BooksVolumesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Volumes, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_volumes_list_builder(
            &self.http_client,
            &args.download,
            &args.filter,
            &args.langRestrict,
            &args.libraryRestrict,
            &args.maxAllowedMaturityRating,
            &args.maxResults,
            &args.orderBy,
            &args.partner,
            &args.printType,
            &args.projection,
            &args.q,
            &args.showPreorders,
            &args.source,
            &args.startIndex,
        )
        .map_err(ProviderError::Api)?;

        let task = books_volumes_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Books volumes associated list.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn books_volumes_associated_list(
        &self,
        args: &BooksVolumesAssociatedListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Volumes, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_volumes_associated_list_builder(
            &self.http_client,
            &args.volumeId,
            &args.association,
            &args.locale,
            &args.maxAllowedMaturityRating,
            &args.source,
        )
        .map_err(ProviderError::Api)?;

        let task = books_volumes_associated_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Books volumes mybooks list.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn books_volumes_mybooks_list(
        &self,
        args: &BooksVolumesMybooksListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Volumes, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_volumes_mybooks_list_builder(
            &self.http_client,
            &args.acquireMethod,
            &args.country,
            &args.locale,
            &args.maxResults,
            &args.processingState,
            &args.source,
            &args.startIndex,
        )
        .map_err(ProviderError::Api)?;

        let task = books_volumes_mybooks_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Books volumes recommended list.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn books_volumes_recommended_list(
        &self,
        args: &BooksVolumesRecommendedListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Volumes, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_volumes_recommended_list_builder(
            &self.http_client,
            &args.locale,
            &args.maxAllowedMaturityRating,
            &args.source,
        )
        .map_err(ProviderError::Api)?;

        let task = books_volumes_recommended_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Books volumes useruploaded list.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn books_volumes_useruploaded_list(
        &self,
        args: &BooksVolumesUseruploadedListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Volumes, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = books_volumes_useruploaded_list_builder(
            &self.http_client,
            &args.locale,
            &args.maxResults,
            &args.processingState,
            &args.source,
            &args.startIndex,
            &args.volumeId,
        )
        .map_err(ProviderError::Api)?;

        let task = books_volumes_useruploaded_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
