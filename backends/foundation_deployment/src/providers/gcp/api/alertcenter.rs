//! AlertcenterProvider - State-aware alertcenter API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       alertcenter API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::alertcenter::{
    alertcenter_alerts_batch_delete_builder, alertcenter_alerts_batch_delete_task,
    alertcenter_alerts_batch_undelete_builder, alertcenter_alerts_batch_undelete_task,
    alertcenter_alerts_delete_builder, alertcenter_alerts_delete_task,
    alertcenter_alerts_undelete_builder, alertcenter_alerts_undelete_task,
    alertcenter_alerts_feedback_create_builder, alertcenter_alerts_feedback_create_task,
    alertcenter_update_settings_builder, alertcenter_update_settings_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::alertcenter::Alert;
use crate::providers::gcp::clients::alertcenter::AlertFeedback;
use crate::providers::gcp::clients::alertcenter::BatchDeleteAlertsResponse;
use crate::providers::gcp::clients::alertcenter::BatchUndeleteAlertsResponse;
use crate::providers::gcp::clients::alertcenter::Empty;
use crate::providers::gcp::clients::alertcenter::Settings;
use crate::providers::gcp::clients::alertcenter::AlertcenterAlertsBatchDeleteArgs;
use crate::providers::gcp::clients::alertcenter::AlertcenterAlertsBatchUndeleteArgs;
use crate::providers::gcp::clients::alertcenter::AlertcenterAlertsDeleteArgs;
use crate::providers::gcp::clients::alertcenter::AlertcenterAlertsFeedbackCreateArgs;
use crate::providers::gcp::clients::alertcenter::AlertcenterAlertsUndeleteArgs;
use crate::providers::gcp::clients::alertcenter::AlertcenterUpdateSettingsArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// AlertcenterProvider with automatic state tracking.
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
/// let provider = AlertcenterProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct AlertcenterProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> AlertcenterProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new AlertcenterProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Alertcenter alerts batch delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchDeleteAlertsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn alertcenter_alerts_batch_delete(
        &self,
        args: &AlertcenterAlertsBatchDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchDeleteAlertsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alertcenter_alerts_batch_delete_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = alertcenter_alerts_batch_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Alertcenter alerts batch undelete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchUndeleteAlertsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn alertcenter_alerts_batch_undelete(
        &self,
        args: &AlertcenterAlertsBatchUndeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchUndeleteAlertsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alertcenter_alerts_batch_undelete_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = alertcenter_alerts_batch_undelete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Alertcenter alerts delete.
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
    pub fn alertcenter_alerts_delete(
        &self,
        args: &AlertcenterAlertsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alertcenter_alerts_delete_builder(
            &self.http_client,
            &args.alertId,
            &args.customerId,
        )
        .map_err(ProviderError::Api)?;

        let task = alertcenter_alerts_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Alertcenter alerts undelete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Alert result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn alertcenter_alerts_undelete(
        &self,
        args: &AlertcenterAlertsUndeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Alert, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alertcenter_alerts_undelete_builder(
            &self.http_client,
            &args.alertId,
        )
        .map_err(ProviderError::Api)?;

        let task = alertcenter_alerts_undelete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Alertcenter alerts feedback create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AlertFeedback result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn alertcenter_alerts_feedback_create(
        &self,
        args: &AlertcenterAlertsFeedbackCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AlertFeedback, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alertcenter_alerts_feedback_create_builder(
            &self.http_client,
            &args.alertId,
            &args.customerId,
        )
        .map_err(ProviderError::Api)?;

        let task = alertcenter_alerts_feedback_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Alertcenter update settings.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Settings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn alertcenter_update_settings(
        &self,
        args: &AlertcenterUpdateSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Settings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alertcenter_update_settings_builder(
            &self.http_client,
            &args.customerId,
        )
        .map_err(ProviderError::Api)?;

        let task = alertcenter_update_settings_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
