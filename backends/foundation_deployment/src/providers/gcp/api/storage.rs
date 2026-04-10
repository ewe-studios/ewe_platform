//! StorageProvider - State-aware storage API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       storage API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::storage::{
    storage_anywhere_caches_disable_builder, storage_anywhere_caches_disable_task,
    storage_anywhere_caches_insert_builder, storage_anywhere_caches_insert_task,
    storage_anywhere_caches_pause_builder, storage_anywhere_caches_pause_task,
    storage_anywhere_caches_resume_builder, storage_anywhere_caches_resume_task,
    storage_anywhere_caches_update_builder, storage_anywhere_caches_update_task,
    storage_bucket_access_controls_delete_builder, storage_bucket_access_controls_delete_task,
    storage_bucket_access_controls_insert_builder, storage_bucket_access_controls_insert_task,
    storage_bucket_access_controls_patch_builder, storage_bucket_access_controls_patch_task,
    storage_bucket_access_controls_update_builder, storage_bucket_access_controls_update_task,
    storage_buckets_delete_builder, storage_buckets_delete_task,
    storage_buckets_insert_builder, storage_buckets_insert_task,
    storage_buckets_lock_retention_policy_builder, storage_buckets_lock_retention_policy_task,
    storage_buckets_patch_builder, storage_buckets_patch_task,
    storage_buckets_relocate_builder, storage_buckets_relocate_task,
    storage_buckets_restore_builder, storage_buckets_restore_task,
    storage_buckets_set_iam_policy_builder, storage_buckets_set_iam_policy_task,
    storage_buckets_update_builder, storage_buckets_update_task,
    storage_channels_stop_builder, storage_channels_stop_task,
    storage_default_object_access_controls_delete_builder, storage_default_object_access_controls_delete_task,
    storage_default_object_access_controls_insert_builder, storage_default_object_access_controls_insert_task,
    storage_default_object_access_controls_patch_builder, storage_default_object_access_controls_patch_task,
    storage_default_object_access_controls_update_builder, storage_default_object_access_controls_update_task,
    storage_folders_delete_builder, storage_folders_delete_task,
    storage_folders_delete_recursive_builder, storage_folders_delete_recursive_task,
    storage_folders_insert_builder, storage_folders_insert_task,
    storage_folders_rename_builder, storage_folders_rename_task,
    storage_managed_folders_delete_builder, storage_managed_folders_delete_task,
    storage_managed_folders_insert_builder, storage_managed_folders_insert_task,
    storage_managed_folders_set_iam_policy_builder, storage_managed_folders_set_iam_policy_task,
    storage_notifications_delete_builder, storage_notifications_delete_task,
    storage_notifications_insert_builder, storage_notifications_insert_task,
    storage_object_access_controls_delete_builder, storage_object_access_controls_delete_task,
    storage_object_access_controls_insert_builder, storage_object_access_controls_insert_task,
    storage_object_access_controls_patch_builder, storage_object_access_controls_patch_task,
    storage_object_access_controls_update_builder, storage_object_access_controls_update_task,
    storage_objects_bulk_restore_builder, storage_objects_bulk_restore_task,
    storage_objects_compose_builder, storage_objects_compose_task,
    storage_objects_copy_builder, storage_objects_copy_task,
    storage_objects_delete_builder, storage_objects_delete_task,
    storage_objects_insert_builder, storage_objects_insert_task,
    storage_objects_move_builder, storage_objects_move_task,
    storage_objects_patch_builder, storage_objects_patch_task,
    storage_objects_restore_builder, storage_objects_restore_task,
    storage_objects_rewrite_builder, storage_objects_rewrite_task,
    storage_objects_set_iam_policy_builder, storage_objects_set_iam_policy_task,
    storage_objects_update_builder, storage_objects_update_task,
    storage_objects_watch_all_builder, storage_objects_watch_all_task,
    storage_buckets_operations_advance_relocate_bucket_builder, storage_buckets_operations_advance_relocate_bucket_task,
    storage_buckets_operations_cancel_builder, storage_buckets_operations_cancel_task,
    storage_projects_hmac_keys_create_builder, storage_projects_hmac_keys_create_task,
    storage_projects_hmac_keys_delete_builder, storage_projects_hmac_keys_delete_task,
    storage_projects_hmac_keys_update_builder, storage_projects_hmac_keys_update_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::storage::AnywhereCache;
use crate::providers::gcp::clients::storage::Bucket;
use crate::providers::gcp::clients::storage::BucketAccessControl;
use crate::providers::gcp::clients::storage::Channel;
use crate::providers::gcp::clients::storage::Folder;
use crate::providers::gcp::clients::storage::GoogleLongrunningOperation;
use crate::providers::gcp::clients::storage::HmacKey;
use crate::providers::gcp::clients::storage::HmacKeyMetadata;
use crate::providers::gcp::clients::storage::ManagedFolder;
use crate::providers::gcp::clients::storage::Notification;
use crate::providers::gcp::clients::storage::Object;
use crate::providers::gcp::clients::storage::ObjectAccessControl;
use crate::providers::gcp::clients::storage::Policy;
use crate::providers::gcp::clients::storage::RewriteResponse;
use crate::providers::gcp::clients::storage::StorageAnywhereCachesDisableArgs;
use crate::providers::gcp::clients::storage::StorageAnywhereCachesInsertArgs;
use crate::providers::gcp::clients::storage::StorageAnywhereCachesPauseArgs;
use crate::providers::gcp::clients::storage::StorageAnywhereCachesResumeArgs;
use crate::providers::gcp::clients::storage::StorageAnywhereCachesUpdateArgs;
use crate::providers::gcp::clients::storage::StorageBucketAccessControlsDeleteArgs;
use crate::providers::gcp::clients::storage::StorageBucketAccessControlsInsertArgs;
use crate::providers::gcp::clients::storage::StorageBucketAccessControlsPatchArgs;
use crate::providers::gcp::clients::storage::StorageBucketAccessControlsUpdateArgs;
use crate::providers::gcp::clients::storage::StorageBucketsDeleteArgs;
use crate::providers::gcp::clients::storage::StorageBucketsInsertArgs;
use crate::providers::gcp::clients::storage::StorageBucketsLockRetentionPolicyArgs;
use crate::providers::gcp::clients::storage::StorageBucketsOperationsAdvanceRelocateBucketArgs;
use crate::providers::gcp::clients::storage::StorageBucketsOperationsCancelArgs;
use crate::providers::gcp::clients::storage::StorageBucketsPatchArgs;
use crate::providers::gcp::clients::storage::StorageBucketsRelocateArgs;
use crate::providers::gcp::clients::storage::StorageBucketsRestoreArgs;
use crate::providers::gcp::clients::storage::StorageBucketsSetIamPolicyArgs;
use crate::providers::gcp::clients::storage::StorageBucketsUpdateArgs;
use crate::providers::gcp::clients::storage::StorageChannelsStopArgs;
use crate::providers::gcp::clients::storage::StorageDefaultObjectAccessControlsDeleteArgs;
use crate::providers::gcp::clients::storage::StorageDefaultObjectAccessControlsInsertArgs;
use crate::providers::gcp::clients::storage::StorageDefaultObjectAccessControlsPatchArgs;
use crate::providers::gcp::clients::storage::StorageDefaultObjectAccessControlsUpdateArgs;
use crate::providers::gcp::clients::storage::StorageFoldersDeleteArgs;
use crate::providers::gcp::clients::storage::StorageFoldersDeleteRecursiveArgs;
use crate::providers::gcp::clients::storage::StorageFoldersInsertArgs;
use crate::providers::gcp::clients::storage::StorageFoldersRenameArgs;
use crate::providers::gcp::clients::storage::StorageManagedFoldersDeleteArgs;
use crate::providers::gcp::clients::storage::StorageManagedFoldersInsertArgs;
use crate::providers::gcp::clients::storage::StorageManagedFoldersSetIamPolicyArgs;
use crate::providers::gcp::clients::storage::StorageNotificationsDeleteArgs;
use crate::providers::gcp::clients::storage::StorageNotificationsInsertArgs;
use crate::providers::gcp::clients::storage::StorageObjectAccessControlsDeleteArgs;
use crate::providers::gcp::clients::storage::StorageObjectAccessControlsInsertArgs;
use crate::providers::gcp::clients::storage::StorageObjectAccessControlsPatchArgs;
use crate::providers::gcp::clients::storage::StorageObjectAccessControlsUpdateArgs;
use crate::providers::gcp::clients::storage::StorageObjectsBulkRestoreArgs;
use crate::providers::gcp::clients::storage::StorageObjectsComposeArgs;
use crate::providers::gcp::clients::storage::StorageObjectsCopyArgs;
use crate::providers::gcp::clients::storage::StorageObjectsDeleteArgs;
use crate::providers::gcp::clients::storage::StorageObjectsInsertArgs;
use crate::providers::gcp::clients::storage::StorageObjectsMoveArgs;
use crate::providers::gcp::clients::storage::StorageObjectsPatchArgs;
use crate::providers::gcp::clients::storage::StorageObjectsRestoreArgs;
use crate::providers::gcp::clients::storage::StorageObjectsRewriteArgs;
use crate::providers::gcp::clients::storage::StorageObjectsSetIamPolicyArgs;
use crate::providers::gcp::clients::storage::StorageObjectsUpdateArgs;
use crate::providers::gcp::clients::storage::StorageObjectsWatchAllArgs;
use crate::providers::gcp::clients::storage::StorageProjectsHmacKeysCreateArgs;
use crate::providers::gcp::clients::storage::StorageProjectsHmacKeysDeleteArgs;
use crate::providers::gcp::clients::storage::StorageProjectsHmacKeysUpdateArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// StorageProvider with automatic state tracking.
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
/// let provider = StorageProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct StorageProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> StorageProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new StorageProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Storage anywhere caches disable.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AnywhereCache result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn storage_anywhere_caches_disable(
        &self,
        args: &StorageAnywhereCachesDisableArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AnywhereCache, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_anywhere_caches_disable_builder(
            &self.http_client,
            &args.bucket,
            &args.anywhereCacheId,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_anywhere_caches_disable_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage anywhere caches insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn storage_anywhere_caches_insert(
        &self,
        args: &StorageAnywhereCachesInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_anywhere_caches_insert_builder(
            &self.http_client,
            &args.bucket,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_anywhere_caches_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage anywhere caches pause.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AnywhereCache result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn storage_anywhere_caches_pause(
        &self,
        args: &StorageAnywhereCachesPauseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AnywhereCache, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_anywhere_caches_pause_builder(
            &self.http_client,
            &args.bucket,
            &args.anywhereCacheId,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_anywhere_caches_pause_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage anywhere caches resume.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AnywhereCache result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn storage_anywhere_caches_resume(
        &self,
        args: &StorageAnywhereCachesResumeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AnywhereCache, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_anywhere_caches_resume_builder(
            &self.http_client,
            &args.bucket,
            &args.anywhereCacheId,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_anywhere_caches_resume_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage anywhere caches update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn storage_anywhere_caches_update(
        &self,
        args: &StorageAnywhereCachesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_anywhere_caches_update_builder(
            &self.http_client,
            &args.bucket,
            &args.anywhereCacheId,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_anywhere_caches_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage bucket access controls delete.
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
    pub fn storage_bucket_access_controls_delete(
        &self,
        args: &StorageBucketAccessControlsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_bucket_access_controls_delete_builder(
            &self.http_client,
            &args.bucket,
            &args.entity,
            &args.userProject,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_bucket_access_controls_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage bucket access controls insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BucketAccessControl result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn storage_bucket_access_controls_insert(
        &self,
        args: &StorageBucketAccessControlsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BucketAccessControl, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_bucket_access_controls_insert_builder(
            &self.http_client,
            &args.bucket,
            &args.userProject,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_bucket_access_controls_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage bucket access controls patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BucketAccessControl result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn storage_bucket_access_controls_patch(
        &self,
        args: &StorageBucketAccessControlsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BucketAccessControl, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_bucket_access_controls_patch_builder(
            &self.http_client,
            &args.bucket,
            &args.entity,
            &args.userProject,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_bucket_access_controls_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage bucket access controls update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BucketAccessControl result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn storage_bucket_access_controls_update(
        &self,
        args: &StorageBucketAccessControlsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BucketAccessControl, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_bucket_access_controls_update_builder(
            &self.http_client,
            &args.bucket,
            &args.entity,
            &args.userProject,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_bucket_access_controls_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage buckets delete.
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
    pub fn storage_buckets_delete(
        &self,
        args: &StorageBucketsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_buckets_delete_builder(
            &self.http_client,
            &args.bucket,
            &args.ifMetagenerationMatch,
            &args.ifMetagenerationNotMatch,
            &args.userProject,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_buckets_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage buckets insert.
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
    pub fn storage_buckets_insert(
        &self,
        args: &StorageBucketsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Bucket, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_buckets_insert_builder(
            &self.http_client,
            &args.project,
            &args.enableObjectRetention,
            &args.predefinedAcl,
            &args.predefinedDefaultObjectAcl,
            &args.project,
            &args.projection,
            &args.userProject,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_buckets_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage buckets lock retention policy.
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
    pub fn storage_buckets_lock_retention_policy(
        &self,
        args: &StorageBucketsLockRetentionPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Bucket, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_buckets_lock_retention_policy_builder(
            &self.http_client,
            &args.bucket,
            &args.ifMetagenerationMatch,
            &args.ifMetagenerationMatch,
            &args.userProject,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_buckets_lock_retention_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage buckets patch.
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
    pub fn storage_buckets_patch(
        &self,
        args: &StorageBucketsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Bucket, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_buckets_patch_builder(
            &self.http_client,
            &args.bucket,
            &args.ifMetagenerationMatch,
            &args.ifMetagenerationNotMatch,
            &args.predefinedAcl,
            &args.predefinedDefaultObjectAcl,
            &args.projection,
            &args.userProject,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_buckets_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage buckets relocate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn storage_buckets_relocate(
        &self,
        args: &StorageBucketsRelocateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_buckets_relocate_builder(
            &self.http_client,
            &args.bucket,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_buckets_relocate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage buckets restore.
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
    pub fn storage_buckets_restore(
        &self,
        args: &StorageBucketsRestoreArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Bucket, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_buckets_restore_builder(
            &self.http_client,
            &args.bucket,
            &args.generation,
            &args.generation,
            &args.projection,
            &args.userProject,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_buckets_restore_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage buckets set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn storage_buckets_set_iam_policy(
        &self,
        args: &StorageBucketsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_buckets_set_iam_policy_builder(
            &self.http_client,
            &args.bucket,
            &args.userProject,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_buckets_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage buckets update.
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
    pub fn storage_buckets_update(
        &self,
        args: &StorageBucketsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Bucket, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_buckets_update_builder(
            &self.http_client,
            &args.bucket,
            &args.ifMetagenerationMatch,
            &args.ifMetagenerationNotMatch,
            &args.predefinedAcl,
            &args.predefinedDefaultObjectAcl,
            &args.projection,
            &args.userProject,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_buckets_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage channels stop.
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
    pub fn storage_channels_stop(
        &self,
        args: &StorageChannelsStopArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_channels_stop_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_channels_stop_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage default object access controls delete.
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
    pub fn storage_default_object_access_controls_delete(
        &self,
        args: &StorageDefaultObjectAccessControlsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_default_object_access_controls_delete_builder(
            &self.http_client,
            &args.bucket,
            &args.entity,
            &args.userProject,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_default_object_access_controls_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage default object access controls insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ObjectAccessControl result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn storage_default_object_access_controls_insert(
        &self,
        args: &StorageDefaultObjectAccessControlsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ObjectAccessControl, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_default_object_access_controls_insert_builder(
            &self.http_client,
            &args.bucket,
            &args.userProject,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_default_object_access_controls_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage default object access controls patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ObjectAccessControl result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn storage_default_object_access_controls_patch(
        &self,
        args: &StorageDefaultObjectAccessControlsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ObjectAccessControl, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_default_object_access_controls_patch_builder(
            &self.http_client,
            &args.bucket,
            &args.entity,
            &args.userProject,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_default_object_access_controls_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage default object access controls update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ObjectAccessControl result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn storage_default_object_access_controls_update(
        &self,
        args: &StorageDefaultObjectAccessControlsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ObjectAccessControl, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_default_object_access_controls_update_builder(
            &self.http_client,
            &args.bucket,
            &args.entity,
            &args.userProject,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_default_object_access_controls_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage folders delete.
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
    pub fn storage_folders_delete(
        &self,
        args: &StorageFoldersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_folders_delete_builder(
            &self.http_client,
            &args.bucket,
            &args.folder,
            &args.ifMetagenerationMatch,
            &args.ifMetagenerationNotMatch,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_folders_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage folders delete recursive.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn storage_folders_delete_recursive(
        &self,
        args: &StorageFoldersDeleteRecursiveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_folders_delete_recursive_builder(
            &self.http_client,
            &args.bucket,
            &args.folder,
            &args.ifMetagenerationMatch,
            &args.ifMetagenerationNotMatch,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_folders_delete_recursive_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage folders insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Folder result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn storage_folders_insert(
        &self,
        args: &StorageFoldersInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Folder, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_folders_insert_builder(
            &self.http_client,
            &args.bucket,
            &args.recursive,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_folders_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage folders rename.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn storage_folders_rename(
        &self,
        args: &StorageFoldersRenameArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_folders_rename_builder(
            &self.http_client,
            &args.bucket,
            &args.sourceFolder,
            &args.destinationFolder,
            &args.ifSourceMetagenerationMatch,
            &args.ifSourceMetagenerationNotMatch,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_folders_rename_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage managed folders delete.
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
    pub fn storage_managed_folders_delete(
        &self,
        args: &StorageManagedFoldersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_managed_folders_delete_builder(
            &self.http_client,
            &args.bucket,
            &args.managedFolder,
            &args.allowNonEmpty,
            &args.ifMetagenerationMatch,
            &args.ifMetagenerationNotMatch,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_managed_folders_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage managed folders insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ManagedFolder result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn storage_managed_folders_insert(
        &self,
        args: &StorageManagedFoldersInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ManagedFolder, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_managed_folders_insert_builder(
            &self.http_client,
            &args.bucket,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_managed_folders_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage managed folders set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn storage_managed_folders_set_iam_policy(
        &self,
        args: &StorageManagedFoldersSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_managed_folders_set_iam_policy_builder(
            &self.http_client,
            &args.bucket,
            &args.managedFolder,
            &args.userProject,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_managed_folders_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage notifications delete.
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
    pub fn storage_notifications_delete(
        &self,
        args: &StorageNotificationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_notifications_delete_builder(
            &self.http_client,
            &args.bucket,
            &args.notification,
            &args.userProject,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_notifications_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage notifications insert.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn storage_notifications_insert(
        &self,
        args: &StorageNotificationsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Notification, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_notifications_insert_builder(
            &self.http_client,
            &args.bucket,
            &args.userProject,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_notifications_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage object access controls delete.
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
    pub fn storage_object_access_controls_delete(
        &self,
        args: &StorageObjectAccessControlsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_object_access_controls_delete_builder(
            &self.http_client,
            &args.bucket,
            &args.object,
            &args.entity,
            &args.generation,
            &args.userProject,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_object_access_controls_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage object access controls insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ObjectAccessControl result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn storage_object_access_controls_insert(
        &self,
        args: &StorageObjectAccessControlsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ObjectAccessControl, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_object_access_controls_insert_builder(
            &self.http_client,
            &args.bucket,
            &args.object,
            &args.generation,
            &args.userProject,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_object_access_controls_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage object access controls patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ObjectAccessControl result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn storage_object_access_controls_patch(
        &self,
        args: &StorageObjectAccessControlsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ObjectAccessControl, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_object_access_controls_patch_builder(
            &self.http_client,
            &args.bucket,
            &args.object,
            &args.entity,
            &args.generation,
            &args.userProject,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_object_access_controls_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage object access controls update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ObjectAccessControl result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn storage_object_access_controls_update(
        &self,
        args: &StorageObjectAccessControlsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ObjectAccessControl, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_object_access_controls_update_builder(
            &self.http_client,
            &args.bucket,
            &args.object,
            &args.entity,
            &args.generation,
            &args.userProject,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_object_access_controls_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage objects bulk restore.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn storage_objects_bulk_restore(
        &self,
        args: &StorageObjectsBulkRestoreArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_objects_bulk_restore_builder(
            &self.http_client,
            &args.bucket,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_objects_bulk_restore_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage objects compose.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Object result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn storage_objects_compose(
        &self,
        args: &StorageObjectsComposeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Object, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_objects_compose_builder(
            &self.http_client,
            &args.destinationBucket,
            &args.destinationObject,
            &args.destinationPredefinedAcl,
            &args.dropContextGroups,
            &args.ifGenerationMatch,
            &args.ifMetagenerationMatch,
            &args.kmsKeyName,
            &args.userProject,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_objects_compose_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage objects copy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Object result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn storage_objects_copy(
        &self,
        args: &StorageObjectsCopyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Object, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_objects_copy_builder(
            &self.http_client,
            &args.sourceBucket,
            &args.sourceObject,
            &args.destinationBucket,
            &args.destinationObject,
            &args.destinationKmsKeyName,
            &args.destinationPredefinedAcl,
            &args.ifGenerationMatch,
            &args.ifGenerationNotMatch,
            &args.ifMetagenerationMatch,
            &args.ifMetagenerationNotMatch,
            &args.ifSourceGenerationMatch,
            &args.ifSourceGenerationNotMatch,
            &args.ifSourceMetagenerationMatch,
            &args.ifSourceMetagenerationNotMatch,
            &args.projection,
            &args.sourceGeneration,
            &args.userProject,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_objects_copy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage objects delete.
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
    pub fn storage_objects_delete(
        &self,
        args: &StorageObjectsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_objects_delete_builder(
            &self.http_client,
            &args.bucket,
            &args.object,
            &args.generation,
            &args.ifGenerationMatch,
            &args.ifGenerationNotMatch,
            &args.ifMetagenerationMatch,
            &args.ifMetagenerationNotMatch,
            &args.userProject,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_objects_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage objects insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Object result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn storage_objects_insert(
        &self,
        args: &StorageObjectsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Object, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_objects_insert_builder(
            &self.http_client,
            &args.bucket,
            &args.contentEncoding,
            &args.ifGenerationMatch,
            &args.ifGenerationNotMatch,
            &args.ifMetagenerationMatch,
            &args.ifMetagenerationNotMatch,
            &args.kmsKeyName,
            &args.name,
            &args.predefinedAcl,
            &args.projection,
            &args.userProject,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_objects_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage objects move.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Object result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn storage_objects_move(
        &self,
        args: &StorageObjectsMoveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Object, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_objects_move_builder(
            &self.http_client,
            &args.bucket,
            &args.sourceObject,
            &args.destinationObject,
            &args.ifGenerationMatch,
            &args.ifGenerationNotMatch,
            &args.ifMetagenerationMatch,
            &args.ifMetagenerationNotMatch,
            &args.ifSourceGenerationMatch,
            &args.ifSourceGenerationNotMatch,
            &args.ifSourceMetagenerationMatch,
            &args.ifSourceMetagenerationNotMatch,
            &args.projection,
            &args.userProject,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_objects_move_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage objects patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Object result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn storage_objects_patch(
        &self,
        args: &StorageObjectsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Object, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_objects_patch_builder(
            &self.http_client,
            &args.bucket,
            &args.object,
            &args.generation,
            &args.ifGenerationMatch,
            &args.ifGenerationNotMatch,
            &args.ifMetagenerationMatch,
            &args.ifMetagenerationNotMatch,
            &args.overrideUnlockedRetention,
            &args.predefinedAcl,
            &args.projection,
            &args.userProject,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_objects_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage objects restore.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Object result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn storage_objects_restore(
        &self,
        args: &StorageObjectsRestoreArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Object, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_objects_restore_builder(
            &self.http_client,
            &args.bucket,
            &args.object,
            &args.generation,
            &args.copySourceAcl,
            &args.generation,
            &args.ifGenerationMatch,
            &args.ifGenerationNotMatch,
            &args.ifMetagenerationMatch,
            &args.ifMetagenerationNotMatch,
            &args.projection,
            &args.restoreToken,
            &args.userProject,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_objects_restore_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage objects rewrite.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RewriteResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn storage_objects_rewrite(
        &self,
        args: &StorageObjectsRewriteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RewriteResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_objects_rewrite_builder(
            &self.http_client,
            &args.sourceBucket,
            &args.sourceObject,
            &args.destinationBucket,
            &args.destinationObject,
            &args.destinationKmsKeyName,
            &args.destinationPredefinedAcl,
            &args.dropContextGroups,
            &args.ifGenerationMatch,
            &args.ifGenerationNotMatch,
            &args.ifMetagenerationMatch,
            &args.ifMetagenerationNotMatch,
            &args.ifSourceGenerationMatch,
            &args.ifSourceGenerationNotMatch,
            &args.ifSourceMetagenerationMatch,
            &args.ifSourceMetagenerationNotMatch,
            &args.maxBytesRewrittenPerCall,
            &args.projection,
            &args.rewriteToken,
            &args.sourceGeneration,
            &args.userProject,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_objects_rewrite_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage objects set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn storage_objects_set_iam_policy(
        &self,
        args: &StorageObjectsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_objects_set_iam_policy_builder(
            &self.http_client,
            &args.bucket,
            &args.object,
            &args.generation,
            &args.userProject,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_objects_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage objects update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Object result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn storage_objects_update(
        &self,
        args: &StorageObjectsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Object, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_objects_update_builder(
            &self.http_client,
            &args.bucket,
            &args.object,
            &args.generation,
            &args.ifGenerationMatch,
            &args.ifGenerationNotMatch,
            &args.ifMetagenerationMatch,
            &args.ifMetagenerationNotMatch,
            &args.overrideUnlockedRetention,
            &args.predefinedAcl,
            &args.projection,
            &args.userProject,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_objects_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage objects watch all.
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
    pub fn storage_objects_watch_all(
        &self,
        args: &StorageObjectsWatchAllArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Channel, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_objects_watch_all_builder(
            &self.http_client,
            &args.bucket,
            &args.delimiter,
            &args.endOffset,
            &args.includeTrailingDelimiter,
            &args.maxResults,
            &args.pageToken,
            &args.prefix,
            &args.projection,
            &args.startOffset,
            &args.userProject,
            &args.versions,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_objects_watch_all_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage buckets operations advance relocate bucket.
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
    pub fn storage_buckets_operations_advance_relocate_bucket(
        &self,
        args: &StorageBucketsOperationsAdvanceRelocateBucketArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_buckets_operations_advance_relocate_bucket_builder(
            &self.http_client,
            &args.bucket,
            &args.operationId,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_buckets_operations_advance_relocate_bucket_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage buckets operations cancel.
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
    pub fn storage_buckets_operations_cancel(
        &self,
        args: &StorageBucketsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_buckets_operations_cancel_builder(
            &self.http_client,
            &args.bucket,
            &args.operationId,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_buckets_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage projects hmac keys create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HmacKey result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn storage_projects_hmac_keys_create(
        &self,
        args: &StorageProjectsHmacKeysCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HmacKey, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_projects_hmac_keys_create_builder(
            &self.http_client,
            &args.projectId,
            &args.serviceAccountEmail,
            &args.serviceAccountEmail,
            &args.userProject,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_projects_hmac_keys_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage projects hmac keys delete.
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
    pub fn storage_projects_hmac_keys_delete(
        &self,
        args: &StorageProjectsHmacKeysDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_projects_hmac_keys_delete_builder(
            &self.http_client,
            &args.projectId,
            &args.accessId,
            &args.userProject,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_projects_hmac_keys_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storage projects hmac keys update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HmacKeyMetadata result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn storage_projects_hmac_keys_update(
        &self,
        args: &StorageProjectsHmacKeysUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HmacKeyMetadata, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storage_projects_hmac_keys_update_builder(
            &self.http_client,
            &args.projectId,
            &args.accessId,
            &args.userProject,
        )
        .map_err(ProviderError::Api)?;

        let task = storage_projects_hmac_keys_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
