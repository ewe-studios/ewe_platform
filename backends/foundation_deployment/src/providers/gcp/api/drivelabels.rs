//! DrivelabelsProvider - State-aware drivelabels API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       drivelabels API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::drivelabels::{
    drivelabels_labels_create_builder, drivelabels_labels_create_task,
    drivelabels_labels_delete_builder, drivelabels_labels_delete_task,
    drivelabels_labels_delta_builder, drivelabels_labels_delta_task,
    drivelabels_labels_disable_builder, drivelabels_labels_disable_task,
    drivelabels_labels_enable_builder, drivelabels_labels_enable_task,
    drivelabels_labels_publish_builder, drivelabels_labels_publish_task,
    drivelabels_labels_update_label_copy_mode_builder, drivelabels_labels_update_label_copy_mode_task,
    drivelabels_labels_update_label_enabled_app_settings_builder, drivelabels_labels_update_label_enabled_app_settings_task,
    drivelabels_labels_update_permissions_builder, drivelabels_labels_update_permissions_task,
    drivelabels_labels_permissions_batch_delete_builder, drivelabels_labels_permissions_batch_delete_task,
    drivelabels_labels_permissions_batch_update_builder, drivelabels_labels_permissions_batch_update_task,
    drivelabels_labels_permissions_create_builder, drivelabels_labels_permissions_create_task,
    drivelabels_labels_permissions_delete_builder, drivelabels_labels_permissions_delete_task,
    drivelabels_labels_revisions_update_permissions_builder, drivelabels_labels_revisions_update_permissions_task,
    drivelabels_labels_revisions_permissions_batch_delete_builder, drivelabels_labels_revisions_permissions_batch_delete_task,
    drivelabels_labels_revisions_permissions_batch_update_builder, drivelabels_labels_revisions_permissions_batch_update_task,
    drivelabels_labels_revisions_permissions_create_builder, drivelabels_labels_revisions_permissions_create_task,
    drivelabels_labels_revisions_permissions_delete_builder, drivelabels_labels_revisions_permissions_delete_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::drivelabels::GoogleAppsDriveLabelsV2BatchUpdateLabelPermissionsResponse;
use crate::providers::gcp::clients::drivelabels::GoogleAppsDriveLabelsV2DeltaUpdateLabelResponse;
use crate::providers::gcp::clients::drivelabels::GoogleAppsDriveLabelsV2Label;
use crate::providers::gcp::clients::drivelabels::GoogleAppsDriveLabelsV2LabelPermission;
use crate::providers::gcp::clients::drivelabels::GoogleProtobufEmpty;
use crate::providers::gcp::clients::drivelabels::DrivelabelsLabelsCreateArgs;
use crate::providers::gcp::clients::drivelabels::DrivelabelsLabelsDeleteArgs;
use crate::providers::gcp::clients::drivelabels::DrivelabelsLabelsDeltaArgs;
use crate::providers::gcp::clients::drivelabels::DrivelabelsLabelsDisableArgs;
use crate::providers::gcp::clients::drivelabels::DrivelabelsLabelsEnableArgs;
use crate::providers::gcp::clients::drivelabels::DrivelabelsLabelsPermissionsBatchDeleteArgs;
use crate::providers::gcp::clients::drivelabels::DrivelabelsLabelsPermissionsBatchUpdateArgs;
use crate::providers::gcp::clients::drivelabels::DrivelabelsLabelsPermissionsCreateArgs;
use crate::providers::gcp::clients::drivelabels::DrivelabelsLabelsPermissionsDeleteArgs;
use crate::providers::gcp::clients::drivelabels::DrivelabelsLabelsPublishArgs;
use crate::providers::gcp::clients::drivelabels::DrivelabelsLabelsRevisionsPermissionsBatchDeleteArgs;
use crate::providers::gcp::clients::drivelabels::DrivelabelsLabelsRevisionsPermissionsBatchUpdateArgs;
use crate::providers::gcp::clients::drivelabels::DrivelabelsLabelsRevisionsPermissionsCreateArgs;
use crate::providers::gcp::clients::drivelabels::DrivelabelsLabelsRevisionsPermissionsDeleteArgs;
use crate::providers::gcp::clients::drivelabels::DrivelabelsLabelsRevisionsUpdatePermissionsArgs;
use crate::providers::gcp::clients::drivelabels::DrivelabelsLabelsUpdateLabelCopyModeArgs;
use crate::providers::gcp::clients::drivelabels::DrivelabelsLabelsUpdateLabelEnabledAppSettingsArgs;
use crate::providers::gcp::clients::drivelabels::DrivelabelsLabelsUpdatePermissionsArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// DrivelabelsProvider with automatic state tracking.
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
/// let provider = DrivelabelsProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct DrivelabelsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> DrivelabelsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new DrivelabelsProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Drivelabels labels create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAppsDriveLabelsV2Label result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn drivelabels_labels_create(
        &self,
        args: &DrivelabelsLabelsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAppsDriveLabelsV2Label, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drivelabels_labels_create_builder(
            &self.http_client,
            &args.languageCode,
            &args.useAdminAccess,
        )
        .map_err(ProviderError::Api)?;

        let task = drivelabels_labels_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drivelabels labels delete.
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
    pub fn drivelabels_labels_delete(
        &self,
        args: &DrivelabelsLabelsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drivelabels_labels_delete_builder(
            &self.http_client,
            &args.name,
            &args.useAdminAccess,
            &args.writeControl.requiredRevisionId,
        )
        .map_err(ProviderError::Api)?;

        let task = drivelabels_labels_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drivelabels labels delta.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAppsDriveLabelsV2DeltaUpdateLabelResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn drivelabels_labels_delta(
        &self,
        args: &DrivelabelsLabelsDeltaArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAppsDriveLabelsV2DeltaUpdateLabelResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drivelabels_labels_delta_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = drivelabels_labels_delta_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drivelabels labels disable.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAppsDriveLabelsV2Label result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn drivelabels_labels_disable(
        &self,
        args: &DrivelabelsLabelsDisableArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAppsDriveLabelsV2Label, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drivelabels_labels_disable_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = drivelabels_labels_disable_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drivelabels labels enable.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAppsDriveLabelsV2Label result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn drivelabels_labels_enable(
        &self,
        args: &DrivelabelsLabelsEnableArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAppsDriveLabelsV2Label, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drivelabels_labels_enable_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = drivelabels_labels_enable_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drivelabels labels publish.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAppsDriveLabelsV2Label result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn drivelabels_labels_publish(
        &self,
        args: &DrivelabelsLabelsPublishArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAppsDriveLabelsV2Label, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drivelabels_labels_publish_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = drivelabels_labels_publish_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drivelabels labels update label copy mode.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAppsDriveLabelsV2Label result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn drivelabels_labels_update_label_copy_mode(
        &self,
        args: &DrivelabelsLabelsUpdateLabelCopyModeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAppsDriveLabelsV2Label, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drivelabels_labels_update_label_copy_mode_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = drivelabels_labels_update_label_copy_mode_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drivelabels labels update label enabled app settings.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAppsDriveLabelsV2Label result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn drivelabels_labels_update_label_enabled_app_settings(
        &self,
        args: &DrivelabelsLabelsUpdateLabelEnabledAppSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAppsDriveLabelsV2Label, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drivelabels_labels_update_label_enabled_app_settings_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = drivelabels_labels_update_label_enabled_app_settings_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drivelabels labels update permissions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAppsDriveLabelsV2LabelPermission result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn drivelabels_labels_update_permissions(
        &self,
        args: &DrivelabelsLabelsUpdatePermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAppsDriveLabelsV2LabelPermission, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drivelabels_labels_update_permissions_builder(
            &self.http_client,
            &args.parent,
            &args.useAdminAccess,
        )
        .map_err(ProviderError::Api)?;

        let task = drivelabels_labels_update_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drivelabels labels permissions batch delete.
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
    pub fn drivelabels_labels_permissions_batch_delete(
        &self,
        args: &DrivelabelsLabelsPermissionsBatchDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drivelabels_labels_permissions_batch_delete_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = drivelabels_labels_permissions_batch_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drivelabels labels permissions batch update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAppsDriveLabelsV2BatchUpdateLabelPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn drivelabels_labels_permissions_batch_update(
        &self,
        args: &DrivelabelsLabelsPermissionsBatchUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAppsDriveLabelsV2BatchUpdateLabelPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drivelabels_labels_permissions_batch_update_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = drivelabels_labels_permissions_batch_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drivelabels labels permissions create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAppsDriveLabelsV2LabelPermission result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn drivelabels_labels_permissions_create(
        &self,
        args: &DrivelabelsLabelsPermissionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAppsDriveLabelsV2LabelPermission, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drivelabels_labels_permissions_create_builder(
            &self.http_client,
            &args.parent,
            &args.useAdminAccess,
        )
        .map_err(ProviderError::Api)?;

        let task = drivelabels_labels_permissions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drivelabels labels permissions delete.
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
    pub fn drivelabels_labels_permissions_delete(
        &self,
        args: &DrivelabelsLabelsPermissionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drivelabels_labels_permissions_delete_builder(
            &self.http_client,
            &args.name,
            &args.useAdminAccess,
        )
        .map_err(ProviderError::Api)?;

        let task = drivelabels_labels_permissions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drivelabels labels revisions update permissions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAppsDriveLabelsV2LabelPermission result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn drivelabels_labels_revisions_update_permissions(
        &self,
        args: &DrivelabelsLabelsRevisionsUpdatePermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAppsDriveLabelsV2LabelPermission, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drivelabels_labels_revisions_update_permissions_builder(
            &self.http_client,
            &args.parent,
            &args.useAdminAccess,
        )
        .map_err(ProviderError::Api)?;

        let task = drivelabels_labels_revisions_update_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drivelabels labels revisions permissions batch delete.
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
    pub fn drivelabels_labels_revisions_permissions_batch_delete(
        &self,
        args: &DrivelabelsLabelsRevisionsPermissionsBatchDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drivelabels_labels_revisions_permissions_batch_delete_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = drivelabels_labels_revisions_permissions_batch_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drivelabels labels revisions permissions batch update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAppsDriveLabelsV2BatchUpdateLabelPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn drivelabels_labels_revisions_permissions_batch_update(
        &self,
        args: &DrivelabelsLabelsRevisionsPermissionsBatchUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAppsDriveLabelsV2BatchUpdateLabelPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drivelabels_labels_revisions_permissions_batch_update_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = drivelabels_labels_revisions_permissions_batch_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drivelabels labels revisions permissions create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAppsDriveLabelsV2LabelPermission result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn drivelabels_labels_revisions_permissions_create(
        &self,
        args: &DrivelabelsLabelsRevisionsPermissionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAppsDriveLabelsV2LabelPermission, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drivelabels_labels_revisions_permissions_create_builder(
            &self.http_client,
            &args.parent,
            &args.useAdminAccess,
        )
        .map_err(ProviderError::Api)?;

        let task = drivelabels_labels_revisions_permissions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drivelabels labels revisions permissions delete.
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
    pub fn drivelabels_labels_revisions_permissions_delete(
        &self,
        args: &DrivelabelsLabelsRevisionsPermissionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drivelabels_labels_revisions_permissions_delete_builder(
            &self.http_client,
            &args.name,
            &args.useAdminAccess,
        )
        .map_err(ProviderError::Api)?;

        let task = drivelabels_labels_revisions_permissions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
