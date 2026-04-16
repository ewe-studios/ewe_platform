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
    alertcenter_alerts_get_builder, alertcenter_alerts_get_task,
    alertcenter_alerts_get_metadata_builder, alertcenter_alerts_get_metadata_task,
    alertcenter_alerts_list_builder, alertcenter_alerts_list_task,
    alertcenter_alerts_undelete_builder, alertcenter_alerts_undelete_task,
    alertcenter_alerts_feedback_create_builder, alertcenter_alerts_feedback_create_task,
    alertcenter_alerts_feedback_list_builder, alertcenter_alerts_feedback_list_task,
    alertcenter_get_settings_builder, alertcenter_get_settings_task,
    alertcenter_update_settings_builder, alertcenter_update_settings_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::alertcenter::Alert;
use crate::providers::gcp::clients::alertcenter::AlertFeedback;
use crate::providers::gcp::clients::alertcenter::AlertMetadata;
use crate::providers::gcp::clients::alertcenter::BatchDeleteAlertsResponse;
use crate::providers::gcp::clients::alertcenter::BatchUndeleteAlertsResponse;
use crate::providers::gcp::clients::alertcenter::Empty;
use crate::providers::gcp::clients::alertcenter::ListAlertFeedbackResponse;
use crate::providers::gcp::clients::alertcenter::ListAlertsResponse;
use crate::providers::gcp::clients::alertcenter::Settings;
use crate::providers::gcp::clients::alertcenter::AlertcenterAlertsDeleteArgs;
use crate::providers::gcp::clients::alertcenter::AlertcenterAlertsFeedbackCreateArgs;
use crate::providers::gcp::clients::alertcenter::AlertcenterAlertsFeedbackListArgs;
use crate::providers::gcp::clients::alertcenter::AlertcenterAlertsGetArgs;
use crate::providers::gcp::clients::alertcenter::AlertcenterAlertsGetMetadataArgs;
use crate::providers::gcp::clients::alertcenter::AlertcenterAlertsListArgs;
use crate::providers::gcp::clients::alertcenter::AlertcenterAlertsUndeleteArgs;
use crate::providers::gcp::clients::alertcenter::AlertcenterGetSettingsArgs;
use crate::providers::gcp::clients::alertcenter::AlertcenterUpdateSettingsArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// AlertcenterProvider with automatic state tracking.
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
/// let provider = AlertcenterProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct AlertcenterProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> AlertcenterProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new AlertcenterProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new AlertcenterProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
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

    /// Alertcenter alerts get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn alertcenter_alerts_get(
        &self,
        args: &AlertcenterAlertsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Alert, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alertcenter_alerts_get_builder(
            &self.http_client,
            &args.alertId,
            &args.customerId,
        )
        .map_err(ProviderError::Api)?;

        let task = alertcenter_alerts_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Alertcenter alerts get metadata.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AlertMetadata result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn alertcenter_alerts_get_metadata(
        &self,
        args: &AlertcenterAlertsGetMetadataArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AlertMetadata, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alertcenter_alerts_get_metadata_builder(
            &self.http_client,
            &args.alertId,
            &args.customerId,
        )
        .map_err(ProviderError::Api)?;

        let task = alertcenter_alerts_get_metadata_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Alertcenter alerts list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAlertsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn alertcenter_alerts_list(
        &self,
        args: &AlertcenterAlertsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAlertsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alertcenter_alerts_list_builder(
            &self.http_client,
            &args.customerId,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = alertcenter_alerts_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Alertcenter alerts feedback list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAlertFeedbackResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn alertcenter_alerts_feedback_list(
        &self,
        args: &AlertcenterAlertsFeedbackListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAlertFeedbackResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alertcenter_alerts_feedback_list_builder(
            &self.http_client,
            &args.alertId,
            &args.customerId,
            &args.filter,
        )
        .map_err(ProviderError::Api)?;

        let task = alertcenter_alerts_feedback_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Alertcenter get settings.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn alertcenter_get_settings(
        &self,
        args: &AlertcenterGetSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Settings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alertcenter_get_settings_builder(
            &self.http_client,
            &args.customerId,
        )
        .map_err(ProviderError::Api)?;

        let task = alertcenter_get_settings_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
