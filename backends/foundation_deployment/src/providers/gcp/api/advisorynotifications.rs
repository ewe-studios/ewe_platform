//! AdvisorynotificationsProvider - State-aware advisorynotifications API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       advisorynotifications API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::advisorynotifications::{
    advisorynotifications_organizations_locations_get_settings_builder, advisorynotifications_organizations_locations_get_settings_task,
    advisorynotifications_organizations_locations_update_settings_builder, advisorynotifications_organizations_locations_update_settings_task,
    advisorynotifications_organizations_locations_notifications_get_builder, advisorynotifications_organizations_locations_notifications_get_task,
    advisorynotifications_organizations_locations_notifications_list_builder, advisorynotifications_organizations_locations_notifications_list_task,
    advisorynotifications_projects_locations_get_settings_builder, advisorynotifications_projects_locations_get_settings_task,
    advisorynotifications_projects_locations_update_settings_builder, advisorynotifications_projects_locations_update_settings_task,
    advisorynotifications_projects_locations_notifications_get_builder, advisorynotifications_projects_locations_notifications_get_task,
    advisorynotifications_projects_locations_notifications_list_builder, advisorynotifications_projects_locations_notifications_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::advisorynotifications::GoogleCloudAdvisorynotificationsV1ListNotificationsResponse;
use crate::providers::gcp::clients::advisorynotifications::GoogleCloudAdvisorynotificationsV1Notification;
use crate::providers::gcp::clients::advisorynotifications::GoogleCloudAdvisorynotificationsV1Settings;
use crate::providers::gcp::clients::advisorynotifications::AdvisorynotificationsOrganizationsLocationsGetSettingsArgs;
use crate::providers::gcp::clients::advisorynotifications::AdvisorynotificationsOrganizationsLocationsNotificationsGetArgs;
use crate::providers::gcp::clients::advisorynotifications::AdvisorynotificationsOrganizationsLocationsNotificationsListArgs;
use crate::providers::gcp::clients::advisorynotifications::AdvisorynotificationsOrganizationsLocationsUpdateSettingsArgs;
use crate::providers::gcp::clients::advisorynotifications::AdvisorynotificationsProjectsLocationsGetSettingsArgs;
use crate::providers::gcp::clients::advisorynotifications::AdvisorynotificationsProjectsLocationsNotificationsGetArgs;
use crate::providers::gcp::clients::advisorynotifications::AdvisorynotificationsProjectsLocationsNotificationsListArgs;
use crate::providers::gcp::clients::advisorynotifications::AdvisorynotificationsProjectsLocationsUpdateSettingsArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// AdvisorynotificationsProvider with automatic state tracking.
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
/// let provider = AdvisorynotificationsProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct AdvisorynotificationsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> AdvisorynotificationsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new AdvisorynotificationsProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Advisorynotifications organizations locations get settings.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudAdvisorynotificationsV1Settings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn advisorynotifications_organizations_locations_get_settings(
        &self,
        args: &AdvisorynotificationsOrganizationsLocationsGetSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudAdvisorynotificationsV1Settings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = advisorynotifications_organizations_locations_get_settings_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = advisorynotifications_organizations_locations_get_settings_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Advisorynotifications organizations locations update settings.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudAdvisorynotificationsV1Settings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn advisorynotifications_organizations_locations_update_settings(
        &self,
        args: &AdvisorynotificationsOrganizationsLocationsUpdateSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudAdvisorynotificationsV1Settings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = advisorynotifications_organizations_locations_update_settings_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = advisorynotifications_organizations_locations_update_settings_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Advisorynotifications organizations locations notifications get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudAdvisorynotificationsV1Notification result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn advisorynotifications_organizations_locations_notifications_get(
        &self,
        args: &AdvisorynotificationsOrganizationsLocationsNotificationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudAdvisorynotificationsV1Notification, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = advisorynotifications_organizations_locations_notifications_get_builder(
            &self.http_client,
            &args.name,
            &args.languageCode,
        )
        .map_err(ProviderError::Api)?;

        let task = advisorynotifications_organizations_locations_notifications_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Advisorynotifications organizations locations notifications list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudAdvisorynotificationsV1ListNotificationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn advisorynotifications_organizations_locations_notifications_list(
        &self,
        args: &AdvisorynotificationsOrganizationsLocationsNotificationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudAdvisorynotificationsV1ListNotificationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = advisorynotifications_organizations_locations_notifications_list_builder(
            &self.http_client,
            &args.parent,
            &args.languageCode,
            &args.pageSize,
            &args.pageToken,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = advisorynotifications_organizations_locations_notifications_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Advisorynotifications projects locations get settings.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudAdvisorynotificationsV1Settings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn advisorynotifications_projects_locations_get_settings(
        &self,
        args: &AdvisorynotificationsProjectsLocationsGetSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudAdvisorynotificationsV1Settings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = advisorynotifications_projects_locations_get_settings_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = advisorynotifications_projects_locations_get_settings_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Advisorynotifications projects locations update settings.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudAdvisorynotificationsV1Settings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn advisorynotifications_projects_locations_update_settings(
        &self,
        args: &AdvisorynotificationsProjectsLocationsUpdateSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudAdvisorynotificationsV1Settings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = advisorynotifications_projects_locations_update_settings_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = advisorynotifications_projects_locations_update_settings_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Advisorynotifications projects locations notifications get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudAdvisorynotificationsV1Notification result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn advisorynotifications_projects_locations_notifications_get(
        &self,
        args: &AdvisorynotificationsProjectsLocationsNotificationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudAdvisorynotificationsV1Notification, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = advisorynotifications_projects_locations_notifications_get_builder(
            &self.http_client,
            &args.name,
            &args.languageCode,
        )
        .map_err(ProviderError::Api)?;

        let task = advisorynotifications_projects_locations_notifications_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Advisorynotifications projects locations notifications list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudAdvisorynotificationsV1ListNotificationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn advisorynotifications_projects_locations_notifications_list(
        &self,
        args: &AdvisorynotificationsProjectsLocationsNotificationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudAdvisorynotificationsV1ListNotificationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = advisorynotifications_projects_locations_notifications_list_builder(
            &self.http_client,
            &args.parent,
            &args.languageCode,
            &args.pageSize,
            &args.pageToken,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = advisorynotifications_projects_locations_notifications_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
