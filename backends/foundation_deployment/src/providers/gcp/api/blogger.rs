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
    blogger_comments_approve_builder, blogger_comments_approve_task,
    blogger_comments_delete_builder, blogger_comments_delete_task,
    blogger_comments_mark_as_spam_builder, blogger_comments_mark_as_spam_task,
    blogger_comments_remove_content_builder, blogger_comments_remove_content_task,
    blogger_pages_delete_builder, blogger_pages_delete_task,
    blogger_pages_insert_builder, blogger_pages_insert_task,
    blogger_pages_patch_builder, blogger_pages_patch_task,
    blogger_pages_publish_builder, blogger_pages_publish_task,
    blogger_pages_revert_builder, blogger_pages_revert_task,
    blogger_pages_update_builder, blogger_pages_update_task,
    blogger_posts_delete_builder, blogger_posts_delete_task,
    blogger_posts_insert_builder, blogger_posts_insert_task,
    blogger_posts_patch_builder, blogger_posts_patch_task,
    blogger_posts_publish_builder, blogger_posts_publish_task,
    blogger_posts_revert_builder, blogger_posts_revert_task,
    blogger_posts_update_builder, blogger_posts_update_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::blogger::Comment;
use crate::providers::gcp::clients::blogger::Page;
use crate::providers::gcp::clients::blogger::Post;
use crate::providers::gcp::clients::blogger::BloggerCommentsApproveArgs;
use crate::providers::gcp::clients::blogger::BloggerCommentsDeleteArgs;
use crate::providers::gcp::clients::blogger::BloggerCommentsMarkAsSpamArgs;
use crate::providers::gcp::clients::blogger::BloggerCommentsRemoveContentArgs;
use crate::providers::gcp::clients::blogger::BloggerPagesDeleteArgs;
use crate::providers::gcp::clients::blogger::BloggerPagesInsertArgs;
use crate::providers::gcp::clients::blogger::BloggerPagesPatchArgs;
use crate::providers::gcp::clients::blogger::BloggerPagesPublishArgs;
use crate::providers::gcp::clients::blogger::BloggerPagesRevertArgs;
use crate::providers::gcp::clients::blogger::BloggerPagesUpdateArgs;
use crate::providers::gcp::clients::blogger::BloggerPostsDeleteArgs;
use crate::providers::gcp::clients::blogger::BloggerPostsInsertArgs;
use crate::providers::gcp::clients::blogger::BloggerPostsPatchArgs;
use crate::providers::gcp::clients::blogger::BloggerPostsPublishArgs;
use crate::providers::gcp::clients::blogger::BloggerPostsRevertArgs;
use crate::providers::gcp::clients::blogger::BloggerPostsUpdateArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// BloggerProvider with automatic state tracking.
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
/// let provider = BloggerProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct BloggerProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> BloggerProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new BloggerProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
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

}
