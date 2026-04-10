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
    calendar_acl_insert_builder, calendar_acl_insert_task,
    calendar_acl_patch_builder, calendar_acl_patch_task,
    calendar_acl_update_builder, calendar_acl_update_task,
    calendar_acl_watch_builder, calendar_acl_watch_task,
    calendar_calendar_list_delete_builder, calendar_calendar_list_delete_task,
    calendar_calendar_list_insert_builder, calendar_calendar_list_insert_task,
    calendar_calendar_list_patch_builder, calendar_calendar_list_patch_task,
    calendar_calendar_list_update_builder, calendar_calendar_list_update_task,
    calendar_calendar_list_watch_builder, calendar_calendar_list_watch_task,
    calendar_calendars_clear_builder, calendar_calendars_clear_task,
    calendar_calendars_delete_builder, calendar_calendars_delete_task,
    calendar_calendars_insert_builder, calendar_calendars_insert_task,
    calendar_calendars_patch_builder, calendar_calendars_patch_task,
    calendar_calendars_update_builder, calendar_calendars_update_task,
    calendar_channels_stop_builder, calendar_channels_stop_task,
    calendar_events_delete_builder, calendar_events_delete_task,
    calendar_events_import_builder, calendar_events_import_task,
    calendar_events_insert_builder, calendar_events_insert_task,
    calendar_events_move_builder, calendar_events_move_task,
    calendar_events_patch_builder, calendar_events_patch_task,
    calendar_events_quick_add_builder, calendar_events_quick_add_task,
    calendar_events_update_builder, calendar_events_update_task,
    calendar_events_watch_builder, calendar_events_watch_task,
    calendar_freebusy_query_builder, calendar_freebusy_query_task,
    calendar_settings_watch_builder, calendar_settings_watch_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::calendar::AclRule;
use crate::providers::gcp::clients::calendar::Calendar;
use crate::providers::gcp::clients::calendar::CalendarListEntry;
use crate::providers::gcp::clients::calendar::Channel;
use crate::providers::gcp::clients::calendar::Event;
use crate::providers::gcp::clients::calendar::FreeBusyResponse;
use crate::providers::gcp::clients::calendar::CalendarAclDeleteArgs;
use crate::providers::gcp::clients::calendar::CalendarAclInsertArgs;
use crate::providers::gcp::clients::calendar::CalendarAclPatchArgs;
use crate::providers::gcp::clients::calendar::CalendarAclUpdateArgs;
use crate::providers::gcp::clients::calendar::CalendarAclWatchArgs;
use crate::providers::gcp::clients::calendar::CalendarCalendarListDeleteArgs;
use crate::providers::gcp::clients::calendar::CalendarCalendarListInsertArgs;
use crate::providers::gcp::clients::calendar::CalendarCalendarListPatchArgs;
use crate::providers::gcp::clients::calendar::CalendarCalendarListUpdateArgs;
use crate::providers::gcp::clients::calendar::CalendarCalendarListWatchArgs;
use crate::providers::gcp::clients::calendar::CalendarCalendarsClearArgs;
use crate::providers::gcp::clients::calendar::CalendarCalendarsDeleteArgs;
use crate::providers::gcp::clients::calendar::CalendarCalendarsInsertArgs;
use crate::providers::gcp::clients::calendar::CalendarCalendarsPatchArgs;
use crate::providers::gcp::clients::calendar::CalendarCalendarsUpdateArgs;
use crate::providers::gcp::clients::calendar::CalendarChannelsStopArgs;
use crate::providers::gcp::clients::calendar::CalendarEventsDeleteArgs;
use crate::providers::gcp::clients::calendar::CalendarEventsImportArgs;
use crate::providers::gcp::clients::calendar::CalendarEventsInsertArgs;
use crate::providers::gcp::clients::calendar::CalendarEventsMoveArgs;
use crate::providers::gcp::clients::calendar::CalendarEventsPatchArgs;
use crate::providers::gcp::clients::calendar::CalendarEventsQuickAddArgs;
use crate::providers::gcp::clients::calendar::CalendarEventsUpdateArgs;
use crate::providers::gcp::clients::calendar::CalendarEventsWatchArgs;
use crate::providers::gcp::clients::calendar::CalendarFreebusyQueryArgs;
use crate::providers::gcp::clients::calendar::CalendarSettingsWatchArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// CalendarProvider with automatic state tracking.
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
/// let provider = CalendarProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct CalendarProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> CalendarProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new CalendarProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Calendar calendar list delete.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Calendar calendar list patch.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Calendar calendar list update.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Calendar calendar list watch.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
            &args.text,
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Calendar freebusy query.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
