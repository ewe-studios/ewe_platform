//! ChatProvider - State-aware chat API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       chat API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::chat::{
    chat_custom_emojis_create_builder, chat_custom_emojis_create_task,
    chat_custom_emojis_delete_builder, chat_custom_emojis_delete_task,
    chat_custom_emojis_get_builder, chat_custom_emojis_get_task,
    chat_custom_emojis_list_builder, chat_custom_emojis_list_task,
    chat_media_download_builder, chat_media_download_task,
    chat_media_upload_builder, chat_media_upload_task,
    chat_spaces_complete_import_builder, chat_spaces_complete_import_task,
    chat_spaces_create_builder, chat_spaces_create_task,
    chat_spaces_delete_builder, chat_spaces_delete_task,
    chat_spaces_find_direct_message_builder, chat_spaces_find_direct_message_task,
    chat_spaces_find_group_chats_builder, chat_spaces_find_group_chats_task,
    chat_spaces_get_builder, chat_spaces_get_task,
    chat_spaces_list_builder, chat_spaces_list_task,
    chat_spaces_patch_builder, chat_spaces_patch_task,
    chat_spaces_search_builder, chat_spaces_search_task,
    chat_spaces_setup_builder, chat_spaces_setup_task,
    chat_spaces_members_create_builder, chat_spaces_members_create_task,
    chat_spaces_members_delete_builder, chat_spaces_members_delete_task,
    chat_spaces_members_get_builder, chat_spaces_members_get_task,
    chat_spaces_members_list_builder, chat_spaces_members_list_task,
    chat_spaces_members_patch_builder, chat_spaces_members_patch_task,
    chat_spaces_messages_create_builder, chat_spaces_messages_create_task,
    chat_spaces_messages_delete_builder, chat_spaces_messages_delete_task,
    chat_spaces_messages_get_builder, chat_spaces_messages_get_task,
    chat_spaces_messages_list_builder, chat_spaces_messages_list_task,
    chat_spaces_messages_patch_builder, chat_spaces_messages_patch_task,
    chat_spaces_messages_update_builder, chat_spaces_messages_update_task,
    chat_spaces_messages_attachments_get_builder, chat_spaces_messages_attachments_get_task,
    chat_spaces_messages_reactions_create_builder, chat_spaces_messages_reactions_create_task,
    chat_spaces_messages_reactions_delete_builder, chat_spaces_messages_reactions_delete_task,
    chat_spaces_messages_reactions_list_builder, chat_spaces_messages_reactions_list_task,
    chat_spaces_space_events_get_builder, chat_spaces_space_events_get_task,
    chat_spaces_space_events_list_builder, chat_spaces_space_events_list_task,
    chat_users_sections_create_builder, chat_users_sections_create_task,
    chat_users_sections_delete_builder, chat_users_sections_delete_task,
    chat_users_sections_list_builder, chat_users_sections_list_task,
    chat_users_sections_patch_builder, chat_users_sections_patch_task,
    chat_users_sections_position_builder, chat_users_sections_position_task,
    chat_users_sections_items_list_builder, chat_users_sections_items_list_task,
    chat_users_sections_items_move_builder, chat_users_sections_items_move_task,
    chat_users_spaces_get_space_read_state_builder, chat_users_spaces_get_space_read_state_task,
    chat_users_spaces_update_space_read_state_builder, chat_users_spaces_update_space_read_state_task,
    chat_users_spaces_space_notification_setting_get_builder, chat_users_spaces_space_notification_setting_get_task,
    chat_users_spaces_space_notification_setting_patch_builder, chat_users_spaces_space_notification_setting_patch_task,
    chat_users_spaces_threads_get_thread_read_state_builder, chat_users_spaces_threads_get_thread_read_state_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::chat::Attachment;
use crate::providers::gcp::clients::chat::CompleteImportSpaceResponse;
use crate::providers::gcp::clients::chat::CustomEmoji;
use crate::providers::gcp::clients::chat::Empty;
use crate::providers::gcp::clients::chat::FindGroupChatsResponse;
use crate::providers::gcp::clients::chat::GoogleChatV1Section;
use crate::providers::gcp::clients::chat::ListCustomEmojisResponse;
use crate::providers::gcp::clients::chat::ListMembershipsResponse;
use crate::providers::gcp::clients::chat::ListMessagesResponse;
use crate::providers::gcp::clients::chat::ListReactionsResponse;
use crate::providers::gcp::clients::chat::ListSectionItemsResponse;
use crate::providers::gcp::clients::chat::ListSectionsResponse;
use crate::providers::gcp::clients::chat::ListSpaceEventsResponse;
use crate::providers::gcp::clients::chat::ListSpacesResponse;
use crate::providers::gcp::clients::chat::Media;
use crate::providers::gcp::clients::chat::Membership;
use crate::providers::gcp::clients::chat::Message;
use crate::providers::gcp::clients::chat::MoveSectionItemResponse;
use crate::providers::gcp::clients::chat::PositionSectionResponse;
use crate::providers::gcp::clients::chat::Reaction;
use crate::providers::gcp::clients::chat::SearchSpacesResponse;
use crate::providers::gcp::clients::chat::Space;
use crate::providers::gcp::clients::chat::SpaceEvent;
use crate::providers::gcp::clients::chat::SpaceNotificationSetting;
use crate::providers::gcp::clients::chat::SpaceReadState;
use crate::providers::gcp::clients::chat::ThreadReadState;
use crate::providers::gcp::clients::chat::UploadAttachmentResponse;
use crate::providers::gcp::clients::chat::ChatCustomEmojisCreateArgs;
use crate::providers::gcp::clients::chat::ChatCustomEmojisDeleteArgs;
use crate::providers::gcp::clients::chat::ChatCustomEmojisGetArgs;
use crate::providers::gcp::clients::chat::ChatCustomEmojisListArgs;
use crate::providers::gcp::clients::chat::ChatMediaDownloadArgs;
use crate::providers::gcp::clients::chat::ChatMediaUploadArgs;
use crate::providers::gcp::clients::chat::ChatSpacesCompleteImportArgs;
use crate::providers::gcp::clients::chat::ChatSpacesCreateArgs;
use crate::providers::gcp::clients::chat::ChatSpacesDeleteArgs;
use crate::providers::gcp::clients::chat::ChatSpacesFindDirectMessageArgs;
use crate::providers::gcp::clients::chat::ChatSpacesFindGroupChatsArgs;
use crate::providers::gcp::clients::chat::ChatSpacesGetArgs;
use crate::providers::gcp::clients::chat::ChatSpacesListArgs;
use crate::providers::gcp::clients::chat::ChatSpacesMembersCreateArgs;
use crate::providers::gcp::clients::chat::ChatSpacesMembersDeleteArgs;
use crate::providers::gcp::clients::chat::ChatSpacesMembersGetArgs;
use crate::providers::gcp::clients::chat::ChatSpacesMembersListArgs;
use crate::providers::gcp::clients::chat::ChatSpacesMembersPatchArgs;
use crate::providers::gcp::clients::chat::ChatSpacesMessagesAttachmentsGetArgs;
use crate::providers::gcp::clients::chat::ChatSpacesMessagesCreateArgs;
use crate::providers::gcp::clients::chat::ChatSpacesMessagesDeleteArgs;
use crate::providers::gcp::clients::chat::ChatSpacesMessagesGetArgs;
use crate::providers::gcp::clients::chat::ChatSpacesMessagesListArgs;
use crate::providers::gcp::clients::chat::ChatSpacesMessagesPatchArgs;
use crate::providers::gcp::clients::chat::ChatSpacesMessagesReactionsCreateArgs;
use crate::providers::gcp::clients::chat::ChatSpacesMessagesReactionsDeleteArgs;
use crate::providers::gcp::clients::chat::ChatSpacesMessagesReactionsListArgs;
use crate::providers::gcp::clients::chat::ChatSpacesMessagesUpdateArgs;
use crate::providers::gcp::clients::chat::ChatSpacesPatchArgs;
use crate::providers::gcp::clients::chat::ChatSpacesSearchArgs;
use crate::providers::gcp::clients::chat::ChatSpacesSetupArgs;
use crate::providers::gcp::clients::chat::ChatSpacesSpaceEventsGetArgs;
use crate::providers::gcp::clients::chat::ChatSpacesSpaceEventsListArgs;
use crate::providers::gcp::clients::chat::ChatUsersSectionsCreateArgs;
use crate::providers::gcp::clients::chat::ChatUsersSectionsDeleteArgs;
use crate::providers::gcp::clients::chat::ChatUsersSectionsItemsListArgs;
use crate::providers::gcp::clients::chat::ChatUsersSectionsItemsMoveArgs;
use crate::providers::gcp::clients::chat::ChatUsersSectionsListArgs;
use crate::providers::gcp::clients::chat::ChatUsersSectionsPatchArgs;
use crate::providers::gcp::clients::chat::ChatUsersSectionsPositionArgs;
use crate::providers::gcp::clients::chat::ChatUsersSpacesGetSpaceReadStateArgs;
use crate::providers::gcp::clients::chat::ChatUsersSpacesSpaceNotificationSettingGetArgs;
use crate::providers::gcp::clients::chat::ChatUsersSpacesSpaceNotificationSettingPatchArgs;
use crate::providers::gcp::clients::chat::ChatUsersSpacesThreadsGetThreadReadStateArgs;
use crate::providers::gcp::clients::chat::ChatUsersSpacesUpdateSpaceReadStateArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// ChatProvider with automatic state tracking.
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
/// let provider = ChatProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct ChatProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> ChatProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new ChatProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Chat custom emojis create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CustomEmoji result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chat_custom_emojis_create(
        &self,
        args: &ChatCustomEmojisCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CustomEmoji, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chat_custom_emojis_create_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = chat_custom_emojis_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chat custom emojis delete.
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
    pub fn chat_custom_emojis_delete(
        &self,
        args: &ChatCustomEmojisDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chat_custom_emojis_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = chat_custom_emojis_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chat custom emojis get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CustomEmoji result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chat_custom_emojis_get(
        &self,
        args: &ChatCustomEmojisGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CustomEmoji, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chat_custom_emojis_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = chat_custom_emojis_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chat custom emojis list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCustomEmojisResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chat_custom_emojis_list(
        &self,
        args: &ChatCustomEmojisListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCustomEmojisResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chat_custom_emojis_list_builder(
            &self.http_client,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = chat_custom_emojis_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chat media download.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Media result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chat_media_download(
        &self,
        args: &ChatMediaDownloadArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Media, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chat_media_download_builder(
            &self.http_client,
            &args.resourceName,
        )
        .map_err(ProviderError::Api)?;

        let task = chat_media_download_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chat media upload.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UploadAttachmentResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chat_media_upload(
        &self,
        args: &ChatMediaUploadArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UploadAttachmentResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chat_media_upload_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = chat_media_upload_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chat spaces complete import.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CompleteImportSpaceResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chat_spaces_complete_import(
        &self,
        args: &ChatSpacesCompleteImportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CompleteImportSpaceResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chat_spaces_complete_import_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = chat_spaces_complete_import_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chat spaces create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Space result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chat_spaces_create(
        &self,
        args: &ChatSpacesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Space, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chat_spaces_create_builder(
            &self.http_client,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = chat_spaces_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chat spaces delete.
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
    pub fn chat_spaces_delete(
        &self,
        args: &ChatSpacesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chat_spaces_delete_builder(
            &self.http_client,
            &args.name,
            &args.useAdminAccess,
        )
        .map_err(ProviderError::Api)?;

        let task = chat_spaces_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chat spaces find direct message.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Space result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chat_spaces_find_direct_message(
        &self,
        args: &ChatSpacesFindDirectMessageArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Space, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chat_spaces_find_direct_message_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = chat_spaces_find_direct_message_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chat spaces find group chats.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FindGroupChatsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chat_spaces_find_group_chats(
        &self,
        args: &ChatSpacesFindGroupChatsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FindGroupChatsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chat_spaces_find_group_chats_builder(
            &self.http_client,
            &args.pageSize,
            &args.pageToken,
            &args.spaceView,
            &args.users,
        )
        .map_err(ProviderError::Api)?;

        let task = chat_spaces_find_group_chats_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chat spaces get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Space result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chat_spaces_get(
        &self,
        args: &ChatSpacesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Space, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chat_spaces_get_builder(
            &self.http_client,
            &args.name,
            &args.useAdminAccess,
        )
        .map_err(ProviderError::Api)?;

        let task = chat_spaces_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chat spaces list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListSpacesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chat_spaces_list(
        &self,
        args: &ChatSpacesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSpacesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chat_spaces_list_builder(
            &self.http_client,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = chat_spaces_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chat spaces patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Space result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chat_spaces_patch(
        &self,
        args: &ChatSpacesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Space, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chat_spaces_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
            &args.useAdminAccess,
        )
        .map_err(ProviderError::Api)?;

        let task = chat_spaces_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chat spaces search.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchSpacesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chat_spaces_search(
        &self,
        args: &ChatSpacesSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchSpacesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chat_spaces_search_builder(
            &self.http_client,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.query,
            &args.useAdminAccess,
        )
        .map_err(ProviderError::Api)?;

        let task = chat_spaces_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chat spaces setup.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Space result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chat_spaces_setup(
        &self,
        args: &ChatSpacesSetupArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Space, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chat_spaces_setup_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = chat_spaces_setup_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chat spaces members create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Membership result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chat_spaces_members_create(
        &self,
        args: &ChatSpacesMembersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Membership, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chat_spaces_members_create_builder(
            &self.http_client,
            &args.parent,
            &args.useAdminAccess,
        )
        .map_err(ProviderError::Api)?;

        let task = chat_spaces_members_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chat spaces members delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Membership result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chat_spaces_members_delete(
        &self,
        args: &ChatSpacesMembersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Membership, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chat_spaces_members_delete_builder(
            &self.http_client,
            &args.name,
            &args.useAdminAccess,
        )
        .map_err(ProviderError::Api)?;

        let task = chat_spaces_members_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chat spaces members get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Membership result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chat_spaces_members_get(
        &self,
        args: &ChatSpacesMembersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Membership, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chat_spaces_members_get_builder(
            &self.http_client,
            &args.name,
            &args.useAdminAccess,
        )
        .map_err(ProviderError::Api)?;

        let task = chat_spaces_members_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chat spaces members list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListMembershipsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chat_spaces_members_list(
        &self,
        args: &ChatSpacesMembersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListMembershipsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chat_spaces_members_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.showGroups,
            &args.showInvited,
            &args.useAdminAccess,
        )
        .map_err(ProviderError::Api)?;

        let task = chat_spaces_members_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chat spaces members patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Membership result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chat_spaces_members_patch(
        &self,
        args: &ChatSpacesMembersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Membership, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chat_spaces_members_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
            &args.useAdminAccess,
        )
        .map_err(ProviderError::Api)?;

        let task = chat_spaces_members_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chat spaces messages create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Message result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chat_spaces_messages_create(
        &self,
        args: &ChatSpacesMessagesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Message, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chat_spaces_messages_create_builder(
            &self.http_client,
            &args.parent,
            &args.messageId,
            &args.messageReplyOption,
            &args.requestId,
            &args.threadKey,
        )
        .map_err(ProviderError::Api)?;

        let task = chat_spaces_messages_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chat spaces messages delete.
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
    pub fn chat_spaces_messages_delete(
        &self,
        args: &ChatSpacesMessagesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chat_spaces_messages_delete_builder(
            &self.http_client,
            &args.name,
            &args.force,
        )
        .map_err(ProviderError::Api)?;

        let task = chat_spaces_messages_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chat spaces messages get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Message result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chat_spaces_messages_get(
        &self,
        args: &ChatSpacesMessagesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Message, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chat_spaces_messages_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = chat_spaces_messages_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chat spaces messages list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListMessagesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chat_spaces_messages_list(
        &self,
        args: &ChatSpacesMessagesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListMessagesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chat_spaces_messages_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.showDeleted,
        )
        .map_err(ProviderError::Api)?;

        let task = chat_spaces_messages_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chat spaces messages patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Message result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chat_spaces_messages_patch(
        &self,
        args: &ChatSpacesMessagesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Message, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chat_spaces_messages_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = chat_spaces_messages_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chat spaces messages update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Message result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chat_spaces_messages_update(
        &self,
        args: &ChatSpacesMessagesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Message, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chat_spaces_messages_update_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = chat_spaces_messages_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chat spaces messages attachments get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Attachment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chat_spaces_messages_attachments_get(
        &self,
        args: &ChatSpacesMessagesAttachmentsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Attachment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chat_spaces_messages_attachments_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = chat_spaces_messages_attachments_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chat spaces messages reactions create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Reaction result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chat_spaces_messages_reactions_create(
        &self,
        args: &ChatSpacesMessagesReactionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Reaction, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chat_spaces_messages_reactions_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = chat_spaces_messages_reactions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chat spaces messages reactions delete.
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
    pub fn chat_spaces_messages_reactions_delete(
        &self,
        args: &ChatSpacesMessagesReactionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chat_spaces_messages_reactions_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = chat_spaces_messages_reactions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chat spaces messages reactions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListReactionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chat_spaces_messages_reactions_list(
        &self,
        args: &ChatSpacesMessagesReactionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListReactionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chat_spaces_messages_reactions_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = chat_spaces_messages_reactions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chat spaces space events get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SpaceEvent result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chat_spaces_space_events_get(
        &self,
        args: &ChatSpacesSpaceEventsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SpaceEvent, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chat_spaces_space_events_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = chat_spaces_space_events_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chat spaces space events list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListSpaceEventsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chat_spaces_space_events_list(
        &self,
        args: &ChatSpacesSpaceEventsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSpaceEventsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chat_spaces_space_events_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = chat_spaces_space_events_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chat users sections create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChatV1Section result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chat_users_sections_create(
        &self,
        args: &ChatUsersSectionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChatV1Section, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chat_users_sections_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = chat_users_sections_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chat users sections delete.
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
    pub fn chat_users_sections_delete(
        &self,
        args: &ChatUsersSectionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chat_users_sections_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = chat_users_sections_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chat users sections list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListSectionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chat_users_sections_list(
        &self,
        args: &ChatUsersSectionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSectionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chat_users_sections_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = chat_users_sections_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chat users sections patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChatV1Section result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chat_users_sections_patch(
        &self,
        args: &ChatUsersSectionsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChatV1Section, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chat_users_sections_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = chat_users_sections_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chat users sections position.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PositionSectionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chat_users_sections_position(
        &self,
        args: &ChatUsersSectionsPositionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PositionSectionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chat_users_sections_position_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = chat_users_sections_position_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chat users sections items list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListSectionItemsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chat_users_sections_items_list(
        &self,
        args: &ChatUsersSectionsItemsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSectionItemsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chat_users_sections_items_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = chat_users_sections_items_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chat users sections items move.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the MoveSectionItemResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chat_users_sections_items_move(
        &self,
        args: &ChatUsersSectionsItemsMoveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<MoveSectionItemResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chat_users_sections_items_move_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = chat_users_sections_items_move_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chat users spaces get space read state.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SpaceReadState result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chat_users_spaces_get_space_read_state(
        &self,
        args: &ChatUsersSpacesGetSpaceReadStateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SpaceReadState, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chat_users_spaces_get_space_read_state_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = chat_users_spaces_get_space_read_state_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chat users spaces update space read state.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SpaceReadState result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chat_users_spaces_update_space_read_state(
        &self,
        args: &ChatUsersSpacesUpdateSpaceReadStateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SpaceReadState, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chat_users_spaces_update_space_read_state_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = chat_users_spaces_update_space_read_state_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chat users spaces space notification setting get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SpaceNotificationSetting result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chat_users_spaces_space_notification_setting_get(
        &self,
        args: &ChatUsersSpacesSpaceNotificationSettingGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SpaceNotificationSetting, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chat_users_spaces_space_notification_setting_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = chat_users_spaces_space_notification_setting_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chat users spaces space notification setting patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SpaceNotificationSetting result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chat_users_spaces_space_notification_setting_patch(
        &self,
        args: &ChatUsersSpacesSpaceNotificationSettingPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SpaceNotificationSetting, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chat_users_spaces_space_notification_setting_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = chat_users_spaces_space_notification_setting_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chat users spaces threads get thread read state.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ThreadReadState result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn chat_users_spaces_threads_get_thread_read_state(
        &self,
        args: &ChatUsersSpacesThreadsGetThreadReadStateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ThreadReadState, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chat_users_spaces_threads_get_thread_read_state_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = chat_users_spaces_threads_get_thread_read_state_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
