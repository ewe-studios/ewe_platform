//! BloggerProvider - State-aware blogger API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       blogger API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::blogger::{
    blogger_blog_user_infos_get_builder, blogger_blog_user_infos_get_task,
    blogger_blogs_get_builder, blogger_blogs_get_task,
    blogger_blogs_get_by_url_builder, blogger_blogs_get_by_url_task,
    blogger_blogs_list_by_user_builder, blogger_blogs_list_by_user_task,
    blogger_comments_approve_builder, blogger_comments_approve_task,
    blogger_comments_delete_builder, blogger_comments_delete_task,
    blogger_comments_get_builder, blogger_comments_get_task,
    blogger_comments_list_builder, blogger_comments_list_task,
    blogger_comments_list_by_blog_builder, blogger_comments_list_by_blog_task,
    blogger_comments_mark_as_spam_builder, blogger_comments_mark_as_spam_task,
    blogger_comments_remove_content_builder, blogger_comments_remove_content_task,
    blogger_page_views_get_builder, blogger_page_views_get_task,
    blogger_pages_delete_builder, blogger_pages_delete_task,
    blogger_pages_get_builder, blogger_pages_get_task,
    blogger_pages_insert_builder, blogger_pages_insert_task,
    blogger_pages_list_builder, blogger_pages_list_task,
    blogger_pages_patch_builder, blogger_pages_patch_task,
    blogger_pages_publish_builder, blogger_pages_publish_task,
    blogger_pages_revert_builder, blogger_pages_revert_task,
    blogger_pages_update_builder, blogger_pages_update_task,
    blogger_post_user_infos_get_builder, blogger_post_user_infos_get_task,
    blogger_post_user_infos_list_builder, blogger_post_user_infos_list_task,
    blogger_posts_delete_builder, blogger_posts_delete_task,
    blogger_posts_get_builder, blogger_posts_get_task,
    blogger_posts_get_by_path_builder, blogger_posts_get_by_path_task,
    blogger_posts_insert_builder, blogger_posts_insert_task,
    blogger_posts_list_builder, blogger_posts_list_task,
    blogger_posts_patch_builder, blogger_posts_patch_task,
    blogger_posts_publish_builder, blogger_posts_publish_task,
    blogger_posts_revert_builder, blogger_posts_revert_task,
    blogger_posts_search_builder, blogger_posts_search_task,
    blogger_posts_update_builder, blogger_posts_update_task,
    blogger_users_get_builder, blogger_users_get_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::blogger::Blog;
use crate::providers::gcp::clients::blogger::BlogList;
use crate::providers::gcp::clients::blogger::BlogUserInfo;
use crate::providers::gcp::clients::blogger::Comment;
use crate::providers::gcp::clients::blogger::CommentList;
use crate::providers::gcp::clients::blogger::Page;
use crate::providers::gcp::clients::blogger::PageList;
use crate::providers::gcp::clients::blogger::Pageviews;
use crate::providers::gcp::clients::blogger::Post;
use crate::providers::gcp::clients::blogger::PostList;
use crate::providers::gcp::clients::blogger::PostUserInfo;
use crate::providers::gcp::clients::blogger::PostUserInfosList;
use crate::providers::gcp::clients::blogger::User;
use crate::providers::gcp::clients::blogger::BloggerBlogUserInfosGetArgs;
use crate::providers::gcp::clients::blogger::BloggerBlogsGetArgs;
use crate::providers::gcp::clients::blogger::BloggerBlogsGetByUrlArgs;
use crate::providers::gcp::clients::blogger::BloggerBlogsListByUserArgs;
use crate::providers::gcp::clients::blogger::BloggerCommentsApproveArgs;
use crate::providers::gcp::clients::blogger::BloggerCommentsDeleteArgs;
use crate::providers::gcp::clients::blogger::BloggerCommentsGetArgs;
use crate::providers::gcp::clients::blogger::BloggerCommentsListArgs;
use crate::providers::gcp::clients::blogger::BloggerCommentsListByBlogArgs;
use crate::providers::gcp::clients::blogger::BloggerCommentsMarkAsSpamArgs;
use crate::providers::gcp::clients::blogger::BloggerCommentsRemoveContentArgs;
use crate::providers::gcp::clients::blogger::BloggerPageViewsGetArgs;
use crate::providers::gcp::clients::blogger::BloggerPagesDeleteArgs;
use crate::providers::gcp::clients::blogger::BloggerPagesGetArgs;
use crate::providers::gcp::clients::blogger::BloggerPagesInsertArgs;
use crate::providers::gcp::clients::blogger::BloggerPagesListArgs;
use crate::providers::gcp::clients::blogger::BloggerPagesPatchArgs;
use crate::providers::gcp::clients::blogger::BloggerPagesPublishArgs;
use crate::providers::gcp::clients::blogger::BloggerPagesRevertArgs;
use crate::providers::gcp::clients::blogger::BloggerPagesUpdateArgs;
use crate::providers::gcp::clients::blogger::BloggerPostUserInfosGetArgs;
use crate::providers::gcp::clients::blogger::BloggerPostUserInfosListArgs;
use crate::providers::gcp::clients::blogger::BloggerPostsDeleteArgs;
use crate::providers::gcp::clients::blogger::BloggerPostsGetArgs;
use crate::providers::gcp::clients::blogger::BloggerPostsGetByPathArgs;
use crate::providers::gcp::clients::blogger::BloggerPostsInsertArgs;
use crate::providers::gcp::clients::blogger::BloggerPostsListArgs;
use crate::providers::gcp::clients::blogger::BloggerPostsPatchArgs;
use crate::providers::gcp::clients::blogger::BloggerPostsPublishArgs;
use crate::providers::gcp::clients::blogger::BloggerPostsRevertArgs;
use crate::providers::gcp::clients::blogger::BloggerPostsSearchArgs;
use crate::providers::gcp::clients::blogger::BloggerPostsUpdateArgs;
use crate::providers::gcp::clients::blogger::BloggerUsersGetArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// BloggerProvider with automatic state tracking.
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
/// let provider = BloggerProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct BloggerProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> BloggerProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new BloggerProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new BloggerProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Blogger blog user infos get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BlogUserInfo result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn blogger_blog_user_infos_get(
        &self,
        args: &BloggerBlogUserInfosGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BlogUserInfo, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = blogger_blog_user_infos_get_builder(
            &self.http_client,
            &args.userId,
            &args.blogId,
            &args.maxPosts,
        )
        .map_err(ProviderError::Api)?;

        let task = blogger_blog_user_infos_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Blogger blogs get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Blog result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn blogger_blogs_get(
        &self,
        args: &BloggerBlogsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Blog, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = blogger_blogs_get_builder(
            &self.http_client,
            &args.blogId,
            &args.maxPosts,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = blogger_blogs_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Blogger blogs get by url.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Blog result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn blogger_blogs_get_by_url(
        &self,
        args: &BloggerBlogsGetByUrlArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Blog, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = blogger_blogs_get_by_url_builder(
            &self.http_client,
            &args.url,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = blogger_blogs_get_by_url_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Blogger blogs list by user.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BlogList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn blogger_blogs_list_by_user(
        &self,
        args: &BloggerBlogsListByUserArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BlogList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = blogger_blogs_list_by_user_builder(
            &self.http_client,
            &args.userId,
            &args.fetchUserInfo,
            &args.role,
            &args.status,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = blogger_blogs_list_by_user_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Blogger comments approve.
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
    pub fn blogger_comments_approve(
        &self,
        args: &BloggerCommentsApproveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Comment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = blogger_comments_approve_builder(
            &self.http_client,
            &args.blogId,
            &args.postId,
            &args.commentId,
        )
        .map_err(ProviderError::Api)?;

        let task = blogger_comments_approve_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Blogger comments delete.
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
    pub fn blogger_comments_delete(
        &self,
        args: &BloggerCommentsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = blogger_comments_delete_builder(
            &self.http_client,
            &args.blogId,
            &args.postId,
            &args.commentId,
        )
        .map_err(ProviderError::Api)?;

        let task = blogger_comments_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Blogger comments get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn blogger_comments_get(
        &self,
        args: &BloggerCommentsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Comment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = blogger_comments_get_builder(
            &self.http_client,
            &args.blogId,
            &args.postId,
            &args.commentId,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = blogger_comments_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Blogger comments list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CommentList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn blogger_comments_list(
        &self,
        args: &BloggerCommentsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CommentList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = blogger_comments_list_builder(
            &self.http_client,
            &args.blogId,
            &args.postId,
            &args.endDate,
            &args.fetchBodies,
            &args.maxResults,
            &args.pageToken,
            &args.startDate,
            &args.status,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = blogger_comments_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Blogger comments list by blog.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CommentList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn blogger_comments_list_by_blog(
        &self,
        args: &BloggerCommentsListByBlogArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CommentList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = blogger_comments_list_by_blog_builder(
            &self.http_client,
            &args.blogId,
            &args.endDate,
            &args.fetchBodies,
            &args.maxResults,
            &args.pageToken,
            &args.startDate,
            &args.status,
        )
        .map_err(ProviderError::Api)?;

        let task = blogger_comments_list_by_blog_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Blogger comments mark as spam.
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
    pub fn blogger_comments_mark_as_spam(
        &self,
        args: &BloggerCommentsMarkAsSpamArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Comment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = blogger_comments_mark_as_spam_builder(
            &self.http_client,
            &args.blogId,
            &args.postId,
            &args.commentId,
        )
        .map_err(ProviderError::Api)?;

        let task = blogger_comments_mark_as_spam_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Blogger comments remove content.
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
    pub fn blogger_comments_remove_content(
        &self,
        args: &BloggerCommentsRemoveContentArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Comment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = blogger_comments_remove_content_builder(
            &self.http_client,
            &args.blogId,
            &args.postId,
            &args.commentId,
        )
        .map_err(ProviderError::Api)?;

        let task = blogger_comments_remove_content_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Blogger page views get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Pageviews result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn blogger_page_views_get(
        &self,
        args: &BloggerPageViewsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Pageviews, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = blogger_page_views_get_builder(
            &self.http_client,
            &args.blogId,
            &args.range,
        )
        .map_err(ProviderError::Api)?;

        let task = blogger_page_views_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Blogger pages delete.
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
    pub fn blogger_pages_delete(
        &self,
        args: &BloggerPagesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = blogger_pages_delete_builder(
            &self.http_client,
            &args.blogId,
            &args.pageId,
            &args.useTrash,
        )
        .map_err(ProviderError::Api)?;

        let task = blogger_pages_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Blogger pages get.
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
    pub fn blogger_pages_get(
        &self,
        args: &BloggerPagesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Page, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = blogger_pages_get_builder(
            &self.http_client,
            &args.blogId,
            &args.pageId,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = blogger_pages_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Blogger pages insert.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn blogger_pages_insert(
        &self,
        args: &BloggerPagesInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Page, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = blogger_pages_insert_builder(
            &self.http_client,
            &args.blogId,
            &args.isDraft,
        )
        .map_err(ProviderError::Api)?;

        let task = blogger_pages_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Blogger pages list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PageList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn blogger_pages_list(
        &self,
        args: &BloggerPagesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PageList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = blogger_pages_list_builder(
            &self.http_client,
            &args.blogId,
            &args.fetchBodies,
            &args.maxResults,
            &args.pageToken,
            &args.status,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = blogger_pages_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Blogger pages patch.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn blogger_pages_patch(
        &self,
        args: &BloggerPagesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Page, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = blogger_pages_patch_builder(
            &self.http_client,
            &args.blogId,
            &args.pageId,
            &args.publish,
            &args.revert,
        )
        .map_err(ProviderError::Api)?;

        let task = blogger_pages_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Blogger pages publish.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn blogger_pages_publish(
        &self,
        args: &BloggerPagesPublishArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Page, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = blogger_pages_publish_builder(
            &self.http_client,
            &args.blogId,
            &args.pageId,
        )
        .map_err(ProviderError::Api)?;

        let task = blogger_pages_publish_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Blogger pages revert.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn blogger_pages_revert(
        &self,
        args: &BloggerPagesRevertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Page, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = blogger_pages_revert_builder(
            &self.http_client,
            &args.blogId,
            &args.pageId,
        )
        .map_err(ProviderError::Api)?;

        let task = blogger_pages_revert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Blogger pages update.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn blogger_pages_update(
        &self,
        args: &BloggerPagesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Page, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = blogger_pages_update_builder(
            &self.http_client,
            &args.blogId,
            &args.pageId,
            &args.publish,
            &args.revert,
        )
        .map_err(ProviderError::Api)?;

        let task = blogger_pages_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Blogger post user infos get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PostUserInfo result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn blogger_post_user_infos_get(
        &self,
        args: &BloggerPostUserInfosGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PostUserInfo, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = blogger_post_user_infos_get_builder(
            &self.http_client,
            &args.userId,
            &args.blogId,
            &args.postId,
            &args.maxComments,
        )
        .map_err(ProviderError::Api)?;

        let task = blogger_post_user_infos_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Blogger post user infos list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PostUserInfosList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn blogger_post_user_infos_list(
        &self,
        args: &BloggerPostUserInfosListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PostUserInfosList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = blogger_post_user_infos_list_builder(
            &self.http_client,
            &args.userId,
            &args.blogId,
            &args.endDate,
            &args.fetchBodies,
            &args.labels,
            &args.maxResults,
            &args.orderBy,
            &args.pageToken,
            &args.startDate,
            &args.status,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = blogger_post_user_infos_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Blogger posts delete.
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
    pub fn blogger_posts_delete(
        &self,
        args: &BloggerPostsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = blogger_posts_delete_builder(
            &self.http_client,
            &args.blogId,
            &args.postId,
            &args.useTrash,
        )
        .map_err(ProviderError::Api)?;

        let task = blogger_posts_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Blogger posts get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Post result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn blogger_posts_get(
        &self,
        args: &BloggerPostsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Post, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = blogger_posts_get_builder(
            &self.http_client,
            &args.blogId,
            &args.postId,
            &args.fetchBody,
            &args.fetchImages,
            &args.maxComments,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = blogger_posts_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Blogger posts get by path.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Post result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn blogger_posts_get_by_path(
        &self,
        args: &BloggerPostsGetByPathArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Post, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = blogger_posts_get_by_path_builder(
            &self.http_client,
            &args.blogId,
            &args.maxComments,
            &args.path,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = blogger_posts_get_by_path_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Blogger posts insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Post result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn blogger_posts_insert(
        &self,
        args: &BloggerPostsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Post, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = blogger_posts_insert_builder(
            &self.http_client,
            &args.blogId,
            &args.fetchBody,
            &args.fetchImages,
            &args.isDraft,
        )
        .map_err(ProviderError::Api)?;

        let task = blogger_posts_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Blogger posts list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PostList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn blogger_posts_list(
        &self,
        args: &BloggerPostsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PostList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = blogger_posts_list_builder(
            &self.http_client,
            &args.blogId,
            &args.endDate,
            &args.fetchBodies,
            &args.fetchImages,
            &args.labels,
            &args.maxResults,
            &args.orderBy,
            &args.pageToken,
            &args.sortOption,
            &args.startDate,
            &args.status,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = blogger_posts_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Blogger posts patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Post result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn blogger_posts_patch(
        &self,
        args: &BloggerPostsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Post, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = blogger_posts_patch_builder(
            &self.http_client,
            &args.blogId,
            &args.postId,
            &args.fetchBody,
            &args.fetchImages,
            &args.maxComments,
            &args.publish,
            &args.revert,
        )
        .map_err(ProviderError::Api)?;

        let task = blogger_posts_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Blogger posts publish.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Post result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn blogger_posts_publish(
        &self,
        args: &BloggerPostsPublishArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Post, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = blogger_posts_publish_builder(
            &self.http_client,
            &args.blogId,
            &args.postId,
            &args.publishDate,
        )
        .map_err(ProviderError::Api)?;

        let task = blogger_posts_publish_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Blogger posts revert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Post result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn blogger_posts_revert(
        &self,
        args: &BloggerPostsRevertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Post, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = blogger_posts_revert_builder(
            &self.http_client,
            &args.blogId,
            &args.postId,
        )
        .map_err(ProviderError::Api)?;

        let task = blogger_posts_revert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Blogger posts search.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PostList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn blogger_posts_search(
        &self,
        args: &BloggerPostsSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PostList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = blogger_posts_search_builder(
            &self.http_client,
            &args.blogId,
            &args.fetchBodies,
            &args.orderBy,
            &args.q,
        )
        .map_err(ProviderError::Api)?;

        let task = blogger_posts_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Blogger posts update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Post result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn blogger_posts_update(
        &self,
        args: &BloggerPostsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Post, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = blogger_posts_update_builder(
            &self.http_client,
            &args.blogId,
            &args.postId,
            &args.fetchBody,
            &args.fetchImages,
            &args.maxComments,
            &args.publish,
            &args.revert,
        )
        .map_err(ProviderError::Api)?;

        let task = blogger_posts_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Blogger users get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the User result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn blogger_users_get(
        &self,
        args: &BloggerUsersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<User, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = blogger_users_get_builder(
            &self.http_client,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = blogger_users_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
