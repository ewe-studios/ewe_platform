//! FirebasestorageProvider - State-aware firebasestorage API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       firebasestorage API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::firebasestorage::{
    firebasestorage_projects_delete_default_bucket_builder, firebasestorage_projects_delete_default_bucket_task,
    firebasestorage_projects_buckets_add_firebase_builder, firebasestorage_projects_buckets_add_firebase_task,
    firebasestorage_projects_buckets_remove_firebase_builder, firebasestorage_projects_buckets_remove_firebase_task,
    firebasestorage_projects_default_bucket_create_builder, firebasestorage_projects_default_bucket_create_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::firebasestorage::Bucket;
use crate::providers::gcp::clients::firebasestorage::DefaultBucket;
use crate::providers::gcp::clients::firebasestorage::Empty;
use crate::providers::gcp::clients::firebasestorage::FirebasestorageProjectsBucketsAddFirebaseArgs;
use crate::providers::gcp::clients::firebasestorage::FirebasestorageProjectsBucketsRemoveFirebaseArgs;
use crate::providers::gcp::clients::firebasestorage::FirebasestorageProjectsDefaultBucketCreateArgs;
use crate::providers::gcp::clients::firebasestorage::FirebasestorageProjectsDeleteDefaultBucketArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// FirebasestorageProvider with automatic state tracking.
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
/// let provider = FirebasestorageProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct FirebasestorageProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> FirebasestorageProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new FirebasestorageProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Firebasestorage projects delete default bucket.
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
    pub fn firebasestorage_projects_delete_default_bucket(
        &self,
        args: &FirebasestorageProjectsDeleteDefaultBucketArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebasestorage_projects_delete_default_bucket_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebasestorage_projects_delete_default_bucket_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebasestorage projects buckets add firebase.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Bucket result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebasestorage_projects_buckets_add_firebase(
        &self,
        args: &FirebasestorageProjectsBucketsAddFirebaseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Bucket, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebasestorage_projects_buckets_add_firebase_builder(
            &self.http_client,
            &args.bucket,
        )
        .map_err(ProviderError::Api)?;

        let task = firebasestorage_projects_buckets_add_firebase_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebasestorage projects buckets remove firebase.
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
    pub fn firebasestorage_projects_buckets_remove_firebase(
        &self,
        args: &FirebasestorageProjectsBucketsRemoveFirebaseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebasestorage_projects_buckets_remove_firebase_builder(
            &self.http_client,
            &args.bucket,
        )
        .map_err(ProviderError::Api)?;

        let task = firebasestorage_projects_buckets_remove_firebase_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebasestorage projects default bucket create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DefaultBucket result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebasestorage_projects_default_bucket_create(
        &self,
        args: &FirebasestorageProjectsDefaultBucketCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DefaultBucket, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebasestorage_projects_default_bucket_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = firebasestorage_projects_default_bucket_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
