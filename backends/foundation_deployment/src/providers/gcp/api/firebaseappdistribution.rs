//! FirebaseappdistributionProvider - State-aware firebaseappdistribution API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       firebaseappdistribution API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::firebaseappdistribution::{
    firebaseappdistribution_media_upload_builder, firebaseappdistribution_media_upload_task,
    firebaseappdistribution_projects_apps_releases_batch_delete_builder, firebaseappdistribution_projects_apps_releases_batch_delete_task,
    firebaseappdistribution_projects_apps_releases_distribute_builder, firebaseappdistribution_projects_apps_releases_distribute_task,
    firebaseappdistribution_projects_apps_releases_patch_builder, firebaseappdistribution_projects_apps_releases_patch_task,
    firebaseappdistribution_projects_apps_releases_feedback_reports_delete_builder, firebaseappdistribution_projects_apps_releases_feedback_reports_delete_task,
    firebaseappdistribution_projects_apps_releases_operations_cancel_builder, firebaseappdistribution_projects_apps_releases_operations_cancel_task,
    firebaseappdistribution_projects_apps_releases_operations_delete_builder, firebaseappdistribution_projects_apps_releases_operations_delete_task,
    firebaseappdistribution_projects_apps_releases_operations_wait_builder, firebaseappdistribution_projects_apps_releases_operations_wait_task,
    firebaseappdistribution_projects_groups_batch_join_builder, firebaseappdistribution_projects_groups_batch_join_task,
    firebaseappdistribution_projects_groups_batch_leave_builder, firebaseappdistribution_projects_groups_batch_leave_task,
    firebaseappdistribution_projects_groups_create_builder, firebaseappdistribution_projects_groups_create_task,
    firebaseappdistribution_projects_groups_delete_builder, firebaseappdistribution_projects_groups_delete_task,
    firebaseappdistribution_projects_groups_patch_builder, firebaseappdistribution_projects_groups_patch_task,
    firebaseappdistribution_projects_testers_batch_add_builder, firebaseappdistribution_projects_testers_batch_add_task,
    firebaseappdistribution_projects_testers_batch_remove_builder, firebaseappdistribution_projects_testers_batch_remove_task,
    firebaseappdistribution_projects_testers_patch_builder, firebaseappdistribution_projects_testers_patch_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::firebaseappdistribution::GoogleFirebaseAppdistroV1BatchAddTestersResponse;
use crate::providers::gcp::clients::firebaseappdistribution::GoogleFirebaseAppdistroV1BatchRemoveTestersResponse;
use crate::providers::gcp::clients::firebaseappdistribution::GoogleFirebaseAppdistroV1DistributeReleaseResponse;
use crate::providers::gcp::clients::firebaseappdistribution::GoogleFirebaseAppdistroV1Group;
use crate::providers::gcp::clients::firebaseappdistribution::GoogleFirebaseAppdistroV1Release;
use crate::providers::gcp::clients::firebaseappdistribution::GoogleFirebaseAppdistroV1Tester;
use crate::providers::gcp::clients::firebaseappdistribution::GoogleLongrunningOperation;
use crate::providers::gcp::clients::firebaseappdistribution::GoogleProtobufEmpty;
use crate::providers::gcp::clients::firebaseappdistribution::FirebaseappdistributionMediaUploadArgs;
use crate::providers::gcp::clients::firebaseappdistribution::FirebaseappdistributionProjectsAppsReleasesBatchDeleteArgs;
use crate::providers::gcp::clients::firebaseappdistribution::FirebaseappdistributionProjectsAppsReleasesDistributeArgs;
use crate::providers::gcp::clients::firebaseappdistribution::FirebaseappdistributionProjectsAppsReleasesFeedbackReportsDeleteArgs;
use crate::providers::gcp::clients::firebaseappdistribution::FirebaseappdistributionProjectsAppsReleasesOperationsCancelArgs;
use crate::providers::gcp::clients::firebaseappdistribution::FirebaseappdistributionProjectsAppsReleasesOperationsDeleteArgs;
use crate::providers::gcp::clients::firebaseappdistribution::FirebaseappdistributionProjectsAppsReleasesOperationsWaitArgs;
use crate::providers::gcp::clients::firebaseappdistribution::FirebaseappdistributionProjectsAppsReleasesPatchArgs;
use crate::providers::gcp::clients::firebaseappdistribution::FirebaseappdistributionProjectsGroupsBatchJoinArgs;
use crate::providers::gcp::clients::firebaseappdistribution::FirebaseappdistributionProjectsGroupsBatchLeaveArgs;
use crate::providers::gcp::clients::firebaseappdistribution::FirebaseappdistributionProjectsGroupsCreateArgs;
use crate::providers::gcp::clients::firebaseappdistribution::FirebaseappdistributionProjectsGroupsDeleteArgs;
use crate::providers::gcp::clients::firebaseappdistribution::FirebaseappdistributionProjectsGroupsPatchArgs;
use crate::providers::gcp::clients::firebaseappdistribution::FirebaseappdistributionProjectsTestersBatchAddArgs;
use crate::providers::gcp::clients::firebaseappdistribution::FirebaseappdistributionProjectsTestersBatchRemoveArgs;
use crate::providers::gcp::clients::firebaseappdistribution::FirebaseappdistributionProjectsTestersPatchArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// FirebaseappdistributionProvider with automatic state tracking.
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
/// let provider = FirebaseappdistributionProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct FirebaseappdistributionProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> FirebaseappdistributionProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new FirebaseappdistributionProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Firebaseappdistribution media upload.
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
    pub fn firebaseappdistribution_media_upload(
        &self,
        args: &FirebaseappdistributionMediaUploadArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseappdistribution_media_upload_builder(
            &self.http_client,
            &args.app,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseappdistribution_media_upload_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseappdistribution projects apps releases batch delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebaseappdistribution_projects_apps_releases_batch_delete(
        &self,
        args: &FirebaseappdistributionProjectsAppsReleasesBatchDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseappdistribution_projects_apps_releases_batch_delete_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseappdistribution_projects_apps_releases_batch_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseappdistribution projects apps releases distribute.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirebaseAppdistroV1DistributeReleaseResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebaseappdistribution_projects_apps_releases_distribute(
        &self,
        args: &FirebaseappdistributionProjectsAppsReleasesDistributeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirebaseAppdistroV1DistributeReleaseResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseappdistribution_projects_apps_releases_distribute_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseappdistribution_projects_apps_releases_distribute_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseappdistribution projects apps releases patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirebaseAppdistroV1Release result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebaseappdistribution_projects_apps_releases_patch(
        &self,
        args: &FirebaseappdistributionProjectsAppsReleasesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirebaseAppdistroV1Release, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseappdistribution_projects_apps_releases_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseappdistribution_projects_apps_releases_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseappdistribution projects apps releases feedback reports delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebaseappdistribution_projects_apps_releases_feedback_reports_delete(
        &self,
        args: &FirebaseappdistributionProjectsAppsReleasesFeedbackReportsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseappdistribution_projects_apps_releases_feedback_reports_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseappdistribution_projects_apps_releases_feedback_reports_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseappdistribution projects apps releases operations cancel.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebaseappdistribution_projects_apps_releases_operations_cancel(
        &self,
        args: &FirebaseappdistributionProjectsAppsReleasesOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseappdistribution_projects_apps_releases_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseappdistribution_projects_apps_releases_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseappdistribution projects apps releases operations delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebaseappdistribution_projects_apps_releases_operations_delete(
        &self,
        args: &FirebaseappdistributionProjectsAppsReleasesOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseappdistribution_projects_apps_releases_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseappdistribution_projects_apps_releases_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseappdistribution projects apps releases operations wait.
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
    pub fn firebaseappdistribution_projects_apps_releases_operations_wait(
        &self,
        args: &FirebaseappdistributionProjectsAppsReleasesOperationsWaitArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseappdistribution_projects_apps_releases_operations_wait_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseappdistribution_projects_apps_releases_operations_wait_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseappdistribution projects groups batch join.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebaseappdistribution_projects_groups_batch_join(
        &self,
        args: &FirebaseappdistributionProjectsGroupsBatchJoinArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseappdistribution_projects_groups_batch_join_builder(
            &self.http_client,
            &args.group,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseappdistribution_projects_groups_batch_join_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseappdistribution projects groups batch leave.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebaseappdistribution_projects_groups_batch_leave(
        &self,
        args: &FirebaseappdistributionProjectsGroupsBatchLeaveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseappdistribution_projects_groups_batch_leave_builder(
            &self.http_client,
            &args.group,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseappdistribution_projects_groups_batch_leave_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseappdistribution projects groups create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirebaseAppdistroV1Group result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebaseappdistribution_projects_groups_create(
        &self,
        args: &FirebaseappdistributionProjectsGroupsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirebaseAppdistroV1Group, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseappdistribution_projects_groups_create_builder(
            &self.http_client,
            &args.parent,
            &args.groupId,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseappdistribution_projects_groups_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseappdistribution projects groups delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebaseappdistribution_projects_groups_delete(
        &self,
        args: &FirebaseappdistributionProjectsGroupsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseappdistribution_projects_groups_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseappdistribution_projects_groups_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseappdistribution projects groups patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirebaseAppdistroV1Group result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebaseappdistribution_projects_groups_patch(
        &self,
        args: &FirebaseappdistributionProjectsGroupsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirebaseAppdistroV1Group, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseappdistribution_projects_groups_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseappdistribution_projects_groups_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseappdistribution projects testers batch add.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirebaseAppdistroV1BatchAddTestersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebaseappdistribution_projects_testers_batch_add(
        &self,
        args: &FirebaseappdistributionProjectsTestersBatchAddArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirebaseAppdistroV1BatchAddTestersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseappdistribution_projects_testers_batch_add_builder(
            &self.http_client,
            &args.project,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseappdistribution_projects_testers_batch_add_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseappdistribution projects testers batch remove.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirebaseAppdistroV1BatchRemoveTestersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebaseappdistribution_projects_testers_batch_remove(
        &self,
        args: &FirebaseappdistributionProjectsTestersBatchRemoveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirebaseAppdistroV1BatchRemoveTestersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseappdistribution_projects_testers_batch_remove_builder(
            &self.http_client,
            &args.project,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseappdistribution_projects_testers_batch_remove_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseappdistribution projects testers patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirebaseAppdistroV1Tester result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebaseappdistribution_projects_testers_patch(
        &self,
        args: &FirebaseappdistributionProjectsTestersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirebaseAppdistroV1Tester, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseappdistribution_projects_testers_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseappdistribution_projects_testers_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
