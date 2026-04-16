//! YoutubeProvider - State-aware youtube API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       youtube API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::youtube::{
    youtube_abuse_reports_insert_builder, youtube_abuse_reports_insert_task,
    youtube_activities_list_builder, youtube_activities_list_task,
    youtube_captions_delete_builder, youtube_captions_delete_task,
    youtube_captions_download_builder, youtube_captions_download_task,
    youtube_captions_insert_builder, youtube_captions_insert_task,
    youtube_captions_list_builder, youtube_captions_list_task,
    youtube_captions_update_builder, youtube_captions_update_task,
    youtube_channel_banners_insert_builder, youtube_channel_banners_insert_task,
    youtube_channel_sections_delete_builder, youtube_channel_sections_delete_task,
    youtube_channel_sections_insert_builder, youtube_channel_sections_insert_task,
    youtube_channel_sections_list_builder, youtube_channel_sections_list_task,
    youtube_channel_sections_update_builder, youtube_channel_sections_update_task,
    youtube_channels_list_builder, youtube_channels_list_task,
    youtube_channels_update_builder, youtube_channels_update_task,
    youtube_comment_threads_insert_builder, youtube_comment_threads_insert_task,
    youtube_comment_threads_list_builder, youtube_comment_threads_list_task,
    youtube_comments_delete_builder, youtube_comments_delete_task,
    youtube_comments_insert_builder, youtube_comments_insert_task,
    youtube_comments_list_builder, youtube_comments_list_task,
    youtube_comments_mark_as_spam_builder, youtube_comments_mark_as_spam_task,
    youtube_comments_set_moderation_status_builder, youtube_comments_set_moderation_status_task,
    youtube_comments_update_builder, youtube_comments_update_task,
    youtube_i18n_languages_list_builder, youtube_i18n_languages_list_task,
    youtube_i18n_regions_list_builder, youtube_i18n_regions_list_task,
    youtube_live_broadcasts_bind_builder, youtube_live_broadcasts_bind_task,
    youtube_live_broadcasts_delete_builder, youtube_live_broadcasts_delete_task,
    youtube_live_broadcasts_insert_builder, youtube_live_broadcasts_insert_task,
    youtube_live_broadcasts_insert_cuepoint_builder, youtube_live_broadcasts_insert_cuepoint_task,
    youtube_live_broadcasts_list_builder, youtube_live_broadcasts_list_task,
    youtube_live_broadcasts_transition_builder, youtube_live_broadcasts_transition_task,
    youtube_live_broadcasts_update_builder, youtube_live_broadcasts_update_task,
    youtube_live_chat_bans_delete_builder, youtube_live_chat_bans_delete_task,
    youtube_live_chat_bans_insert_builder, youtube_live_chat_bans_insert_task,
    youtube_live_chat_messages_delete_builder, youtube_live_chat_messages_delete_task,
    youtube_live_chat_messages_insert_builder, youtube_live_chat_messages_insert_task,
    youtube_live_chat_messages_list_builder, youtube_live_chat_messages_list_task,
    youtube_live_chat_messages_transition_builder, youtube_live_chat_messages_transition_task,
    youtube_live_chat_moderators_delete_builder, youtube_live_chat_moderators_delete_task,
    youtube_live_chat_moderators_insert_builder, youtube_live_chat_moderators_insert_task,
    youtube_live_chat_moderators_list_builder, youtube_live_chat_moderators_list_task,
    youtube_live_streams_delete_builder, youtube_live_streams_delete_task,
    youtube_live_streams_insert_builder, youtube_live_streams_insert_task,
    youtube_live_streams_list_builder, youtube_live_streams_list_task,
    youtube_live_streams_update_builder, youtube_live_streams_update_task,
    youtube_members_list_builder, youtube_members_list_task,
    youtube_memberships_levels_list_builder, youtube_memberships_levels_list_task,
    youtube_playlist_images_delete_builder, youtube_playlist_images_delete_task,
    youtube_playlist_images_insert_builder, youtube_playlist_images_insert_task,
    youtube_playlist_images_list_builder, youtube_playlist_images_list_task,
    youtube_playlist_images_update_builder, youtube_playlist_images_update_task,
    youtube_playlist_items_delete_builder, youtube_playlist_items_delete_task,
    youtube_playlist_items_insert_builder, youtube_playlist_items_insert_task,
    youtube_playlist_items_list_builder, youtube_playlist_items_list_task,
    youtube_playlist_items_update_builder, youtube_playlist_items_update_task,
    youtube_playlists_delete_builder, youtube_playlists_delete_task,
    youtube_playlists_insert_builder, youtube_playlists_insert_task,
    youtube_playlists_list_builder, youtube_playlists_list_task,
    youtube_playlists_update_builder, youtube_playlists_update_task,
    youtube_search_list_builder, youtube_search_list_task,
    youtube_subscriptions_delete_builder, youtube_subscriptions_delete_task,
    youtube_subscriptions_insert_builder, youtube_subscriptions_insert_task,
    youtube_subscriptions_list_builder, youtube_subscriptions_list_task,
    youtube_super_chat_events_list_builder, youtube_super_chat_events_list_task,
    youtube_tests_insert_builder, youtube_tests_insert_task,
    youtube_third_party_links_delete_builder, youtube_third_party_links_delete_task,
    youtube_third_party_links_insert_builder, youtube_third_party_links_insert_task,
    youtube_third_party_links_list_builder, youtube_third_party_links_list_task,
    youtube_third_party_links_update_builder, youtube_third_party_links_update_task,
    youtube_thumbnails_set_builder, youtube_thumbnails_set_task,
    youtube_video_abuse_report_reasons_list_builder, youtube_video_abuse_report_reasons_list_task,
    youtube_video_categories_list_builder, youtube_video_categories_list_task,
    youtube_video_trainability_get_builder, youtube_video_trainability_get_task,
    youtube_videos_delete_builder, youtube_videos_delete_task,
    youtube_videos_get_rating_builder, youtube_videos_get_rating_task,
    youtube_videos_insert_builder, youtube_videos_insert_task,
    youtube_videos_list_builder, youtube_videos_list_task,
    youtube_videos_rate_builder, youtube_videos_rate_task,
    youtube_videos_report_abuse_builder, youtube_videos_report_abuse_task,
    youtube_videos_update_builder, youtube_videos_update_task,
    youtube_watermarks_set_builder, youtube_watermarks_set_task,
    youtube_watermarks_unset_builder, youtube_watermarks_unset_task,
    youtube_youtube_v3_update_comment_threads_builder, youtube_youtube_v3_update_comment_threads_task,
    youtube_youtube_v3_live_chat_messages_stream_builder, youtube_youtube_v3_live_chat_messages_stream_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::youtube::AbuseReport;
use crate::providers::gcp::clients::youtube::ActivityListResponse;
use crate::providers::gcp::clients::youtube::Caption;
use crate::providers::gcp::clients::youtube::CaptionListResponse;
use crate::providers::gcp::clients::youtube::Channel;
use crate::providers::gcp::clients::youtube::ChannelBannerResource;
use crate::providers::gcp::clients::youtube::ChannelListResponse;
use crate::providers::gcp::clients::youtube::ChannelSection;
use crate::providers::gcp::clients::youtube::ChannelSectionListResponse;
use crate::providers::gcp::clients::youtube::Comment;
use crate::providers::gcp::clients::youtube::CommentListResponse;
use crate::providers::gcp::clients::youtube::CommentThread;
use crate::providers::gcp::clients::youtube::CommentThreadListResponse;
use crate::providers::gcp::clients::youtube::Cuepoint;
use crate::providers::gcp::clients::youtube::I18nLanguageListResponse;
use crate::providers::gcp::clients::youtube::I18nRegionListResponse;
use crate::providers::gcp::clients::youtube::LiveBroadcast;
use crate::providers::gcp::clients::youtube::LiveBroadcastListResponse;
use crate::providers::gcp::clients::youtube::LiveChatBan;
use crate::providers::gcp::clients::youtube::LiveChatMessage;
use crate::providers::gcp::clients::youtube::LiveChatMessageListResponse;
use crate::providers::gcp::clients::youtube::LiveChatModerator;
use crate::providers::gcp::clients::youtube::LiveChatModeratorListResponse;
use crate::providers::gcp::clients::youtube::LiveStream;
use crate::providers::gcp::clients::youtube::LiveStreamListResponse;
use crate::providers::gcp::clients::youtube::MemberListResponse;
use crate::providers::gcp::clients::youtube::MembershipsLevelListResponse;
use crate::providers::gcp::clients::youtube::Playlist;
use crate::providers::gcp::clients::youtube::PlaylistImage;
use crate::providers::gcp::clients::youtube::PlaylistImageListResponse;
use crate::providers::gcp::clients::youtube::PlaylistItem;
use crate::providers::gcp::clients::youtube::PlaylistItemListResponse;
use crate::providers::gcp::clients::youtube::PlaylistListResponse;
use crate::providers::gcp::clients::youtube::SearchListResponse;
use crate::providers::gcp::clients::youtube::Subscription;
use crate::providers::gcp::clients::youtube::SubscriptionListResponse;
use crate::providers::gcp::clients::youtube::SuperChatEventListResponse;
use crate::providers::gcp::clients::youtube::TestItem;
use crate::providers::gcp::clients::youtube::ThirdPartyLink;
use crate::providers::gcp::clients::youtube::ThirdPartyLinkListResponse;
use crate::providers::gcp::clients::youtube::ThumbnailSetResponse;
use crate::providers::gcp::clients::youtube::Video;
use crate::providers::gcp::clients::youtube::VideoAbuseReportReasonListResponse;
use crate::providers::gcp::clients::youtube::VideoCategoryListResponse;
use crate::providers::gcp::clients::youtube::VideoGetRatingResponse;
use crate::providers::gcp::clients::youtube::VideoListResponse;
use crate::providers::gcp::clients::youtube::VideoTrainability;
use crate::providers::gcp::clients::youtube::YoutubeAbuseReportsInsertArgs;
use crate::providers::gcp::clients::youtube::YoutubeActivitiesListArgs;
use crate::providers::gcp::clients::youtube::YoutubeCaptionsDeleteArgs;
use crate::providers::gcp::clients::youtube::YoutubeCaptionsDownloadArgs;
use crate::providers::gcp::clients::youtube::YoutubeCaptionsInsertArgs;
use crate::providers::gcp::clients::youtube::YoutubeCaptionsListArgs;
use crate::providers::gcp::clients::youtube::YoutubeCaptionsUpdateArgs;
use crate::providers::gcp::clients::youtube::YoutubeChannelBannersInsertArgs;
use crate::providers::gcp::clients::youtube::YoutubeChannelSectionsDeleteArgs;
use crate::providers::gcp::clients::youtube::YoutubeChannelSectionsInsertArgs;
use crate::providers::gcp::clients::youtube::YoutubeChannelSectionsListArgs;
use crate::providers::gcp::clients::youtube::YoutubeChannelSectionsUpdateArgs;
use crate::providers::gcp::clients::youtube::YoutubeChannelsListArgs;
use crate::providers::gcp::clients::youtube::YoutubeChannelsUpdateArgs;
use crate::providers::gcp::clients::youtube::YoutubeCommentThreadsInsertArgs;
use crate::providers::gcp::clients::youtube::YoutubeCommentThreadsListArgs;
use crate::providers::gcp::clients::youtube::YoutubeCommentsDeleteArgs;
use crate::providers::gcp::clients::youtube::YoutubeCommentsInsertArgs;
use crate::providers::gcp::clients::youtube::YoutubeCommentsListArgs;
use crate::providers::gcp::clients::youtube::YoutubeCommentsMarkAsSpamArgs;
use crate::providers::gcp::clients::youtube::YoutubeCommentsSetModerationStatusArgs;
use crate::providers::gcp::clients::youtube::YoutubeCommentsUpdateArgs;
use crate::providers::gcp::clients::youtube::YoutubeI18NLanguagesListArgs;
use crate::providers::gcp::clients::youtube::YoutubeI18NRegionsListArgs;
use crate::providers::gcp::clients::youtube::YoutubeLiveBroadcastsBindArgs;
use crate::providers::gcp::clients::youtube::YoutubeLiveBroadcastsDeleteArgs;
use crate::providers::gcp::clients::youtube::YoutubeLiveBroadcastsInsertArgs;
use crate::providers::gcp::clients::youtube::YoutubeLiveBroadcastsInsertCuepointArgs;
use crate::providers::gcp::clients::youtube::YoutubeLiveBroadcastsListArgs;
use crate::providers::gcp::clients::youtube::YoutubeLiveBroadcastsTransitionArgs;
use crate::providers::gcp::clients::youtube::YoutubeLiveBroadcastsUpdateArgs;
use crate::providers::gcp::clients::youtube::YoutubeLiveChatBansDeleteArgs;
use crate::providers::gcp::clients::youtube::YoutubeLiveChatBansInsertArgs;
use crate::providers::gcp::clients::youtube::YoutubeLiveChatMessagesDeleteArgs;
use crate::providers::gcp::clients::youtube::YoutubeLiveChatMessagesInsertArgs;
use crate::providers::gcp::clients::youtube::YoutubeLiveChatMessagesListArgs;
use crate::providers::gcp::clients::youtube::YoutubeLiveChatMessagesTransitionArgs;
use crate::providers::gcp::clients::youtube::YoutubeLiveChatModeratorsDeleteArgs;
use crate::providers::gcp::clients::youtube::YoutubeLiveChatModeratorsInsertArgs;
use crate::providers::gcp::clients::youtube::YoutubeLiveChatModeratorsListArgs;
use crate::providers::gcp::clients::youtube::YoutubeLiveStreamsDeleteArgs;
use crate::providers::gcp::clients::youtube::YoutubeLiveStreamsInsertArgs;
use crate::providers::gcp::clients::youtube::YoutubeLiveStreamsListArgs;
use crate::providers::gcp::clients::youtube::YoutubeLiveStreamsUpdateArgs;
use crate::providers::gcp::clients::youtube::YoutubeMembersListArgs;
use crate::providers::gcp::clients::youtube::YoutubeMembershipsLevelsListArgs;
use crate::providers::gcp::clients::youtube::YoutubePlaylistImagesDeleteArgs;
use crate::providers::gcp::clients::youtube::YoutubePlaylistImagesInsertArgs;
use crate::providers::gcp::clients::youtube::YoutubePlaylistImagesListArgs;
use crate::providers::gcp::clients::youtube::YoutubePlaylistImagesUpdateArgs;
use crate::providers::gcp::clients::youtube::YoutubePlaylistItemsDeleteArgs;
use crate::providers::gcp::clients::youtube::YoutubePlaylistItemsInsertArgs;
use crate::providers::gcp::clients::youtube::YoutubePlaylistItemsListArgs;
use crate::providers::gcp::clients::youtube::YoutubePlaylistItemsUpdateArgs;
use crate::providers::gcp::clients::youtube::YoutubePlaylistsDeleteArgs;
use crate::providers::gcp::clients::youtube::YoutubePlaylistsInsertArgs;
use crate::providers::gcp::clients::youtube::YoutubePlaylistsListArgs;
use crate::providers::gcp::clients::youtube::YoutubePlaylistsUpdateArgs;
use crate::providers::gcp::clients::youtube::YoutubeSearchListArgs;
use crate::providers::gcp::clients::youtube::YoutubeSubscriptionsDeleteArgs;
use crate::providers::gcp::clients::youtube::YoutubeSubscriptionsInsertArgs;
use crate::providers::gcp::clients::youtube::YoutubeSubscriptionsListArgs;
use crate::providers::gcp::clients::youtube::YoutubeSuperChatEventsListArgs;
use crate::providers::gcp::clients::youtube::YoutubeTestsInsertArgs;
use crate::providers::gcp::clients::youtube::YoutubeThirdPartyLinksDeleteArgs;
use crate::providers::gcp::clients::youtube::YoutubeThirdPartyLinksInsertArgs;
use crate::providers::gcp::clients::youtube::YoutubeThirdPartyLinksListArgs;
use crate::providers::gcp::clients::youtube::YoutubeThirdPartyLinksUpdateArgs;
use crate::providers::gcp::clients::youtube::YoutubeThumbnailsSetArgs;
use crate::providers::gcp::clients::youtube::YoutubeVideoAbuseReportReasonsListArgs;
use crate::providers::gcp::clients::youtube::YoutubeVideoCategoriesListArgs;
use crate::providers::gcp::clients::youtube::YoutubeVideoTrainabilityGetArgs;
use crate::providers::gcp::clients::youtube::YoutubeVideosDeleteArgs;
use crate::providers::gcp::clients::youtube::YoutubeVideosGetRatingArgs;
use crate::providers::gcp::clients::youtube::YoutubeVideosInsertArgs;
use crate::providers::gcp::clients::youtube::YoutubeVideosListArgs;
use crate::providers::gcp::clients::youtube::YoutubeVideosRateArgs;
use crate::providers::gcp::clients::youtube::YoutubeVideosReportAbuseArgs;
use crate::providers::gcp::clients::youtube::YoutubeVideosUpdateArgs;
use crate::providers::gcp::clients::youtube::YoutubeWatermarksSetArgs;
use crate::providers::gcp::clients::youtube::YoutubeWatermarksUnsetArgs;
use crate::providers::gcp::clients::youtube::YoutubeYoutubeV3LiveChatMessagesStreamArgs;
use crate::providers::gcp::clients::youtube::YoutubeYoutubeV3UpdateCommentThreadsArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// YoutubeProvider with automatic state tracking.
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
/// let provider = YoutubeProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct YoutubeProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> YoutubeProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new YoutubeProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new YoutubeProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Youtube abuse reports insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AbuseReport result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn youtube_abuse_reports_insert(
        &self,
        args: &YoutubeAbuseReportsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AbuseReport, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_abuse_reports_insert_builder(
            &self.http_client,
            &args.part,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_abuse_reports_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube activities list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ActivityListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn youtube_activities_list(
        &self,
        args: &YoutubeActivitiesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ActivityListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_activities_list_builder(
            &self.http_client,
            &args.channelId,
            &args.home,
            &args.maxResults,
            &args.mine,
            &args.pageToken,
            &args.part,
            &args.publishedAfter,
            &args.publishedBefore,
            &args.regionCode,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_activities_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube captions delete.
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
    pub fn youtube_captions_delete(
        &self,
        args: &YoutubeCaptionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_captions_delete_builder(
            &self.http_client,
            &args.id,
            &args.onBehalfOf,
            &args.onBehalfOfContentOwner,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_captions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube captions download.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn youtube_captions_download(
        &self,
        args: &YoutubeCaptionsDownloadArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_captions_download_builder(
            &self.http_client,
            &args.id,
            &args.onBehalfOf,
            &args.onBehalfOfContentOwner,
            &args.tfmt,
            &args.tlang,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_captions_download_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube captions insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Caption result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn youtube_captions_insert(
        &self,
        args: &YoutubeCaptionsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Caption, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_captions_insert_builder(
            &self.http_client,
            &args.onBehalfOf,
            &args.onBehalfOfContentOwner,
            &args.part,
            &args.sync,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_captions_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube captions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CaptionListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn youtube_captions_list(
        &self,
        args: &YoutubeCaptionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CaptionListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_captions_list_builder(
            &self.http_client,
            &args.id,
            &args.onBehalfOf,
            &args.onBehalfOfContentOwner,
            &args.part,
            &args.videoId,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_captions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube captions update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Caption result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn youtube_captions_update(
        &self,
        args: &YoutubeCaptionsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Caption, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_captions_update_builder(
            &self.http_client,
            &args.onBehalfOf,
            &args.onBehalfOfContentOwner,
            &args.part,
            &args.sync,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_captions_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube channel banners insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ChannelBannerResource result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn youtube_channel_banners_insert(
        &self,
        args: &YoutubeChannelBannersInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ChannelBannerResource, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_channel_banners_insert_builder(
            &self.http_client,
            &args.channelId,
            &args.onBehalfOfContentOwner,
            &args.onBehalfOfContentOwnerChannel,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_channel_banners_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube channel sections delete.
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
    pub fn youtube_channel_sections_delete(
        &self,
        args: &YoutubeChannelSectionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_channel_sections_delete_builder(
            &self.http_client,
            &args.id,
            &args.onBehalfOfContentOwner,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_channel_sections_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube channel sections insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ChannelSection result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn youtube_channel_sections_insert(
        &self,
        args: &YoutubeChannelSectionsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ChannelSection, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_channel_sections_insert_builder(
            &self.http_client,
            &args.onBehalfOfContentOwner,
            &args.onBehalfOfContentOwnerChannel,
            &args.part,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_channel_sections_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube channel sections list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ChannelSectionListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn youtube_channel_sections_list(
        &self,
        args: &YoutubeChannelSectionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ChannelSectionListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_channel_sections_list_builder(
            &self.http_client,
            &args.channelId,
            &args.hl,
            &args.id,
            &args.mine,
            &args.onBehalfOfContentOwner,
            &args.part,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_channel_sections_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube channel sections update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ChannelSection result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn youtube_channel_sections_update(
        &self,
        args: &YoutubeChannelSectionsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ChannelSection, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_channel_sections_update_builder(
            &self.http_client,
            &args.onBehalfOfContentOwner,
            &args.part,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_channel_sections_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube channels list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ChannelListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn youtube_channels_list(
        &self,
        args: &YoutubeChannelsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ChannelListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_channels_list_builder(
            &self.http_client,
            &args.categoryId,
            &args.forHandle,
            &args.forUsername,
            &args.hl,
            &args.id,
            &args.managedByMe,
            &args.maxResults,
            &args.mine,
            &args.mySubscribers,
            &args.onBehalfOfContentOwner,
            &args.pageToken,
            &args.part,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_channels_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube channels update.
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
    pub fn youtube_channels_update(
        &self,
        args: &YoutubeChannelsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Channel, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_channels_update_builder(
            &self.http_client,
            &args.onBehalfOfContentOwner,
            &args.part,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_channels_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube comment threads insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CommentThread result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn youtube_comment_threads_insert(
        &self,
        args: &YoutubeCommentThreadsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CommentThread, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_comment_threads_insert_builder(
            &self.http_client,
            &args.part,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_comment_threads_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube comment threads list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CommentThreadListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn youtube_comment_threads_list(
        &self,
        args: &YoutubeCommentThreadsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CommentThreadListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_comment_threads_list_builder(
            &self.http_client,
            &args.allThreadsRelatedToChannelId,
            &args.channelId,
            &args.id,
            &args.maxResults,
            &args.moderationStatus,
            &args.order,
            &args.pageToken,
            &args.part,
            &args.postId,
            &args.searchTerms,
            &args.textFormat,
            &args.videoId,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_comment_threads_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube comments delete.
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
    pub fn youtube_comments_delete(
        &self,
        args: &YoutubeCommentsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_comments_delete_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_comments_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube comments insert.
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
    pub fn youtube_comments_insert(
        &self,
        args: &YoutubeCommentsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Comment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_comments_insert_builder(
            &self.http_client,
            &args.part,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_comments_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube comments list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CommentListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn youtube_comments_list(
        &self,
        args: &YoutubeCommentsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CommentListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_comments_list_builder(
            &self.http_client,
            &args.id,
            &args.maxResults,
            &args.pageToken,
            &args.parentId,
            &args.part,
            &args.textFormat,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_comments_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube comments mark as spam.
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
    pub fn youtube_comments_mark_as_spam(
        &self,
        args: &YoutubeCommentsMarkAsSpamArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_comments_mark_as_spam_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_comments_mark_as_spam_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube comments set moderation status.
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
    pub fn youtube_comments_set_moderation_status(
        &self,
        args: &YoutubeCommentsSetModerationStatusArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_comments_set_moderation_status_builder(
            &self.http_client,
            &args.banAuthor,
            &args.id,
            &args.moderationStatus,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_comments_set_moderation_status_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube comments update.
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
    pub fn youtube_comments_update(
        &self,
        args: &YoutubeCommentsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Comment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_comments_update_builder(
            &self.http_client,
            &args.part,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_comments_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube i18n languages list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the I18nLanguageListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn youtube_i18n_languages_list(
        &self,
        args: &YoutubeI18NLanguagesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<I18nLanguageListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_i18n_languages_list_builder(
            &self.http_client,
            &args.hl,
            &args.part,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_i18n_languages_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube i18n regions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the I18nRegionListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn youtube_i18n_regions_list(
        &self,
        args: &YoutubeI18NRegionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<I18nRegionListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_i18n_regions_list_builder(
            &self.http_client,
            &args.hl,
            &args.part,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_i18n_regions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube live broadcasts bind.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LiveBroadcast result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn youtube_live_broadcasts_bind(
        &self,
        args: &YoutubeLiveBroadcastsBindArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LiveBroadcast, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_live_broadcasts_bind_builder(
            &self.http_client,
            &args.id,
            &args.onBehalfOfContentOwner,
            &args.onBehalfOfContentOwnerChannel,
            &args.part,
            &args.streamId,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_live_broadcasts_bind_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube live broadcasts delete.
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
    pub fn youtube_live_broadcasts_delete(
        &self,
        args: &YoutubeLiveBroadcastsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_live_broadcasts_delete_builder(
            &self.http_client,
            &args.id,
            &args.onBehalfOfContentOwner,
            &args.onBehalfOfContentOwnerChannel,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_live_broadcasts_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube live broadcasts insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LiveBroadcast result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn youtube_live_broadcasts_insert(
        &self,
        args: &YoutubeLiveBroadcastsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LiveBroadcast, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_live_broadcasts_insert_builder(
            &self.http_client,
            &args.onBehalfOfContentOwner,
            &args.onBehalfOfContentOwnerChannel,
            &args.part,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_live_broadcasts_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube live broadcasts insert cuepoint.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Cuepoint result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn youtube_live_broadcasts_insert_cuepoint(
        &self,
        args: &YoutubeLiveBroadcastsInsertCuepointArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Cuepoint, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_live_broadcasts_insert_cuepoint_builder(
            &self.http_client,
            &args.id,
            &args.onBehalfOfContentOwner,
            &args.onBehalfOfContentOwnerChannel,
            &args.part,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_live_broadcasts_insert_cuepoint_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube live broadcasts list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LiveBroadcastListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn youtube_live_broadcasts_list(
        &self,
        args: &YoutubeLiveBroadcastsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LiveBroadcastListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_live_broadcasts_list_builder(
            &self.http_client,
            &args.broadcastStatus,
            &args.broadcastType,
            &args.id,
            &args.maxResults,
            &args.mine,
            &args.onBehalfOfContentOwner,
            &args.onBehalfOfContentOwnerChannel,
            &args.pageToken,
            &args.part,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_live_broadcasts_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube live broadcasts transition.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LiveBroadcast result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn youtube_live_broadcasts_transition(
        &self,
        args: &YoutubeLiveBroadcastsTransitionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LiveBroadcast, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_live_broadcasts_transition_builder(
            &self.http_client,
            &args.broadcastStatus,
            &args.id,
            &args.onBehalfOfContentOwner,
            &args.onBehalfOfContentOwnerChannel,
            &args.part,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_live_broadcasts_transition_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube live broadcasts update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LiveBroadcast result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn youtube_live_broadcasts_update(
        &self,
        args: &YoutubeLiveBroadcastsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LiveBroadcast, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_live_broadcasts_update_builder(
            &self.http_client,
            &args.onBehalfOfContentOwner,
            &args.onBehalfOfContentOwnerChannel,
            &args.part,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_live_broadcasts_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube live chat bans delete.
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
    pub fn youtube_live_chat_bans_delete(
        &self,
        args: &YoutubeLiveChatBansDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_live_chat_bans_delete_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_live_chat_bans_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube live chat bans insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LiveChatBan result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn youtube_live_chat_bans_insert(
        &self,
        args: &YoutubeLiveChatBansInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LiveChatBan, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_live_chat_bans_insert_builder(
            &self.http_client,
            &args.part,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_live_chat_bans_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube live chat messages delete.
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
    pub fn youtube_live_chat_messages_delete(
        &self,
        args: &YoutubeLiveChatMessagesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_live_chat_messages_delete_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_live_chat_messages_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube live chat messages insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LiveChatMessage result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn youtube_live_chat_messages_insert(
        &self,
        args: &YoutubeLiveChatMessagesInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LiveChatMessage, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_live_chat_messages_insert_builder(
            &self.http_client,
            &args.part,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_live_chat_messages_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube live chat messages list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LiveChatMessageListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn youtube_live_chat_messages_list(
        &self,
        args: &YoutubeLiveChatMessagesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LiveChatMessageListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_live_chat_messages_list_builder(
            &self.http_client,
            &args.hl,
            &args.liveChatId,
            &args.maxResults,
            &args.pageToken,
            &args.part,
            &args.profileImageSize,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_live_chat_messages_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube live chat messages transition.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LiveChatMessage result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn youtube_live_chat_messages_transition(
        &self,
        args: &YoutubeLiveChatMessagesTransitionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LiveChatMessage, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_live_chat_messages_transition_builder(
            &self.http_client,
            &args.id,
            &args.status,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_live_chat_messages_transition_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube live chat moderators delete.
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
    pub fn youtube_live_chat_moderators_delete(
        &self,
        args: &YoutubeLiveChatModeratorsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_live_chat_moderators_delete_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_live_chat_moderators_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube live chat moderators insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LiveChatModerator result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn youtube_live_chat_moderators_insert(
        &self,
        args: &YoutubeLiveChatModeratorsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LiveChatModerator, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_live_chat_moderators_insert_builder(
            &self.http_client,
            &args.part,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_live_chat_moderators_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube live chat moderators list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LiveChatModeratorListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn youtube_live_chat_moderators_list(
        &self,
        args: &YoutubeLiveChatModeratorsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LiveChatModeratorListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_live_chat_moderators_list_builder(
            &self.http_client,
            &args.liveChatId,
            &args.maxResults,
            &args.pageToken,
            &args.part,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_live_chat_moderators_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube live streams delete.
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
    pub fn youtube_live_streams_delete(
        &self,
        args: &YoutubeLiveStreamsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_live_streams_delete_builder(
            &self.http_client,
            &args.id,
            &args.onBehalfOfContentOwner,
            &args.onBehalfOfContentOwnerChannel,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_live_streams_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube live streams insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LiveStream result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn youtube_live_streams_insert(
        &self,
        args: &YoutubeLiveStreamsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LiveStream, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_live_streams_insert_builder(
            &self.http_client,
            &args.onBehalfOfContentOwner,
            &args.onBehalfOfContentOwnerChannel,
            &args.part,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_live_streams_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube live streams list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LiveStreamListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn youtube_live_streams_list(
        &self,
        args: &YoutubeLiveStreamsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LiveStreamListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_live_streams_list_builder(
            &self.http_client,
            &args.id,
            &args.maxResults,
            &args.mine,
            &args.onBehalfOfContentOwner,
            &args.onBehalfOfContentOwnerChannel,
            &args.pageToken,
            &args.part,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_live_streams_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube live streams update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LiveStream result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn youtube_live_streams_update(
        &self,
        args: &YoutubeLiveStreamsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LiveStream, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_live_streams_update_builder(
            &self.http_client,
            &args.onBehalfOfContentOwner,
            &args.onBehalfOfContentOwnerChannel,
            &args.part,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_live_streams_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube members list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the MemberListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn youtube_members_list(
        &self,
        args: &YoutubeMembersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<MemberListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_members_list_builder(
            &self.http_client,
            &args.filterByMemberChannelId,
            &args.hasAccessToLevel,
            &args.maxResults,
            &args.mode,
            &args.pageToken,
            &args.part,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_members_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube memberships levels list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the MembershipsLevelListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn youtube_memberships_levels_list(
        &self,
        args: &YoutubeMembershipsLevelsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<MembershipsLevelListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_memberships_levels_list_builder(
            &self.http_client,
            &args.part,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_memberships_levels_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube playlist images delete.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn youtube_playlist_images_delete(
        &self,
        args: &YoutubePlaylistImagesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_playlist_images_delete_builder(
            &self.http_client,
            &args.id,
            &args.onBehalfOfContentOwner,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_playlist_images_delete_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube playlist images insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PlaylistImage result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn youtube_playlist_images_insert(
        &self,
        args: &YoutubePlaylistImagesInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PlaylistImage, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_playlist_images_insert_builder(
            &self.http_client,
            &args.onBehalfOfContentOwner,
            &args.onBehalfOfContentOwnerChannel,
            &args.part,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_playlist_images_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube playlist images list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PlaylistImageListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn youtube_playlist_images_list(
        &self,
        args: &YoutubePlaylistImagesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PlaylistImageListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_playlist_images_list_builder(
            &self.http_client,
            &args.maxResults,
            &args.onBehalfOfContentOwner,
            &args.onBehalfOfContentOwnerChannel,
            &args.pageToken,
            &args.parent,
            &args.part,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_playlist_images_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube playlist images update.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PlaylistImage result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn youtube_playlist_images_update(
        &self,
        args: &YoutubePlaylistImagesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PlaylistImage, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_playlist_images_update_builder(
            &self.http_client,
            &args.onBehalfOfContentOwner,
            &args.part,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_playlist_images_update_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube playlist items delete.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn youtube_playlist_items_delete(
        &self,
        args: &YoutubePlaylistItemsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_playlist_items_delete_builder(
            &self.http_client,
            &args.id,
            &args.onBehalfOfContentOwner,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_playlist_items_delete_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube playlist items insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PlaylistItem result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn youtube_playlist_items_insert(
        &self,
        args: &YoutubePlaylistItemsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PlaylistItem, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_playlist_items_insert_builder(
            &self.http_client,
            &args.onBehalfOfContentOwner,
            &args.part,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_playlist_items_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube playlist items list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PlaylistItemListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn youtube_playlist_items_list(
        &self,
        args: &YoutubePlaylistItemsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PlaylistItemListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_playlist_items_list_builder(
            &self.http_client,
            &args.id,
            &args.maxResults,
            &args.onBehalfOfContentOwner,
            &args.pageToken,
            &args.part,
            &args.playlistId,
            &args.videoId,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_playlist_items_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube playlist items update.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PlaylistItem result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn youtube_playlist_items_update(
        &self,
        args: &YoutubePlaylistItemsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PlaylistItem, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_playlist_items_update_builder(
            &self.http_client,
            &args.onBehalfOfContentOwner,
            &args.part,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_playlist_items_update_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube playlists delete.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn youtube_playlists_delete(
        &self,
        args: &YoutubePlaylistsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_playlists_delete_builder(
            &self.http_client,
            &args.id,
            &args.onBehalfOfContentOwner,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_playlists_delete_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube playlists insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Playlist result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn youtube_playlists_insert(
        &self,
        args: &YoutubePlaylistsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Playlist, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_playlists_insert_builder(
            &self.http_client,
            &args.onBehalfOfContentOwner,
            &args.onBehalfOfContentOwnerChannel,
            &args.part,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_playlists_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube playlists list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PlaylistListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn youtube_playlists_list(
        &self,
        args: &YoutubePlaylistsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PlaylistListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_playlists_list_builder(
            &self.http_client,
            &args.channelId,
            &args.hl,
            &args.id,
            &args.maxResults,
            &args.mine,
            &args.onBehalfOfContentOwner,
            &args.onBehalfOfContentOwnerChannel,
            &args.pageToken,
            &args.part,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_playlists_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube playlists update.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Playlist result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn youtube_playlists_update(
        &self,
        args: &YoutubePlaylistsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Playlist, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_playlists_update_builder(
            &self.http_client,
            &args.onBehalfOfContentOwner,
            &args.part,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_playlists_update_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube search list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn youtube_search_list(
        &self,
        args: &YoutubeSearchListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_search_list_builder(
            &self.http_client,
            &args.channelId,
            &args.channelType,
            &args.eventType,
            &args.forContentOwner,
            &args.forDeveloper,
            &args.forMine,
            &args.location,
            &args.locationRadius,
            &args.maxResults,
            &args.onBehalfOfContentOwner,
            &args.order,
            &args.pageToken,
            &args.part,
            &args.publishedAfter,
            &args.publishedBefore,
            &args.q,
            &args.regionCode,
            &args.relevanceLanguage,
            &args.safeSearch,
            &args.topicId,
            &args.type_rs,
            &args.videoCaption,
            &args.videoCategoryId,
            &args.videoDefinition,
            &args.videoDimension,
            &args.videoDuration,
            &args.videoEmbeddable,
            &args.videoLicense,
            &args.videoPaidProductPlacement,
            &args.videoSyndicated,
            &args.videoType,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_search_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube subscriptions delete.
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
    pub fn youtube_subscriptions_delete(
        &self,
        args: &YoutubeSubscriptionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_subscriptions_delete_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_subscriptions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube subscriptions insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Subscription result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn youtube_subscriptions_insert(
        &self,
        args: &YoutubeSubscriptionsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Subscription, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_subscriptions_insert_builder(
            &self.http_client,
            &args.part,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_subscriptions_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube subscriptions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SubscriptionListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn youtube_subscriptions_list(
        &self,
        args: &YoutubeSubscriptionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SubscriptionListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_subscriptions_list_builder(
            &self.http_client,
            &args.channelId,
            &args.forChannelId,
            &args.id,
            &args.maxResults,
            &args.mine,
            &args.myRecentSubscribers,
            &args.mySubscribers,
            &args.onBehalfOfContentOwner,
            &args.onBehalfOfContentOwnerChannel,
            &args.order,
            &args.pageToken,
            &args.part,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_subscriptions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube super chat events list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SuperChatEventListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn youtube_super_chat_events_list(
        &self,
        args: &YoutubeSuperChatEventsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SuperChatEventListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_super_chat_events_list_builder(
            &self.http_client,
            &args.hl,
            &args.maxResults,
            &args.pageToken,
            &args.part,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_super_chat_events_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube tests insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TestItem result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn youtube_tests_insert(
        &self,
        args: &YoutubeTestsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestItem, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_tests_insert_builder(
            &self.http_client,
            &args.externalChannelId,
            &args.onBehalfOfContentOwnerChannel,
            &args.part,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_tests_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube third party links delete.
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
    pub fn youtube_third_party_links_delete(
        &self,
        args: &YoutubeThirdPartyLinksDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_third_party_links_delete_builder(
            &self.http_client,
            &args.externalChannelId,
            &args.linkingToken,
            &args.part,
            &args.type_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_third_party_links_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube third party links insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ThirdPartyLink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn youtube_third_party_links_insert(
        &self,
        args: &YoutubeThirdPartyLinksInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ThirdPartyLink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_third_party_links_insert_builder(
            &self.http_client,
            &args.externalChannelId,
            &args.part,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_third_party_links_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube third party links list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ThirdPartyLinkListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn youtube_third_party_links_list(
        &self,
        args: &YoutubeThirdPartyLinksListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ThirdPartyLinkListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_third_party_links_list_builder(
            &self.http_client,
            &args.externalChannelId,
            &args.linkingToken,
            &args.part,
            &args.type_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_third_party_links_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube third party links update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ThirdPartyLink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn youtube_third_party_links_update(
        &self,
        args: &YoutubeThirdPartyLinksUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ThirdPartyLink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_third_party_links_update_builder(
            &self.http_client,
            &args.externalChannelId,
            &args.part,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_third_party_links_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube thumbnails set.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ThumbnailSetResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn youtube_thumbnails_set(
        &self,
        args: &YoutubeThumbnailsSetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ThumbnailSetResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_thumbnails_set_builder(
            &self.http_client,
            &args.onBehalfOfContentOwner,
            &args.videoId,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_thumbnails_set_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube video abuse report reasons list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the VideoAbuseReportReasonListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn youtube_video_abuse_report_reasons_list(
        &self,
        args: &YoutubeVideoAbuseReportReasonsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<VideoAbuseReportReasonListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_video_abuse_report_reasons_list_builder(
            &self.http_client,
            &args.hl,
            &args.part,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_video_abuse_report_reasons_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube video categories list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the VideoCategoryListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn youtube_video_categories_list(
        &self,
        args: &YoutubeVideoCategoriesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<VideoCategoryListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_video_categories_list_builder(
            &self.http_client,
            &args.hl,
            &args.id,
            &args.part,
            &args.regionCode,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_video_categories_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube video trainability get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the VideoTrainability result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn youtube_video_trainability_get(
        &self,
        args: &YoutubeVideoTrainabilityGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<VideoTrainability, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_video_trainability_get_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_video_trainability_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube videos delete.
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
    pub fn youtube_videos_delete(
        &self,
        args: &YoutubeVideosDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_videos_delete_builder(
            &self.http_client,
            &args.id,
            &args.onBehalfOfContentOwner,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_videos_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube videos get rating.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the VideoGetRatingResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn youtube_videos_get_rating(
        &self,
        args: &YoutubeVideosGetRatingArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<VideoGetRatingResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_videos_get_rating_builder(
            &self.http_client,
            &args.id,
            &args.onBehalfOfContentOwner,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_videos_get_rating_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube videos insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Video result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn youtube_videos_insert(
        &self,
        args: &YoutubeVideosInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Video, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_videos_insert_builder(
            &self.http_client,
            &args.autoLevels,
            &args.notifySubscribers,
            &args.onBehalfOfContentOwner,
            &args.onBehalfOfContentOwnerChannel,
            &args.part,
            &args.stabilize,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_videos_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube videos list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the VideoListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn youtube_videos_list(
        &self,
        args: &YoutubeVideosListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<VideoListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_videos_list_builder(
            &self.http_client,
            &args.chart,
            &args.hl,
            &args.id,
            &args.locale,
            &args.maxHeight,
            &args.maxResults,
            &args.maxWidth,
            &args.myRating,
            &args.onBehalfOfContentOwner,
            &args.pageToken,
            &args.part,
            &args.regionCode,
            &args.videoCategoryId,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_videos_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube videos rate.
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
    pub fn youtube_videos_rate(
        &self,
        args: &YoutubeVideosRateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_videos_rate_builder(
            &self.http_client,
            &args.id,
            &args.rating,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_videos_rate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube videos report abuse.
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
    pub fn youtube_videos_report_abuse(
        &self,
        args: &YoutubeVideosReportAbuseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_videos_report_abuse_builder(
            &self.http_client,
            &args.onBehalfOfContentOwner,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_videos_report_abuse_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube videos update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Video result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn youtube_videos_update(
        &self,
        args: &YoutubeVideosUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Video, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_videos_update_builder(
            &self.http_client,
            &args.onBehalfOfContentOwner,
            &args.part,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_videos_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube watermarks set.
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
    pub fn youtube_watermarks_set(
        &self,
        args: &YoutubeWatermarksSetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_watermarks_set_builder(
            &self.http_client,
            &args.channelId,
            &args.onBehalfOfContentOwner,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_watermarks_set_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube watermarks unset.
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
    pub fn youtube_watermarks_unset(
        &self,
        args: &YoutubeWatermarksUnsetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_watermarks_unset_builder(
            &self.http_client,
            &args.channelId,
            &args.onBehalfOfContentOwner,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_watermarks_unset_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube youtube v3 update comment threads.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CommentThread result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn youtube_youtube_v3_update_comment_threads(
        &self,
        args: &YoutubeYoutubeV3UpdateCommentThreadsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CommentThread, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_youtube_v3_update_comment_threads_builder(
            &self.http_client,
            &args.part,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_youtube_v3_update_comment_threads_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube youtube v3 live chat messages stream.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LiveChatMessageListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn youtube_youtube_v3_live_chat_messages_stream(
        &self,
        args: &YoutubeYoutubeV3LiveChatMessagesStreamArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LiveChatMessageListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_youtube_v3_live_chat_messages_stream_builder(
            &self.http_client,
            &args.hl,
            &args.liveChatId,
            &args.maxResults,
            &args.pageToken,
            &args.part,
            &args.profileImageSize,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_youtube_v3_live_chat_messages_stream_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
