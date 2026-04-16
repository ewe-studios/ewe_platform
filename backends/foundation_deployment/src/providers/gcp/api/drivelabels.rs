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
    drivelabels_labels_get_builder, drivelabels_labels_get_task,
    drivelabels_labels_list_builder, drivelabels_labels_list_task,
    drivelabels_labels_publish_builder, drivelabels_labels_publish_task,
    drivelabels_labels_update_label_copy_mode_builder, drivelabels_labels_update_label_copy_mode_task,
    drivelabels_labels_update_label_enabled_app_settings_builder, drivelabels_labels_update_label_enabled_app_settings_task,
    drivelabels_labels_update_permissions_builder, drivelabels_labels_update_permissions_task,
    drivelabels_labels_locks_list_builder, drivelabels_labels_locks_list_task,
    drivelabels_labels_permissions_batch_delete_builder, drivelabels_labels_permissions_batch_delete_task,
    drivelabels_labels_permissions_batch_update_builder, drivelabels_labels_permissions_batch_update_task,
    drivelabels_labels_permissions_create_builder, drivelabels_labels_permissions_create_task,
    drivelabels_labels_permissions_delete_builder, drivelabels_labels_permissions_delete_task,
    drivelabels_labels_permissions_list_builder, drivelabels_labels_permissions_list_task,
    drivelabels_labels_revisions_update_permissions_builder, drivelabels_labels_revisions_update_permissions_task,
    drivelabels_labels_revisions_locks_list_builder, drivelabels_labels_revisions_locks_list_task,
    drivelabels_labels_revisions_permissions_batch_delete_builder, drivelabels_labels_revisions_permissions_batch_delete_task,
    drivelabels_labels_revisions_permissions_batch_update_builder, drivelabels_labels_revisions_permissions_batch_update_task,
    drivelabels_labels_revisions_permissions_create_builder, drivelabels_labels_revisions_permissions_create_task,
    drivelabels_labels_revisions_permissions_delete_builder, drivelabels_labels_revisions_permissions_delete_task,
    drivelabels_labels_revisions_permissions_list_builder, drivelabels_labels_revisions_permissions_list_task,
    drivelabels_limits_get_label_builder, drivelabels_limits_get_label_task,
    drivelabels_users_get_capabilities_builder, drivelabels_users_get_capabilities_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::drivelabels::GoogleAppsDriveLabelsV2BatchUpdateLabelPermissionsResponse;
use crate::providers::gcp::clients::drivelabels::GoogleAppsDriveLabelsV2DeltaUpdateLabelResponse;
use crate::providers::gcp::clients::drivelabels::GoogleAppsDriveLabelsV2Label;
use crate::providers::gcp::clients::drivelabels::GoogleAppsDriveLabelsV2LabelLimits;
use crate::providers::gcp::clients::drivelabels::GoogleAppsDriveLabelsV2LabelPermission;
use crate::providers::gcp::clients::drivelabels::GoogleAppsDriveLabelsV2ListLabelLocksResponse;
use crate::providers::gcp::clients::drivelabels::GoogleAppsDriveLabelsV2ListLabelPermissionsResponse;
use crate::providers::gcp::clients::drivelabels::GoogleAppsDriveLabelsV2ListLabelsResponse;
use crate::providers::gcp::clients::drivelabels::GoogleAppsDriveLabelsV2UserCapabilities;
use crate::providers::gcp::clients::drivelabels::GoogleProtobufEmpty;
use crate::providers::gcp::clients::drivelabels::DrivelabelsLabelsCreateArgs;
use crate::providers::gcp::clients::drivelabels::DrivelabelsLabelsDeleteArgs;
use crate::providers::gcp::clients::drivelabels::DrivelabelsLabelsDeltaArgs;
use crate::providers::gcp::clients::drivelabels::DrivelabelsLabelsDisableArgs;
use crate::providers::gcp::clients::drivelabels::DrivelabelsLabelsEnableArgs;
use crate::providers::gcp::clients::drivelabels::DrivelabelsLabelsGetArgs;
use crate::providers::gcp::clients::drivelabels::DrivelabelsLabelsListArgs;
use crate::providers::gcp::clients::drivelabels::DrivelabelsLabelsLocksListArgs;
use crate::providers::gcp::clients::drivelabels::DrivelabelsLabelsPermissionsBatchDeleteArgs;
use crate::providers::gcp::clients::drivelabels::DrivelabelsLabelsPermissionsBatchUpdateArgs;
use crate::providers::gcp::clients::drivelabels::DrivelabelsLabelsPermissionsCreateArgs;
use crate::providers::gcp::clients::drivelabels::DrivelabelsLabelsPermissionsDeleteArgs;
use crate::providers::gcp::clients::drivelabels::DrivelabelsLabelsPermissionsListArgs;
use crate::providers::gcp::clients::drivelabels::DrivelabelsLabelsPublishArgs;
use crate::providers::gcp::clients::drivelabels::DrivelabelsLabelsRevisionsLocksListArgs;
use crate::providers::gcp::clients::drivelabels::DrivelabelsLabelsRevisionsPermissionsBatchDeleteArgs;
use crate::providers::gcp::clients::drivelabels::DrivelabelsLabelsRevisionsPermissionsBatchUpdateArgs;
use crate::providers::gcp::clients::drivelabels::DrivelabelsLabelsRevisionsPermissionsCreateArgs;
use crate::providers::gcp::clients::drivelabels::DrivelabelsLabelsRevisionsPermissionsDeleteArgs;
use crate::providers::gcp::clients::drivelabels::DrivelabelsLabelsRevisionsPermissionsListArgs;
use crate::providers::gcp::clients::drivelabels::DrivelabelsLabelsRevisionsUpdatePermissionsArgs;
use crate::providers::gcp::clients::drivelabels::DrivelabelsLabelsUpdateLabelCopyModeArgs;
use crate::providers::gcp::clients::drivelabels::DrivelabelsLabelsUpdateLabelEnabledAppSettingsArgs;
use crate::providers::gcp::clients::drivelabels::DrivelabelsLabelsUpdatePermissionsArgs;
use crate::providers::gcp::clients::drivelabels::DrivelabelsLimitsGetLabelArgs;
use crate::providers::gcp::clients::drivelabels::DrivelabelsUsersGetCapabilitiesArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// DrivelabelsProvider with automatic state tracking.
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
/// let provider = DrivelabelsProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct DrivelabelsProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> DrivelabelsProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new DrivelabelsProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new DrivelabelsProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
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
            &args.writeControl_requiredRevisionId,
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

    /// Drivelabels labels get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn drivelabels_labels_get(
        &self,
        args: &DrivelabelsLabelsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAppsDriveLabelsV2Label, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drivelabels_labels_get_builder(
            &self.http_client,
            &args.name,
            &args.languageCode,
            &args.useAdminAccess,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = drivelabels_labels_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drivelabels labels list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAppsDriveLabelsV2ListLabelsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn drivelabels_labels_list(
        &self,
        args: &DrivelabelsLabelsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAppsDriveLabelsV2ListLabelsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drivelabels_labels_list_builder(
            &self.http_client,
            &args.customer,
            &args.languageCode,
            &args.minimumRole,
            &args.pageSize,
            &args.pageToken,
            &args.publishedOnly,
            &args.useAdminAccess,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = drivelabels_labels_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Drivelabels labels locks list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAppsDriveLabelsV2ListLabelLocksResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn drivelabels_labels_locks_list(
        &self,
        args: &DrivelabelsLabelsLocksListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAppsDriveLabelsV2ListLabelLocksResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drivelabels_labels_locks_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = drivelabels_labels_locks_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Drivelabels labels permissions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAppsDriveLabelsV2ListLabelPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn drivelabels_labels_permissions_list(
        &self,
        args: &DrivelabelsLabelsPermissionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAppsDriveLabelsV2ListLabelPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drivelabels_labels_permissions_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.useAdminAccess,
        )
        .map_err(ProviderError::Api)?;

        let task = drivelabels_labels_permissions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Drivelabels labels revisions locks list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAppsDriveLabelsV2ListLabelLocksResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn drivelabels_labels_revisions_locks_list(
        &self,
        args: &DrivelabelsLabelsRevisionsLocksListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAppsDriveLabelsV2ListLabelLocksResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drivelabels_labels_revisions_locks_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = drivelabels_labels_revisions_locks_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Drivelabels labels revisions permissions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAppsDriveLabelsV2ListLabelPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn drivelabels_labels_revisions_permissions_list(
        &self,
        args: &DrivelabelsLabelsRevisionsPermissionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAppsDriveLabelsV2ListLabelPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drivelabels_labels_revisions_permissions_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.useAdminAccess,
        )
        .map_err(ProviderError::Api)?;

        let task = drivelabels_labels_revisions_permissions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drivelabels limits get label.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAppsDriveLabelsV2LabelLimits result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn drivelabels_limits_get_label(
        &self,
        args: &DrivelabelsLimitsGetLabelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAppsDriveLabelsV2LabelLimits, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drivelabels_limits_get_label_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = drivelabels_limits_get_label_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Drivelabels users get capabilities.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAppsDriveLabelsV2UserCapabilities result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn drivelabels_users_get_capabilities(
        &self,
        args: &DrivelabelsUsersGetCapabilitiesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAppsDriveLabelsV2UserCapabilities, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = drivelabels_users_get_capabilities_builder(
            &self.http_client,
            &args.name,
            &args.customer,
        )
        .map_err(ProviderError::Api)?;

        let task = drivelabels_users_get_capabilities_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
