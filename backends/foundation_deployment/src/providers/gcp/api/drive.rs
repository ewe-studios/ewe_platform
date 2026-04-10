//! DriveProvider - State-aware drive API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       drive API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::drive::{
    drive_accessproposals_resolve_builder, drive_accessproposals_resolve_task,
    drive_changes_watch_builder, drive_changes_watch_task,
    drive_channels_stop_builder, drive_channels_stop_task,
    drive_comments_create_builder, drive_comments_create_task,
    drive_comments_delete_builder, drive_comments_delete_task,
    drive_comments_update_builder, drive_comments_update_task,
    drive_drives_create_builder, drive_drives_create_task,
    drive_drives_delete_builder, drive_drives_delete_task,
    drive_drives_hide_builder, drive_drives_hide_task,
    drive_drives_unhide_builder, drive_drives_unhide_task,
    drive_drives_update_builder, drive_drives_update_task,
    drive_files_copy_builder, drive_files_copy_task,
    drive_files_create_builder, drive_files_create_task,
    drive_files_delete_builder, drive_files_delete_task,
    drive_files_download_builder, drive_files_download_task,
    drive_files_empty_trash_builder, drive_files_empty_trash_task,
    drive_files_modify_labels_builder, drive_files_modify_labels_task,
    drive_files_update_builder, drive_files_update_task,
    drive_files_watch_builder, drive_files_watch_task,
    drive_permissions_create_builder, drive_permissions_create_task,
    drive_permissions_delete_builder, drive_permissions_delete_task,
    drive_permissions_update_builder, drive_permissions_update_task,
    drive_replies_create_builder, drive_replies_create_task,
    drive_replies_delete_builder, drive_replies_delete_task,
    drive_replies_update_builder, drive_replies_update_task,
    drive_revisions_delete_builder, drive_revisions_delete_task,
    drive_revisions_update_builder, drive_revisions_update_task,
    drive_teamdrives_create_builder, drive_teamdrives_create_task,
    drive_teamdrives_delete_builder, drive_teamdrives_delete_task,
    drive_teamdrives_update_builder, drive_teamdrives_update_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::drive::Channel;
use crate::providers::gcp::clients::drive::Comment;
use crate::providers::gcp::clients::drive::Drive;
use crate::providers::gcp::clients::drive::File;
use crate::providers::gcp::clients::drive::ModifyLabelsResponse;
use crate::providers::gcp::clients::drive::Operation;
use crate::providers::gcp::clients::drive::Permission;
use crate::providers::gcp::clients::drive::Reply;
use crate::providers::gcp::clients::drive::Revision;
use crate::providers::gcp::clients::drive::TeamDrive;
use crate::providers::gcp::clients::drive::DriveAccessproposalsResolveArgs;
use crate::providers::gcp::clients::drive::DriveChangesWatchArgs;
use crate::providers::gcp::clients::drive::DriveChannelsStopArgs;
use crate::providers::gcp::clients::drive::DriveCommentsCreateArgs;
use crate::providers::gcp::clients::drive::DriveCommentsDeleteArgs;
use crate::providers::gcp::clients::drive::DriveCommentsUpdateArgs;
use crate::providers::gcp::clients::drive::DriveDrivesCreateArgs;
use crate::providers::gcp::clients::drive::DriveDrivesDeleteArgs;
use crate::providers::gcp::clients::drive::DriveDrivesHideArgs;
use crate::providers::gcp::clients::drive::DriveDrivesUnhideArgs;
use crate::providers::gcp::clients::drive::DriveDrivesUpdateArgs;
use crate::providers::gcp::clients::drive::DriveFilesCopyArgs;
use crate::providers::gcp::clients::drive::DriveFilesCreateArgs;
use crate::providers::gcp::clients::drive::DriveFilesDeleteArgs;
use crate::providers::gcp::clients::drive::DriveFilesDownloadArgs;
use crate::providers::gcp::clients::drive::DriveFilesEmptyTrashArgs;
use crate::providers::gcp::clients::drive::DriveFilesModifyLabelsArgs;
use crate::providers::gcp::clients::drive::DriveFilesUpdateArgs;
use crate::providers::gcp::clients::drive::DriveFilesWatchArgs;
use crate::providers::gcp::clients::drive::DrivePermissionsCreateArgs;
use crate::providers::gcp::clients::drive::DrivePermissionsDeleteArgs;
use crate::providers::gcp::clients::drive::DrivePermissionsUpdateArgs;
use crate::providers::gcp::clients::drive::DriveRepliesCreateArgs;
use crate::providers::gcp::clients::drive::DriveRepliesDeleteArgs;
use crate::providers::gcp::clients::drive::DriveRepliesUpdateArgs;
use crate::providers::gcp::clients::drive::DriveRevisionsDeleteArgs;
use crate::providers::gcp::clients::drive::DriveRevisionsUpdateArgs;
use crate::providers::gcp::clients::drive::DriveTeamdrivesCreateArgs;
use crate::providers::gcp::clients::drive::DriveTeamdrivesDeleteArgs;
use crate::providers::gcp::clients::drive::DriveTeamdrivesUpdateArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// DriveProvider with automatic state tracking.
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
/// let provider = DriveProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct DriveProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> DriveProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new DriveProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Drive accessproposals resolve.
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
    pub fn drive_accessproposals_resolve(
        &self,
        args: &DriveAccessproposalsResolveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drive_accessproposals_resolve_builder(
            &self.http_client,
            &args.fileId,
            &args.proposalId,
        )
        .map_err(ProviderError::Api)?;

        let task = drive_accessproposals_resolve_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drive changes watch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Channel result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn drive_changes_watch(
        &self,
        args: &DriveChangesWatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Channel, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drive_changes_watch_builder(
            &self.http_client,
            &args.pageToken,
            &args.driveId,
            &args.includeCorpusRemovals,
            &args.includeItemsFromAllDrives,
            &args.includeLabels,
            &args.includePermissionsForView,
            &args.includeRemoved,
            &args.includeTeamDriveItems,
            &args.pageSize,
            &args.pageToken,
            &args.restrictToMyDrive,
            &args.spaces,
            &args.supportsAllDrives,
            &args.supportsTeamDrives,
            &args.teamDriveId,
        )
        .map_err(ProviderError::Api)?;

        let task = drive_changes_watch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drive channels stop.
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
    pub fn drive_channels_stop(
        &self,
        args: &DriveChannelsStopArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drive_channels_stop_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = drive_channels_stop_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drive comments create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Comment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn drive_comments_create(
        &self,
        args: &DriveCommentsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Comment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drive_comments_create_builder(
            &self.http_client,
            &args.fileId,
        )
        .map_err(ProviderError::Api)?;

        let task = drive_comments_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drive comments delete.
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
    pub fn drive_comments_delete(
        &self,
        args: &DriveCommentsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drive_comments_delete_builder(
            &self.http_client,
            &args.fileId,
            &args.commentId,
        )
        .map_err(ProviderError::Api)?;

        let task = drive_comments_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drive comments update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Comment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn drive_comments_update(
        &self,
        args: &DriveCommentsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Comment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drive_comments_update_builder(
            &self.http_client,
            &args.fileId,
            &args.commentId,
        )
        .map_err(ProviderError::Api)?;

        let task = drive_comments_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drive drives create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Drive result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn drive_drives_create(
        &self,
        args: &DriveDrivesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Drive, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drive_drives_create_builder(
            &self.http_client,
            &args.requestId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = drive_drives_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drive drives delete.
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
    pub fn drive_drives_delete(
        &self,
        args: &DriveDrivesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drive_drives_delete_builder(
            &self.http_client,
            &args.driveId,
            &args.allowItemDeletion,
            &args.useDomainAdminAccess,
        )
        .map_err(ProviderError::Api)?;

        let task = drive_drives_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drive drives hide.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Drive result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn drive_drives_hide(
        &self,
        args: &DriveDrivesHideArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Drive, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drive_drives_hide_builder(
            &self.http_client,
            &args.driveId,
        )
        .map_err(ProviderError::Api)?;

        let task = drive_drives_hide_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drive drives unhide.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Drive result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn drive_drives_unhide(
        &self,
        args: &DriveDrivesUnhideArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Drive, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drive_drives_unhide_builder(
            &self.http_client,
            &args.driveId,
        )
        .map_err(ProviderError::Api)?;

        let task = drive_drives_unhide_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drive drives update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Drive result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn drive_drives_update(
        &self,
        args: &DriveDrivesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Drive, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drive_drives_update_builder(
            &self.http_client,
            &args.driveId,
            &args.useDomainAdminAccess,
        )
        .map_err(ProviderError::Api)?;

        let task = drive_drives_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drive files copy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the File result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn drive_files_copy(
        &self,
        args: &DriveFilesCopyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<File, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drive_files_copy_builder(
            &self.http_client,
            &args.fileId,
            &args.enforceSingleParent,
            &args.ignoreDefaultVisibility,
            &args.includeLabels,
            &args.includePermissionsForView,
            &args.keepRevisionForever,
            &args.ocrLanguage,
            &args.supportsAllDrives,
            &args.supportsTeamDrives,
        )
        .map_err(ProviderError::Api)?;

        let task = drive_files_copy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drive files create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the File result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn drive_files_create(
        &self,
        args: &DriveFilesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<File, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drive_files_create_builder(
            &self.http_client,
            &args.enforceSingleParent,
            &args.ignoreDefaultVisibility,
            &args.includeLabels,
            &args.includePermissionsForView,
            &args.keepRevisionForever,
            &args.ocrLanguage,
            &args.supportsAllDrives,
            &args.supportsTeamDrives,
            &args.useContentAsIndexableText,
        )
        .map_err(ProviderError::Api)?;

        let task = drive_files_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drive files delete.
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
    pub fn drive_files_delete(
        &self,
        args: &DriveFilesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drive_files_delete_builder(
            &self.http_client,
            &args.fileId,
            &args.enforceSingleParent,
            &args.supportsAllDrives,
            &args.supportsTeamDrives,
        )
        .map_err(ProviderError::Api)?;

        let task = drive_files_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drive files download.
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
    pub fn drive_files_download(
        &self,
        args: &DriveFilesDownloadArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drive_files_download_builder(
            &self.http_client,
            &args.fileId,
            &args.mimeType,
            &args.revisionId,
        )
        .map_err(ProviderError::Api)?;

        let task = drive_files_download_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drive files empty trash.
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
    pub fn drive_files_empty_trash(
        &self,
        args: &DriveFilesEmptyTrashArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drive_files_empty_trash_builder(
            &self.http_client,
            &args.driveId,
            &args.enforceSingleParent,
        )
        .map_err(ProviderError::Api)?;

        let task = drive_files_empty_trash_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drive files modify labels.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ModifyLabelsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn drive_files_modify_labels(
        &self,
        args: &DriveFilesModifyLabelsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ModifyLabelsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drive_files_modify_labels_builder(
            &self.http_client,
            &args.fileId,
        )
        .map_err(ProviderError::Api)?;

        let task = drive_files_modify_labels_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drive files update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the File result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn drive_files_update(
        &self,
        args: &DriveFilesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<File, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drive_files_update_builder(
            &self.http_client,
            &args.fileId,
            &args.addParents,
            &args.enforceSingleParent,
            &args.includeLabels,
            &args.includePermissionsForView,
            &args.keepRevisionForever,
            &args.ocrLanguage,
            &args.removeParents,
            &args.supportsAllDrives,
            &args.supportsTeamDrives,
            &args.useContentAsIndexableText,
        )
        .map_err(ProviderError::Api)?;

        let task = drive_files_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drive files watch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Channel result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn drive_files_watch(
        &self,
        args: &DriveFilesWatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Channel, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drive_files_watch_builder(
            &self.http_client,
            &args.fileId,
            &args.acknowledgeAbuse,
            &args.includeLabels,
            &args.includePermissionsForView,
            &args.supportsAllDrives,
            &args.supportsTeamDrives,
        )
        .map_err(ProviderError::Api)?;

        let task = drive_files_watch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drive permissions create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Permission result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn drive_permissions_create(
        &self,
        args: &DrivePermissionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Permission, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drive_permissions_create_builder(
            &self.http_client,
            &args.fileId,
            &args.emailMessage,
            &args.enforceExpansiveAccess,
            &args.enforceSingleParent,
            &args.moveToNewOwnersRoot,
            &args.sendNotificationEmail,
            &args.supportsAllDrives,
            &args.supportsTeamDrives,
            &args.transferOwnership,
            &args.useDomainAdminAccess,
        )
        .map_err(ProviderError::Api)?;

        let task = drive_permissions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drive permissions delete.
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
    pub fn drive_permissions_delete(
        &self,
        args: &DrivePermissionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drive_permissions_delete_builder(
            &self.http_client,
            &args.fileId,
            &args.permissionId,
            &args.enforceExpansiveAccess,
            &args.supportsAllDrives,
            &args.supportsTeamDrives,
            &args.useDomainAdminAccess,
        )
        .map_err(ProviderError::Api)?;

        let task = drive_permissions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drive permissions update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Permission result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn drive_permissions_update(
        &self,
        args: &DrivePermissionsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Permission, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drive_permissions_update_builder(
            &self.http_client,
            &args.fileId,
            &args.permissionId,
            &args.enforceExpansiveAccess,
            &args.removeExpiration,
            &args.supportsAllDrives,
            &args.supportsTeamDrives,
            &args.transferOwnership,
            &args.useDomainAdminAccess,
        )
        .map_err(ProviderError::Api)?;

        let task = drive_permissions_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drive replies create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Reply result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn drive_replies_create(
        &self,
        args: &DriveRepliesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Reply, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drive_replies_create_builder(
            &self.http_client,
            &args.fileId,
            &args.commentId,
        )
        .map_err(ProviderError::Api)?;

        let task = drive_replies_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drive replies delete.
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
    pub fn drive_replies_delete(
        &self,
        args: &DriveRepliesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drive_replies_delete_builder(
            &self.http_client,
            &args.fileId,
            &args.commentId,
            &args.replyId,
        )
        .map_err(ProviderError::Api)?;

        let task = drive_replies_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drive replies update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Reply result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn drive_replies_update(
        &self,
        args: &DriveRepliesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Reply, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drive_replies_update_builder(
            &self.http_client,
            &args.fileId,
            &args.commentId,
            &args.replyId,
        )
        .map_err(ProviderError::Api)?;

        let task = drive_replies_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drive revisions delete.
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
    pub fn drive_revisions_delete(
        &self,
        args: &DriveRevisionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drive_revisions_delete_builder(
            &self.http_client,
            &args.fileId,
            &args.revisionId,
        )
        .map_err(ProviderError::Api)?;

        let task = drive_revisions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drive revisions update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Revision result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn drive_revisions_update(
        &self,
        args: &DriveRevisionsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Revision, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drive_revisions_update_builder(
            &self.http_client,
            &args.fileId,
            &args.revisionId,
        )
        .map_err(ProviderError::Api)?;

        let task = drive_revisions_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drive teamdrives create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TeamDrive result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn drive_teamdrives_create(
        &self,
        args: &DriveTeamdrivesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TeamDrive, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drive_teamdrives_create_builder(
            &self.http_client,
            &args.requestId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = drive_teamdrives_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drive teamdrives delete.
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
    pub fn drive_teamdrives_delete(
        &self,
        args: &DriveTeamdrivesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drive_teamdrives_delete_builder(
            &self.http_client,
            &args.teamDriveId,
        )
        .map_err(ProviderError::Api)?;

        let task = drive_teamdrives_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drive teamdrives update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TeamDrive result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn drive_teamdrives_update(
        &self,
        args: &DriveTeamdrivesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TeamDrive, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drive_teamdrives_update_builder(
            &self.http_client,
            &args.teamDriveId,
            &args.useDomainAdminAccess,
        )
        .map_err(ProviderError::Api)?;

        let task = drive_teamdrives_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
