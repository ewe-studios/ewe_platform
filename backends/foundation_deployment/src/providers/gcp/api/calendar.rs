//! CalendarProvider - State-aware calendar API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       calendar API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::calendar::{
    calendar_acl_delete_builder, calendar_acl_delete_task,
    calendar_acl_get_builder, calendar_acl_get_task,
    calendar_acl_insert_builder, calendar_acl_insert_task,
    calendar_acl_list_builder, calendar_acl_list_task,
    calendar_acl_patch_builder, calendar_acl_patch_task,
    calendar_acl_update_builder, calendar_acl_update_task,
    calendar_acl_watch_builder, calendar_acl_watch_task,
    calendar_calendar_list_delete_builder, calendar_calendar_list_delete_task,
    calendar_calendar_list_get_builder, calendar_calendar_list_get_task,
    calendar_calendar_list_insert_builder, calendar_calendar_list_insert_task,
    calendar_calendar_list_list_builder, calendar_calendar_list_list_task,
    calendar_calendar_list_patch_builder, calendar_calendar_list_patch_task,
    calendar_calendar_list_update_builder, calendar_calendar_list_update_task,
    calendar_calendar_list_watch_builder, calendar_calendar_list_watch_task,
    calendar_calendars_clear_builder, calendar_calendars_clear_task,
    calendar_calendars_delete_builder, calendar_calendars_delete_task,
    calendar_calendars_get_builder, calendar_calendars_get_task,
    calendar_calendars_insert_builder, calendar_calendars_insert_task,
    calendar_calendars_patch_builder, calendar_calendars_patch_task,
    calendar_calendars_update_builder, calendar_calendars_update_task,
    calendar_channels_stop_builder, calendar_channels_stop_task,
    calendar_colors_get_builder, calendar_colors_get_task,
    calendar_events_delete_builder, calendar_events_delete_task,
    calendar_events_get_builder, calendar_events_get_task,
    calendar_events_import_builder, calendar_events_import_task,
    calendar_events_insert_builder, calendar_events_insert_task,
    calendar_events_instances_builder, calendar_events_instances_task,
    calendar_events_list_builder, calendar_events_list_task,
    calendar_events_move_builder, calendar_events_move_task,
    calendar_events_patch_builder, calendar_events_patch_task,
    calendar_events_quick_add_builder, calendar_events_quick_add_task,
    calendar_events_update_builder, calendar_events_update_task,
    calendar_events_watch_builder, calendar_events_watch_task,
    calendar_freebusy_query_builder, calendar_freebusy_query_task,
    calendar_settings_get_builder, calendar_settings_get_task,
    calendar_settings_list_builder, calendar_settings_list_task,
    calendar_settings_watch_builder, calendar_settings_watch_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::calendar::Acl;
use crate::providers::gcp::clients::calendar::AclRule;
use crate::providers::gcp::clients::calendar::Calendar;
use crate::providers::gcp::clients::calendar::CalendarList;
use crate::providers::gcp::clients::calendar::CalendarListEntry;
use crate::providers::gcp::clients::calendar::Channel;
use crate::providers::gcp::clients::calendar::Colors;
use crate::providers::gcp::clients::calendar::Event;
use crate::providers::gcp::clients::calendar::Events;
use crate::providers::gcp::clients::calendar::FreeBusyResponse;
use crate::providers::gcp::clients::calendar::Setting;
use crate::providers::gcp::clients::calendar::Settings;
use crate::providers::gcp::clients::calendar::CalendarAclDeleteArgs;
use crate::providers::gcp::clients::calendar::CalendarAclGetArgs;
use crate::providers::gcp::clients::calendar::CalendarAclInsertArgs;
use crate::providers::gcp::clients::calendar::CalendarAclListArgs;
use crate::providers::gcp::clients::calendar::CalendarAclPatchArgs;
use crate::providers::gcp::clients::calendar::CalendarAclUpdateArgs;
use crate::providers::gcp::clients::calendar::CalendarAclWatchArgs;
use crate::providers::gcp::clients::calendar::CalendarCalendarListDeleteArgs;
use crate::providers::gcp::clients::calendar::CalendarCalendarListGetArgs;
use crate::providers::gcp::clients::calendar::CalendarCalendarListInsertArgs;
use crate::providers::gcp::clients::calendar::CalendarCalendarListListArgs;
use crate::providers::gcp::clients::calendar::CalendarCalendarListPatchArgs;
use crate::providers::gcp::clients::calendar::CalendarCalendarListUpdateArgs;
use crate::providers::gcp::clients::calendar::CalendarCalendarListWatchArgs;
use crate::providers::gcp::clients::calendar::CalendarCalendarsClearArgs;
use crate::providers::gcp::clients::calendar::CalendarCalendarsDeleteArgs;
use crate::providers::gcp::clients::calendar::CalendarCalendarsGetArgs;
use crate::providers::gcp::clients::calendar::CalendarCalendarsPatchArgs;
use crate::providers::gcp::clients::calendar::CalendarCalendarsUpdateArgs;
use crate::providers::gcp::clients::calendar::CalendarEventsDeleteArgs;
use crate::providers::gcp::clients::calendar::CalendarEventsGetArgs;
use crate::providers::gcp::clients::calendar::CalendarEventsImportArgs;
use crate::providers::gcp::clients::calendar::CalendarEventsInsertArgs;
use crate::providers::gcp::clients::calendar::CalendarEventsInstancesArgs;
use crate::providers::gcp::clients::calendar::CalendarEventsListArgs;
use crate::providers::gcp::clients::calendar::CalendarEventsMoveArgs;
use crate::providers::gcp::clients::calendar::CalendarEventsPatchArgs;
use crate::providers::gcp::clients::calendar::CalendarEventsQuickAddArgs;
use crate::providers::gcp::clients::calendar::CalendarEventsUpdateArgs;
use crate::providers::gcp::clients::calendar::CalendarEventsWatchArgs;
use crate::providers::gcp::clients::calendar::CalendarSettingsGetArgs;
use crate::providers::gcp::clients::calendar::CalendarSettingsListArgs;
use crate::providers::gcp::clients::calendar::CalendarSettingsWatchArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// CalendarProvider with automatic state tracking.
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
/// let provider = CalendarProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct CalendarProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> CalendarProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new CalendarProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new CalendarProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Calendar acl delete.
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
    pub fn calendar_acl_delete(
        &self,
        args: &CalendarAclDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = calendar_acl_delete_builder(
            &self.http_client,
            &args.calendarId,
            &args.ruleId,
        )
        .map_err(ProviderError::Api)?;

        let task = calendar_acl_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Calendar acl get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AclRule result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn calendar_acl_get(
        &self,
        args: &CalendarAclGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AclRule, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = calendar_acl_get_builder(
            &self.http_client,
            &args.calendarId,
            &args.ruleId,
        )
        .map_err(ProviderError::Api)?;

        let task = calendar_acl_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Calendar acl insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AclRule result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn calendar_acl_insert(
        &self,
        args: &CalendarAclInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AclRule, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = calendar_acl_insert_builder(
            &self.http_client,
            &args.calendarId,
            &args.sendNotifications,
        )
        .map_err(ProviderError::Api)?;

        let task = calendar_acl_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Calendar acl list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Acl result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn calendar_acl_list(
        &self,
        args: &CalendarAclListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Acl, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = calendar_acl_list_builder(
            &self.http_client,
            &args.calendarId,
            &args.maxResults,
            &args.pageToken,
            &args.showDeleted,
            &args.syncToken,
        )
        .map_err(ProviderError::Api)?;

        let task = calendar_acl_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Calendar acl patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AclRule result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn calendar_acl_patch(
        &self,
        args: &CalendarAclPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AclRule, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = calendar_acl_patch_builder(
            &self.http_client,
            &args.calendarId,
            &args.ruleId,
            &args.sendNotifications,
        )
        .map_err(ProviderError::Api)?;

        let task = calendar_acl_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Calendar acl update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AclRule result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn calendar_acl_update(
        &self,
        args: &CalendarAclUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AclRule, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = calendar_acl_update_builder(
            &self.http_client,
            &args.calendarId,
            &args.ruleId,
            &args.sendNotifications,
        )
        .map_err(ProviderError::Api)?;

        let task = calendar_acl_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Calendar acl watch.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn calendar_acl_watch(
        &self,
        args: &CalendarAclWatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Channel, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = calendar_acl_watch_builder(
            &self.http_client,
            &args.calendarId,
            &args.maxResults,
            &args.pageToken,
            &args.showDeleted,
            &args.syncToken,
        )
        .map_err(ProviderError::Api)?;

        let task = calendar_acl_watch_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Calendar calendar list delete.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn calendar_calendar_list_delete(
        &self,
        args: &CalendarCalendarListDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = calendar_calendar_list_delete_builder(
            &self.http_client,
            &args.calendarId,
        )
        .map_err(ProviderError::Api)?;

        let task = calendar_calendar_list_delete_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Calendar calendar list get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CalendarListEntry result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn calendar_calendar_list_get(
        &self,
        args: &CalendarCalendarListGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CalendarListEntry, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = calendar_calendar_list_get_builder(
            &self.http_client,
            &args.calendarId,
        )
        .map_err(ProviderError::Api)?;

        let task = calendar_calendar_list_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Calendar calendar list insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CalendarListEntry result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn calendar_calendar_list_insert(
        &self,
        args: &CalendarCalendarListInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CalendarListEntry, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = calendar_calendar_list_insert_builder(
            &self.http_client,
            &args.colorRgbFormat,
        )
        .map_err(ProviderError::Api)?;

        let task = calendar_calendar_list_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Calendar calendar list list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CalendarList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn calendar_calendar_list_list(
        &self,
        args: &CalendarCalendarListListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CalendarList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = calendar_calendar_list_list_builder(
            &self.http_client,
            &args.maxResults,
            &args.minAccessRole,
            &args.pageToken,
            &args.showDeleted,
            &args.showHidden,
            &args.syncToken,
        )
        .map_err(ProviderError::Api)?;

        let task = calendar_calendar_list_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Calendar calendar list patch.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CalendarListEntry result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn calendar_calendar_list_patch(
        &self,
        args: &CalendarCalendarListPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CalendarListEntry, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = calendar_calendar_list_patch_builder(
            &self.http_client,
            &args.calendarId,
            &args.colorRgbFormat,
        )
        .map_err(ProviderError::Api)?;

        let task = calendar_calendar_list_patch_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Calendar calendar list update.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CalendarListEntry result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn calendar_calendar_list_update(
        &self,
        args: &CalendarCalendarListUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CalendarListEntry, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = calendar_calendar_list_update_builder(
            &self.http_client,
            &args.calendarId,
            &args.colorRgbFormat,
        )
        .map_err(ProviderError::Api)?;

        let task = calendar_calendar_list_update_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Calendar calendar list watch.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn calendar_calendar_list_watch(
        &self,
        args: &CalendarCalendarListWatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Channel, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = calendar_calendar_list_watch_builder(
            &self.http_client,
            &args.maxResults,
            &args.minAccessRole,
            &args.pageToken,
            &args.showDeleted,
            &args.showHidden,
            &args.syncToken,
        )
        .map_err(ProviderError::Api)?;

        let task = calendar_calendar_list_watch_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Calendar calendars clear.
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
    pub fn calendar_calendars_clear(
        &self,
        args: &CalendarCalendarsClearArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = calendar_calendars_clear_builder(
            &self.http_client,
            &args.calendarId,
        )
        .map_err(ProviderError::Api)?;

        let task = calendar_calendars_clear_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Calendar calendars delete.
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
    pub fn calendar_calendars_delete(
        &self,
        args: &CalendarCalendarsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = calendar_calendars_delete_builder(
            &self.http_client,
            &args.calendarId,
        )
        .map_err(ProviderError::Api)?;

        let task = calendar_calendars_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Calendar calendars get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Calendar result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn calendar_calendars_get(
        &self,
        args: &CalendarCalendarsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Calendar, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = calendar_calendars_get_builder(
            &self.http_client,
            &args.calendarId,
        )
        .map_err(ProviderError::Api)?;

        let task = calendar_calendars_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Calendar calendars insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Calendar result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn calendar_calendars_insert(
        &self,
        args: &CalendarCalendarsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Calendar, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = calendar_calendars_insert_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = calendar_calendars_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Calendar calendars patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Calendar result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn calendar_calendars_patch(
        &self,
        args: &CalendarCalendarsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Calendar, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = calendar_calendars_patch_builder(
            &self.http_client,
            &args.calendarId,
        )
        .map_err(ProviderError::Api)?;

        let task = calendar_calendars_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Calendar calendars update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Calendar result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn calendar_calendars_update(
        &self,
        args: &CalendarCalendarsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Calendar, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = calendar_calendars_update_builder(
            &self.http_client,
            &args.calendarId,
        )
        .map_err(ProviderError::Api)?;

        let task = calendar_calendars_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Calendar channels stop.
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
    pub fn calendar_channels_stop(
        &self,
        args: &CalendarChannelsStopArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = calendar_channels_stop_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = calendar_channels_stop_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Calendar colors get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Colors result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn calendar_colors_get(
        &self,
        args: &CalendarColorsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Colors, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = calendar_colors_get_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = calendar_colors_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Calendar events delete.
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
    pub fn calendar_events_delete(
        &self,
        args: &CalendarEventsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = calendar_events_delete_builder(
            &self.http_client,
            &args.calendarId,
            &args.eventId,
            &args.sendNotifications,
            &args.sendUpdates,
        )
        .map_err(ProviderError::Api)?;

        let task = calendar_events_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Calendar events get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Event result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn calendar_events_get(
        &self,
        args: &CalendarEventsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Event, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = calendar_events_get_builder(
            &self.http_client,
            &args.calendarId,
            &args.eventId,
            &args.alwaysIncludeEmail,
            &args.maxAttendees,
            &args.timeZone,
        )
        .map_err(ProviderError::Api)?;

        let task = calendar_events_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Calendar events import.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Event result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn calendar_events_import(
        &self,
        args: &CalendarEventsImportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Event, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = calendar_events_import_builder(
            &self.http_client,
            &args.calendarId,
            &args.conferenceDataVersion,
            &args.supportsAttachments,
        )
        .map_err(ProviderError::Api)?;

        let task = calendar_events_import_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Calendar events insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Event result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn calendar_events_insert(
        &self,
        args: &CalendarEventsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Event, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = calendar_events_insert_builder(
            &self.http_client,
            &args.calendarId,
            &args.conferenceDataVersion,
            &args.maxAttendees,
            &args.sendNotifications,
            &args.sendUpdates,
            &args.supportsAttachments,
        )
        .map_err(ProviderError::Api)?;

        let task = calendar_events_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Calendar events instances.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Events result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn calendar_events_instances(
        &self,
        args: &CalendarEventsInstancesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Events, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = calendar_events_instances_builder(
            &self.http_client,
            &args.calendarId,
            &args.eventId,
            &args.alwaysIncludeEmail,
            &args.maxAttendees,
            &args.maxResults,
            &args.originalStart,
            &args.pageToken,
            &args.showDeleted,
            &args.timeMax,
            &args.timeMin,
            &args.timeZone,
        )
        .map_err(ProviderError::Api)?;

        let task = calendar_events_instances_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Calendar events list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Events result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn calendar_events_list(
        &self,
        args: &CalendarEventsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Events, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = calendar_events_list_builder(
            &self.http_client,
            &args.calendarId,
            &args.alwaysIncludeEmail,
            &args.eventTypes,
            &args.iCalUID,
            &args.maxAttendees,
            &args.maxResults,
            &args.orderBy,
            &args.pageToken,
            &args.privateExtendedProperty,
            &args.q,
            &args.sharedExtendedProperty,
            &args.showDeleted,
            &args.showHiddenInvitations,
            &args.singleEvents,
            &args.syncToken,
            &args.timeMax,
            &args.timeMin,
            &args.timeZone,
            &args.updatedMin,
        )
        .map_err(ProviderError::Api)?;

        let task = calendar_events_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Calendar events move.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Event result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn calendar_events_move(
        &self,
        args: &CalendarEventsMoveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Event, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = calendar_events_move_builder(
            &self.http_client,
            &args.calendarId,
            &args.eventId,
            &args.destination,
            &args.sendNotifications,
            &args.sendUpdates,
        )
        .map_err(ProviderError::Api)?;

        let task = calendar_events_move_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Calendar events patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Event result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn calendar_events_patch(
        &self,
        args: &CalendarEventsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Event, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = calendar_events_patch_builder(
            &self.http_client,
            &args.calendarId,
            &args.eventId,
            &args.alwaysIncludeEmail,
            &args.conferenceDataVersion,
            &args.maxAttendees,
            &args.sendNotifications,
            &args.sendUpdates,
            &args.supportsAttachments,
        )
        .map_err(ProviderError::Api)?;

        let task = calendar_events_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Calendar events quick add.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Event result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn calendar_events_quick_add(
        &self,
        args: &CalendarEventsQuickAddArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Event, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = calendar_events_quick_add_builder(
            &self.http_client,
            &args.calendarId,
            &args.sendNotifications,
            &args.sendUpdates,
            &args.text,
        )
        .map_err(ProviderError::Api)?;

        let task = calendar_events_quick_add_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Calendar events update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Event result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn calendar_events_update(
        &self,
        args: &CalendarEventsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Event, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = calendar_events_update_builder(
            &self.http_client,
            &args.calendarId,
            &args.eventId,
            &args.alwaysIncludeEmail,
            &args.conferenceDataVersion,
            &args.maxAttendees,
            &args.sendNotifications,
            &args.sendUpdates,
            &args.supportsAttachments,
        )
        .map_err(ProviderError::Api)?;

        let task = calendar_events_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Calendar events watch.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn calendar_events_watch(
        &self,
        args: &CalendarEventsWatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Channel, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = calendar_events_watch_builder(
            &self.http_client,
            &args.calendarId,
            &args.alwaysIncludeEmail,
            &args.eventTypes,
            &args.iCalUID,
            &args.maxAttendees,
            &args.maxResults,
            &args.orderBy,
            &args.pageToken,
            &args.privateExtendedProperty,
            &args.q,
            &args.sharedExtendedProperty,
            &args.showDeleted,
            &args.showHiddenInvitations,
            &args.singleEvents,
            &args.syncToken,
            &args.timeMax,
            &args.timeMin,
            &args.timeZone,
            &args.updatedMin,
        )
        .map_err(ProviderError::Api)?;

        let task = calendar_events_watch_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Calendar freebusy query.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FreeBusyResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn calendar_freebusy_query(
        &self,
        args: &CalendarFreebusyQueryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FreeBusyResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = calendar_freebusy_query_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = calendar_freebusy_query_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Calendar settings get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Setting result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn calendar_settings_get(
        &self,
        args: &CalendarSettingsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Setting, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = calendar_settings_get_builder(
            &self.http_client,
            &args.setting,
        )
        .map_err(ProviderError::Api)?;

        let task = calendar_settings_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Calendar settings list.
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
    pub fn calendar_settings_list(
        &self,
        args: &CalendarSettingsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Settings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = calendar_settings_list_builder(
            &self.http_client,
            &args.maxResults,
            &args.pageToken,
            &args.syncToken,
        )
        .map_err(ProviderError::Api)?;

        let task = calendar_settings_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Calendar settings watch.
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
    pub fn calendar_settings_watch(
        &self,
        args: &CalendarSettingsWatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Channel, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = calendar_settings_watch_builder(
            &self.http_client,
            &args.maxResults,
            &args.pageToken,
            &args.syncToken,
        )
        .map_err(ProviderError::Api)?;

        let task = calendar_settings_watch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
