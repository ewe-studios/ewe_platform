//! MybusinessnotificationsProvider - State-aware mybusinessnotifications API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       mybusinessnotifications API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::mybusinessnotifications::{
    mybusinessnotifications_accounts_get_notification_setting_builder, mybusinessnotifications_accounts_get_notification_setting_task,
    mybusinessnotifications_accounts_update_notification_setting_builder, mybusinessnotifications_accounts_update_notification_setting_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::mybusinessnotifications::NotificationSetting;
use crate::providers::gcp::clients::mybusinessnotifications::MybusinessnotificationsAccountsGetNotificationSettingArgs;
use crate::providers::gcp::clients::mybusinessnotifications::MybusinessnotificationsAccountsUpdateNotificationSettingArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// MybusinessnotificationsProvider with automatic state tracking.
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
/// let provider = MybusinessnotificationsProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct MybusinessnotificationsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> MybusinessnotificationsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new MybusinessnotificationsProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Mybusinessnotifications accounts get notification setting.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NotificationSetting result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn mybusinessnotifications_accounts_get_notification_setting(
        &self,
        args: &MybusinessnotificationsAccountsGetNotificationSettingArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NotificationSetting, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessnotifications_accounts_get_notification_setting_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessnotifications_accounts_get_notification_setting_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Mybusinessnotifications accounts update notification setting.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NotificationSetting result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn mybusinessnotifications_accounts_update_notification_setting(
        &self,
        args: &MybusinessnotificationsAccountsUpdateNotificationSettingArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NotificationSetting, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessnotifications_accounts_update_notification_setting_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessnotifications_accounts_update_notification_setting_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
