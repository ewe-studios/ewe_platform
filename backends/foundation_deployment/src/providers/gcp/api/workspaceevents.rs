//! WorkspaceeventsProvider - State-aware workspaceevents API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       workspaceevents API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::workspaceevents::{
    workspaceevents_message_stream_builder, workspaceevents_message_stream_task,
    workspaceevents_subscriptions_create_builder, workspaceevents_subscriptions_create_task,
    workspaceevents_subscriptions_delete_builder, workspaceevents_subscriptions_delete_task,
    workspaceevents_subscriptions_patch_builder, workspaceevents_subscriptions_patch_task,
    workspaceevents_subscriptions_reactivate_builder, workspaceevents_subscriptions_reactivate_task,
    workspaceevents_tasks_cancel_builder, workspaceevents_tasks_cancel_task,
    workspaceevents_tasks_push_notification_configs_create_builder, workspaceevents_tasks_push_notification_configs_create_task,
    workspaceevents_tasks_push_notification_configs_delete_builder, workspaceevents_tasks_push_notification_configs_delete_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::workspaceevents::Empty;
use crate::providers::gcp::clients::workspaceevents::Operation;
use crate::providers::gcp::clients::workspaceevents::StreamResponse;
use crate::providers::gcp::clients::workspaceevents::Task;
use crate::providers::gcp::clients::workspaceevents::TaskPushNotificationConfig;
use crate::providers::gcp::clients::workspaceevents::WorkspaceeventsMessageStreamArgs;
use crate::providers::gcp::clients::workspaceevents::WorkspaceeventsSubscriptionsCreateArgs;
use crate::providers::gcp::clients::workspaceevents::WorkspaceeventsSubscriptionsDeleteArgs;
use crate::providers::gcp::clients::workspaceevents::WorkspaceeventsSubscriptionsPatchArgs;
use crate::providers::gcp::clients::workspaceevents::WorkspaceeventsSubscriptionsReactivateArgs;
use crate::providers::gcp::clients::workspaceevents::WorkspaceeventsTasksCancelArgs;
use crate::providers::gcp::clients::workspaceevents::WorkspaceeventsTasksPushNotificationConfigsCreateArgs;
use crate::providers::gcp::clients::workspaceevents::WorkspaceeventsTasksPushNotificationConfigsDeleteArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// WorkspaceeventsProvider with automatic state tracking.
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
/// let provider = WorkspaceeventsProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct WorkspaceeventsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> WorkspaceeventsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new WorkspaceeventsProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Workspaceevents message stream.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the StreamResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn workspaceevents_message_stream(
        &self,
        args: &WorkspaceeventsMessageStreamArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<StreamResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workspaceevents_message_stream_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = workspaceevents_message_stream_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workspaceevents subscriptions create.
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
    pub fn workspaceevents_subscriptions_create(
        &self,
        args: &WorkspaceeventsSubscriptionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workspaceevents_subscriptions_create_builder(
            &self.http_client,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = workspaceevents_subscriptions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workspaceevents subscriptions delete.
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
    pub fn workspaceevents_subscriptions_delete(
        &self,
        args: &WorkspaceeventsSubscriptionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workspaceevents_subscriptions_delete_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.etag,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = workspaceevents_subscriptions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workspaceevents subscriptions patch.
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
    pub fn workspaceevents_subscriptions_patch(
        &self,
        args: &WorkspaceeventsSubscriptionsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workspaceevents_subscriptions_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = workspaceevents_subscriptions_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workspaceevents subscriptions reactivate.
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
    pub fn workspaceevents_subscriptions_reactivate(
        &self,
        args: &WorkspaceeventsSubscriptionsReactivateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workspaceevents_subscriptions_reactivate_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = workspaceevents_subscriptions_reactivate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workspaceevents tasks cancel.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Task result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn workspaceevents_tasks_cancel(
        &self,
        args: &WorkspaceeventsTasksCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Task, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workspaceevents_tasks_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = workspaceevents_tasks_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workspaceevents tasks push notification configs create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TaskPushNotificationConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn workspaceevents_tasks_push_notification_configs_create(
        &self,
        args: &WorkspaceeventsTasksPushNotificationConfigsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TaskPushNotificationConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workspaceevents_tasks_push_notification_configs_create_builder(
            &self.http_client,
            &args.parent,
            &args.configId,
            &args.tenant,
        )
        .map_err(ProviderError::Api)?;

        let task = workspaceevents_tasks_push_notification_configs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workspaceevents tasks push notification configs delete.
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
    pub fn workspaceevents_tasks_push_notification_configs_delete(
        &self,
        args: &WorkspaceeventsTasksPushNotificationConfigsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workspaceevents_tasks_push_notification_configs_delete_builder(
            &self.http_client,
            &args.name,
            &args.tenant,
        )
        .map_err(ProviderError::Api)?;

        let task = workspaceevents_tasks_push_notification_configs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
