//! GmailProvider - State-aware gmail API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       gmail API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::gmail::{
    gmail_users_get_profile_builder, gmail_users_get_profile_task,
    gmail_users_stop_builder, gmail_users_stop_task,
    gmail_users_watch_builder, gmail_users_watch_task,
    gmail_users_drafts_create_builder, gmail_users_drafts_create_task,
    gmail_users_drafts_delete_builder, gmail_users_drafts_delete_task,
    gmail_users_drafts_get_builder, gmail_users_drafts_get_task,
    gmail_users_drafts_list_builder, gmail_users_drafts_list_task,
    gmail_users_drafts_send_builder, gmail_users_drafts_send_task,
    gmail_users_drafts_update_builder, gmail_users_drafts_update_task,
    gmail_users_history_list_builder, gmail_users_history_list_task,
    gmail_users_labels_create_builder, gmail_users_labels_create_task,
    gmail_users_labels_delete_builder, gmail_users_labels_delete_task,
    gmail_users_labels_get_builder, gmail_users_labels_get_task,
    gmail_users_labels_list_builder, gmail_users_labels_list_task,
    gmail_users_labels_patch_builder, gmail_users_labels_patch_task,
    gmail_users_labels_update_builder, gmail_users_labels_update_task,
    gmail_users_messages_batch_delete_builder, gmail_users_messages_batch_delete_task,
    gmail_users_messages_batch_modify_builder, gmail_users_messages_batch_modify_task,
    gmail_users_messages_delete_builder, gmail_users_messages_delete_task,
    gmail_users_messages_get_builder, gmail_users_messages_get_task,
    gmail_users_messages_import_builder, gmail_users_messages_import_task,
    gmail_users_messages_insert_builder, gmail_users_messages_insert_task,
    gmail_users_messages_list_builder, gmail_users_messages_list_task,
    gmail_users_messages_modify_builder, gmail_users_messages_modify_task,
    gmail_users_messages_send_builder, gmail_users_messages_send_task,
    gmail_users_messages_trash_builder, gmail_users_messages_trash_task,
    gmail_users_messages_untrash_builder, gmail_users_messages_untrash_task,
    gmail_users_messages_attachments_get_builder, gmail_users_messages_attachments_get_task,
    gmail_users_settings_get_auto_forwarding_builder, gmail_users_settings_get_auto_forwarding_task,
    gmail_users_settings_get_imap_builder, gmail_users_settings_get_imap_task,
    gmail_users_settings_get_language_builder, gmail_users_settings_get_language_task,
    gmail_users_settings_get_pop_builder, gmail_users_settings_get_pop_task,
    gmail_users_settings_get_vacation_builder, gmail_users_settings_get_vacation_task,
    gmail_users_settings_update_auto_forwarding_builder, gmail_users_settings_update_auto_forwarding_task,
    gmail_users_settings_update_imap_builder, gmail_users_settings_update_imap_task,
    gmail_users_settings_update_language_builder, gmail_users_settings_update_language_task,
    gmail_users_settings_update_pop_builder, gmail_users_settings_update_pop_task,
    gmail_users_settings_update_vacation_builder, gmail_users_settings_update_vacation_task,
    gmail_users_settings_cse_identities_create_builder, gmail_users_settings_cse_identities_create_task,
    gmail_users_settings_cse_identities_delete_builder, gmail_users_settings_cse_identities_delete_task,
    gmail_users_settings_cse_identities_get_builder, gmail_users_settings_cse_identities_get_task,
    gmail_users_settings_cse_identities_list_builder, gmail_users_settings_cse_identities_list_task,
    gmail_users_settings_cse_identities_patch_builder, gmail_users_settings_cse_identities_patch_task,
    gmail_users_settings_cse_keypairs_create_builder, gmail_users_settings_cse_keypairs_create_task,
    gmail_users_settings_cse_keypairs_disable_builder, gmail_users_settings_cse_keypairs_disable_task,
    gmail_users_settings_cse_keypairs_enable_builder, gmail_users_settings_cse_keypairs_enable_task,
    gmail_users_settings_cse_keypairs_get_builder, gmail_users_settings_cse_keypairs_get_task,
    gmail_users_settings_cse_keypairs_list_builder, gmail_users_settings_cse_keypairs_list_task,
    gmail_users_settings_cse_keypairs_obliterate_builder, gmail_users_settings_cse_keypairs_obliterate_task,
    gmail_users_settings_delegates_create_builder, gmail_users_settings_delegates_create_task,
    gmail_users_settings_delegates_delete_builder, gmail_users_settings_delegates_delete_task,
    gmail_users_settings_delegates_get_builder, gmail_users_settings_delegates_get_task,
    gmail_users_settings_delegates_list_builder, gmail_users_settings_delegates_list_task,
    gmail_users_settings_filters_create_builder, gmail_users_settings_filters_create_task,
    gmail_users_settings_filters_delete_builder, gmail_users_settings_filters_delete_task,
    gmail_users_settings_filters_get_builder, gmail_users_settings_filters_get_task,
    gmail_users_settings_filters_list_builder, gmail_users_settings_filters_list_task,
    gmail_users_settings_forwarding_addresses_create_builder, gmail_users_settings_forwarding_addresses_create_task,
    gmail_users_settings_forwarding_addresses_delete_builder, gmail_users_settings_forwarding_addresses_delete_task,
    gmail_users_settings_forwarding_addresses_get_builder, gmail_users_settings_forwarding_addresses_get_task,
    gmail_users_settings_forwarding_addresses_list_builder, gmail_users_settings_forwarding_addresses_list_task,
    gmail_users_settings_send_as_create_builder, gmail_users_settings_send_as_create_task,
    gmail_users_settings_send_as_delete_builder, gmail_users_settings_send_as_delete_task,
    gmail_users_settings_send_as_get_builder, gmail_users_settings_send_as_get_task,
    gmail_users_settings_send_as_list_builder, gmail_users_settings_send_as_list_task,
    gmail_users_settings_send_as_patch_builder, gmail_users_settings_send_as_patch_task,
    gmail_users_settings_send_as_update_builder, gmail_users_settings_send_as_update_task,
    gmail_users_settings_send_as_verify_builder, gmail_users_settings_send_as_verify_task,
    gmail_users_settings_send_as_smime_info_delete_builder, gmail_users_settings_send_as_smime_info_delete_task,
    gmail_users_settings_send_as_smime_info_get_builder, gmail_users_settings_send_as_smime_info_get_task,
    gmail_users_settings_send_as_smime_info_insert_builder, gmail_users_settings_send_as_smime_info_insert_task,
    gmail_users_settings_send_as_smime_info_list_builder, gmail_users_settings_send_as_smime_info_list_task,
    gmail_users_settings_send_as_smime_info_set_default_builder, gmail_users_settings_send_as_smime_info_set_default_task,
    gmail_users_threads_delete_builder, gmail_users_threads_delete_task,
    gmail_users_threads_get_builder, gmail_users_threads_get_task,
    gmail_users_threads_list_builder, gmail_users_threads_list_task,
    gmail_users_threads_modify_builder, gmail_users_threads_modify_task,
    gmail_users_threads_trash_builder, gmail_users_threads_trash_task,
    gmail_users_threads_untrash_builder, gmail_users_threads_untrash_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::gmail::AutoForwarding;
use crate::providers::gcp::clients::gmail::CseIdentity;
use crate::providers::gcp::clients::gmail::CseKeyPair;
use crate::providers::gcp::clients::gmail::Delegate;
use crate::providers::gcp::clients::gmail::Draft;
use crate::providers::gcp::clients::gmail::Filter;
use crate::providers::gcp::clients::gmail::ForwardingAddress;
use crate::providers::gcp::clients::gmail::ImapSettings;
use crate::providers::gcp::clients::gmail::Label;
use crate::providers::gcp::clients::gmail::LanguageSettings;
use crate::providers::gcp::clients::gmail::ListCseIdentitiesResponse;
use crate::providers::gcp::clients::gmail::ListCseKeyPairsResponse;
use crate::providers::gcp::clients::gmail::ListDelegatesResponse;
use crate::providers::gcp::clients::gmail::ListDraftsResponse;
use crate::providers::gcp::clients::gmail::ListFiltersResponse;
use crate::providers::gcp::clients::gmail::ListForwardingAddressesResponse;
use crate::providers::gcp::clients::gmail::ListHistoryResponse;
use crate::providers::gcp::clients::gmail::ListLabelsResponse;
use crate::providers::gcp::clients::gmail::ListMessagesResponse;
use crate::providers::gcp::clients::gmail::ListSendAsResponse;
use crate::providers::gcp::clients::gmail::ListSmimeInfoResponse;
use crate::providers::gcp::clients::gmail::ListThreadsResponse;
use crate::providers::gcp::clients::gmail::Message;
use crate::providers::gcp::clients::gmail::MessagePartBody;
use crate::providers::gcp::clients::gmail::PopSettings;
use crate::providers::gcp::clients::gmail::Profile;
use crate::providers::gcp::clients::gmail::SendAs;
use crate::providers::gcp::clients::gmail::SmimeInfo;
use crate::providers::gcp::clients::gmail::Thread;
use crate::providers::gcp::clients::gmail::VacationSettings;
use crate::providers::gcp::clients::gmail::WatchResponse;
use crate::providers::gcp::clients::gmail::GmailUsersDraftsCreateArgs;
use crate::providers::gcp::clients::gmail::GmailUsersDraftsDeleteArgs;
use crate::providers::gcp::clients::gmail::GmailUsersDraftsGetArgs;
use crate::providers::gcp::clients::gmail::GmailUsersDraftsListArgs;
use crate::providers::gcp::clients::gmail::GmailUsersDraftsSendArgs;
use crate::providers::gcp::clients::gmail::GmailUsersDraftsUpdateArgs;
use crate::providers::gcp::clients::gmail::GmailUsersGetProfileArgs;
use crate::providers::gcp::clients::gmail::GmailUsersHistoryListArgs;
use crate::providers::gcp::clients::gmail::GmailUsersLabelsCreateArgs;
use crate::providers::gcp::clients::gmail::GmailUsersLabelsDeleteArgs;
use crate::providers::gcp::clients::gmail::GmailUsersLabelsGetArgs;
use crate::providers::gcp::clients::gmail::GmailUsersLabelsListArgs;
use crate::providers::gcp::clients::gmail::GmailUsersLabelsPatchArgs;
use crate::providers::gcp::clients::gmail::GmailUsersLabelsUpdateArgs;
use crate::providers::gcp::clients::gmail::GmailUsersMessagesAttachmentsGetArgs;
use crate::providers::gcp::clients::gmail::GmailUsersMessagesBatchDeleteArgs;
use crate::providers::gcp::clients::gmail::GmailUsersMessagesBatchModifyArgs;
use crate::providers::gcp::clients::gmail::GmailUsersMessagesDeleteArgs;
use crate::providers::gcp::clients::gmail::GmailUsersMessagesGetArgs;
use crate::providers::gcp::clients::gmail::GmailUsersMessagesImportArgs;
use crate::providers::gcp::clients::gmail::GmailUsersMessagesInsertArgs;
use crate::providers::gcp::clients::gmail::GmailUsersMessagesListArgs;
use crate::providers::gcp::clients::gmail::GmailUsersMessagesModifyArgs;
use crate::providers::gcp::clients::gmail::GmailUsersMessagesSendArgs;
use crate::providers::gcp::clients::gmail::GmailUsersMessagesTrashArgs;
use crate::providers::gcp::clients::gmail::GmailUsersMessagesUntrashArgs;
use crate::providers::gcp::clients::gmail::GmailUsersSettingsCseIdentitiesCreateArgs;
use crate::providers::gcp::clients::gmail::GmailUsersSettingsCseIdentitiesDeleteArgs;
use crate::providers::gcp::clients::gmail::GmailUsersSettingsCseIdentitiesGetArgs;
use crate::providers::gcp::clients::gmail::GmailUsersSettingsCseIdentitiesListArgs;
use crate::providers::gcp::clients::gmail::GmailUsersSettingsCseIdentitiesPatchArgs;
use crate::providers::gcp::clients::gmail::GmailUsersSettingsCseKeypairsCreateArgs;
use crate::providers::gcp::clients::gmail::GmailUsersSettingsCseKeypairsDisableArgs;
use crate::providers::gcp::clients::gmail::GmailUsersSettingsCseKeypairsEnableArgs;
use crate::providers::gcp::clients::gmail::GmailUsersSettingsCseKeypairsGetArgs;
use crate::providers::gcp::clients::gmail::GmailUsersSettingsCseKeypairsListArgs;
use crate::providers::gcp::clients::gmail::GmailUsersSettingsCseKeypairsObliterateArgs;
use crate::providers::gcp::clients::gmail::GmailUsersSettingsDelegatesCreateArgs;
use crate::providers::gcp::clients::gmail::GmailUsersSettingsDelegatesDeleteArgs;
use crate::providers::gcp::clients::gmail::GmailUsersSettingsDelegatesGetArgs;
use crate::providers::gcp::clients::gmail::GmailUsersSettingsDelegatesListArgs;
use crate::providers::gcp::clients::gmail::GmailUsersSettingsFiltersCreateArgs;
use crate::providers::gcp::clients::gmail::GmailUsersSettingsFiltersDeleteArgs;
use crate::providers::gcp::clients::gmail::GmailUsersSettingsFiltersGetArgs;
use crate::providers::gcp::clients::gmail::GmailUsersSettingsFiltersListArgs;
use crate::providers::gcp::clients::gmail::GmailUsersSettingsForwardingAddressesCreateArgs;
use crate::providers::gcp::clients::gmail::GmailUsersSettingsForwardingAddressesDeleteArgs;
use crate::providers::gcp::clients::gmail::GmailUsersSettingsForwardingAddressesGetArgs;
use crate::providers::gcp::clients::gmail::GmailUsersSettingsForwardingAddressesListArgs;
use crate::providers::gcp::clients::gmail::GmailUsersSettingsGetAutoForwardingArgs;
use crate::providers::gcp::clients::gmail::GmailUsersSettingsGetImapArgs;
use crate::providers::gcp::clients::gmail::GmailUsersSettingsGetLanguageArgs;
use crate::providers::gcp::clients::gmail::GmailUsersSettingsGetPopArgs;
use crate::providers::gcp::clients::gmail::GmailUsersSettingsGetVacationArgs;
use crate::providers::gcp::clients::gmail::GmailUsersSettingsSendAsCreateArgs;
use crate::providers::gcp::clients::gmail::GmailUsersSettingsSendAsDeleteArgs;
use crate::providers::gcp::clients::gmail::GmailUsersSettingsSendAsGetArgs;
use crate::providers::gcp::clients::gmail::GmailUsersSettingsSendAsListArgs;
use crate::providers::gcp::clients::gmail::GmailUsersSettingsSendAsPatchArgs;
use crate::providers::gcp::clients::gmail::GmailUsersSettingsSendAsSmimeInfoDeleteArgs;
use crate::providers::gcp::clients::gmail::GmailUsersSettingsSendAsSmimeInfoGetArgs;
use crate::providers::gcp::clients::gmail::GmailUsersSettingsSendAsSmimeInfoInsertArgs;
use crate::providers::gcp::clients::gmail::GmailUsersSettingsSendAsSmimeInfoListArgs;
use crate::providers::gcp::clients::gmail::GmailUsersSettingsSendAsSmimeInfoSetDefaultArgs;
use crate::providers::gcp::clients::gmail::GmailUsersSettingsSendAsUpdateArgs;
use crate::providers::gcp::clients::gmail::GmailUsersSettingsSendAsVerifyArgs;
use crate::providers::gcp::clients::gmail::GmailUsersSettingsUpdateAutoForwardingArgs;
use crate::providers::gcp::clients::gmail::GmailUsersSettingsUpdateImapArgs;
use crate::providers::gcp::clients::gmail::GmailUsersSettingsUpdateLanguageArgs;
use crate::providers::gcp::clients::gmail::GmailUsersSettingsUpdatePopArgs;
use crate::providers::gcp::clients::gmail::GmailUsersSettingsUpdateVacationArgs;
use crate::providers::gcp::clients::gmail::GmailUsersStopArgs;
use crate::providers::gcp::clients::gmail::GmailUsersThreadsDeleteArgs;
use crate::providers::gcp::clients::gmail::GmailUsersThreadsGetArgs;
use crate::providers::gcp::clients::gmail::GmailUsersThreadsListArgs;
use crate::providers::gcp::clients::gmail::GmailUsersThreadsModifyArgs;
use crate::providers::gcp::clients::gmail::GmailUsersThreadsTrashArgs;
use crate::providers::gcp::clients::gmail::GmailUsersThreadsUntrashArgs;
use crate::providers::gcp::clients::gmail::GmailUsersWatchArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// GmailProvider with automatic state tracking.
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
/// let provider = GmailProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct GmailProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> GmailProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new GmailProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Gmail users get profile.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Profile result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gmail_users_get_profile(
        &self,
        args: &GmailUsersGetProfileArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Profile, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_get_profile_builder(
            &self.http_client,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_get_profile_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users stop.
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
    pub fn gmail_users_stop(
        &self,
        args: &GmailUsersStopArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_stop_builder(
            &self.http_client,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_stop_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users watch.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WatchResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gmail_users_watch(
        &self,
        args: &GmailUsersWatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WatchResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_watch_builder(
            &self.http_client,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_watch_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users drafts create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Draft result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gmail_users_drafts_create(
        &self,
        args: &GmailUsersDraftsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Draft, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_drafts_create_builder(
            &self.http_client,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_drafts_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users drafts delete.
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
    pub fn gmail_users_drafts_delete(
        &self,
        args: &GmailUsersDraftsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_drafts_delete_builder(
            &self.http_client,
            &args.userId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_drafts_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users drafts get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Draft result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gmail_users_drafts_get(
        &self,
        args: &GmailUsersDraftsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Draft, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_drafts_get_builder(
            &self.http_client,
            &args.userId,
            &args.id,
            &args.format,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_drafts_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users drafts list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDraftsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gmail_users_drafts_list(
        &self,
        args: &GmailUsersDraftsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDraftsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_drafts_list_builder(
            &self.http_client,
            &args.userId,
            &args.includeSpamTrash,
            &args.maxResults,
            &args.pageToken,
            &args.q,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_drafts_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users drafts send.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Message result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gmail_users_drafts_send(
        &self,
        args: &GmailUsersDraftsSendArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Message, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_drafts_send_builder(
            &self.http_client,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_drafts_send_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users drafts update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Draft result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gmail_users_drafts_update(
        &self,
        args: &GmailUsersDraftsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Draft, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_drafts_update_builder(
            &self.http_client,
            &args.userId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_drafts_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users history list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListHistoryResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gmail_users_history_list(
        &self,
        args: &GmailUsersHistoryListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListHistoryResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_history_list_builder(
            &self.http_client,
            &args.userId,
            &args.historyTypes,
            &args.labelId,
            &args.maxResults,
            &args.pageToken,
            &args.startHistoryId,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_history_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users labels create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Label result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gmail_users_labels_create(
        &self,
        args: &GmailUsersLabelsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Label, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_labels_create_builder(
            &self.http_client,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_labels_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users labels delete.
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
    pub fn gmail_users_labels_delete(
        &self,
        args: &GmailUsersLabelsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_labels_delete_builder(
            &self.http_client,
            &args.userId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_labels_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users labels get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Label result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gmail_users_labels_get(
        &self,
        args: &GmailUsersLabelsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Label, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_labels_get_builder(
            &self.http_client,
            &args.userId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_labels_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users labels list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListLabelsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gmail_users_labels_list(
        &self,
        args: &GmailUsersLabelsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLabelsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_labels_list_builder(
            &self.http_client,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_labels_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users labels patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Label result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gmail_users_labels_patch(
        &self,
        args: &GmailUsersLabelsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Label, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_labels_patch_builder(
            &self.http_client,
            &args.userId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_labels_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users labels update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Label result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gmail_users_labels_update(
        &self,
        args: &GmailUsersLabelsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Label, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_labels_update_builder(
            &self.http_client,
            &args.userId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_labels_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users messages batch delete.
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
    pub fn gmail_users_messages_batch_delete(
        &self,
        args: &GmailUsersMessagesBatchDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_messages_batch_delete_builder(
            &self.http_client,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_messages_batch_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users messages batch modify.
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
    pub fn gmail_users_messages_batch_modify(
        &self,
        args: &GmailUsersMessagesBatchModifyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_messages_batch_modify_builder(
            &self.http_client,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_messages_batch_modify_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users messages delete.
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
    pub fn gmail_users_messages_delete(
        &self,
        args: &GmailUsersMessagesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_messages_delete_builder(
            &self.http_client,
            &args.userId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_messages_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users messages get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Message result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gmail_users_messages_get(
        &self,
        args: &GmailUsersMessagesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Message, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_messages_get_builder(
            &self.http_client,
            &args.userId,
            &args.id,
            &args.format,
            &args.metadataHeaders,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_messages_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users messages import.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Message result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gmail_users_messages_import(
        &self,
        args: &GmailUsersMessagesImportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Message, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_messages_import_builder(
            &self.http_client,
            &args.userId,
            &args.deleted,
            &args.internalDateSource,
            &args.neverMarkSpam,
            &args.processForCalendar,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_messages_import_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users messages insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Message result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gmail_users_messages_insert(
        &self,
        args: &GmailUsersMessagesInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Message, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_messages_insert_builder(
            &self.http_client,
            &args.userId,
            &args.deleted,
            &args.internalDateSource,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_messages_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users messages list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListMessagesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gmail_users_messages_list(
        &self,
        args: &GmailUsersMessagesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListMessagesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_messages_list_builder(
            &self.http_client,
            &args.userId,
            &args.includeSpamTrash,
            &args.labelIds,
            &args.maxResults,
            &args.pageToken,
            &args.q,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_messages_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users messages modify.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Message result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gmail_users_messages_modify(
        &self,
        args: &GmailUsersMessagesModifyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Message, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_messages_modify_builder(
            &self.http_client,
            &args.userId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_messages_modify_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users messages send.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Message result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gmail_users_messages_send(
        &self,
        args: &GmailUsersMessagesSendArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Message, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_messages_send_builder(
            &self.http_client,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_messages_send_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users messages trash.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Message result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gmail_users_messages_trash(
        &self,
        args: &GmailUsersMessagesTrashArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Message, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_messages_trash_builder(
            &self.http_client,
            &args.userId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_messages_trash_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users messages untrash.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Message result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gmail_users_messages_untrash(
        &self,
        args: &GmailUsersMessagesUntrashArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Message, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_messages_untrash_builder(
            &self.http_client,
            &args.userId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_messages_untrash_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users messages attachments get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the MessagePartBody result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gmail_users_messages_attachments_get(
        &self,
        args: &GmailUsersMessagesAttachmentsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<MessagePartBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_messages_attachments_get_builder(
            &self.http_client,
            &args.userId,
            &args.messageId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_messages_attachments_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users settings get auto forwarding.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AutoForwarding result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gmail_users_settings_get_auto_forwarding(
        &self,
        args: &GmailUsersSettingsGetAutoForwardingArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AutoForwarding, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_settings_get_auto_forwarding_builder(
            &self.http_client,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_settings_get_auto_forwarding_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users settings get imap.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ImapSettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gmail_users_settings_get_imap(
        &self,
        args: &GmailUsersSettingsGetImapArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ImapSettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_settings_get_imap_builder(
            &self.http_client,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_settings_get_imap_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users settings get language.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LanguageSettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gmail_users_settings_get_language(
        &self,
        args: &GmailUsersSettingsGetLanguageArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LanguageSettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_settings_get_language_builder(
            &self.http_client,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_settings_get_language_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users settings get pop.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PopSettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gmail_users_settings_get_pop(
        &self,
        args: &GmailUsersSettingsGetPopArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PopSettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_settings_get_pop_builder(
            &self.http_client,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_settings_get_pop_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users settings get vacation.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the VacationSettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gmail_users_settings_get_vacation(
        &self,
        args: &GmailUsersSettingsGetVacationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<VacationSettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_settings_get_vacation_builder(
            &self.http_client,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_settings_get_vacation_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users settings update auto forwarding.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AutoForwarding result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gmail_users_settings_update_auto_forwarding(
        &self,
        args: &GmailUsersSettingsUpdateAutoForwardingArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AutoForwarding, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_settings_update_auto_forwarding_builder(
            &self.http_client,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_settings_update_auto_forwarding_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users settings update imap.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ImapSettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gmail_users_settings_update_imap(
        &self,
        args: &GmailUsersSettingsUpdateImapArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ImapSettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_settings_update_imap_builder(
            &self.http_client,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_settings_update_imap_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users settings update language.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LanguageSettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gmail_users_settings_update_language(
        &self,
        args: &GmailUsersSettingsUpdateLanguageArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LanguageSettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_settings_update_language_builder(
            &self.http_client,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_settings_update_language_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users settings update pop.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PopSettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gmail_users_settings_update_pop(
        &self,
        args: &GmailUsersSettingsUpdatePopArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PopSettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_settings_update_pop_builder(
            &self.http_client,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_settings_update_pop_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users settings update vacation.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the VacationSettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gmail_users_settings_update_vacation(
        &self,
        args: &GmailUsersSettingsUpdateVacationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<VacationSettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_settings_update_vacation_builder(
            &self.http_client,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_settings_update_vacation_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users settings cse identities create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CseIdentity result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gmail_users_settings_cse_identities_create(
        &self,
        args: &GmailUsersSettingsCseIdentitiesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CseIdentity, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_settings_cse_identities_create_builder(
            &self.http_client,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_settings_cse_identities_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users settings cse identities delete.
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
    pub fn gmail_users_settings_cse_identities_delete(
        &self,
        args: &GmailUsersSettingsCseIdentitiesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_settings_cse_identities_delete_builder(
            &self.http_client,
            &args.userId,
            &args.cseEmailAddress,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_settings_cse_identities_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users settings cse identities get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CseIdentity result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gmail_users_settings_cse_identities_get(
        &self,
        args: &GmailUsersSettingsCseIdentitiesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CseIdentity, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_settings_cse_identities_get_builder(
            &self.http_client,
            &args.userId,
            &args.cseEmailAddress,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_settings_cse_identities_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users settings cse identities list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCseIdentitiesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gmail_users_settings_cse_identities_list(
        &self,
        args: &GmailUsersSettingsCseIdentitiesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCseIdentitiesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_settings_cse_identities_list_builder(
            &self.http_client,
            &args.userId,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_settings_cse_identities_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users settings cse identities patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CseIdentity result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gmail_users_settings_cse_identities_patch(
        &self,
        args: &GmailUsersSettingsCseIdentitiesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CseIdentity, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_settings_cse_identities_patch_builder(
            &self.http_client,
            &args.userId,
            &args.emailAddress,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_settings_cse_identities_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users settings cse keypairs create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CseKeyPair result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gmail_users_settings_cse_keypairs_create(
        &self,
        args: &GmailUsersSettingsCseKeypairsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CseKeyPair, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_settings_cse_keypairs_create_builder(
            &self.http_client,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_settings_cse_keypairs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users settings cse keypairs disable.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CseKeyPair result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gmail_users_settings_cse_keypairs_disable(
        &self,
        args: &GmailUsersSettingsCseKeypairsDisableArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CseKeyPair, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_settings_cse_keypairs_disable_builder(
            &self.http_client,
            &args.userId,
            &args.keyPairId,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_settings_cse_keypairs_disable_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users settings cse keypairs enable.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CseKeyPair result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gmail_users_settings_cse_keypairs_enable(
        &self,
        args: &GmailUsersSettingsCseKeypairsEnableArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CseKeyPair, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_settings_cse_keypairs_enable_builder(
            &self.http_client,
            &args.userId,
            &args.keyPairId,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_settings_cse_keypairs_enable_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users settings cse keypairs get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CseKeyPair result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gmail_users_settings_cse_keypairs_get(
        &self,
        args: &GmailUsersSettingsCseKeypairsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CseKeyPair, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_settings_cse_keypairs_get_builder(
            &self.http_client,
            &args.userId,
            &args.keyPairId,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_settings_cse_keypairs_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users settings cse keypairs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCseKeyPairsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gmail_users_settings_cse_keypairs_list(
        &self,
        args: &GmailUsersSettingsCseKeypairsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCseKeyPairsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_settings_cse_keypairs_list_builder(
            &self.http_client,
            &args.userId,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_settings_cse_keypairs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users settings cse keypairs obliterate.
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
    pub fn gmail_users_settings_cse_keypairs_obliterate(
        &self,
        args: &GmailUsersSettingsCseKeypairsObliterateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_settings_cse_keypairs_obliterate_builder(
            &self.http_client,
            &args.userId,
            &args.keyPairId,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_settings_cse_keypairs_obliterate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users settings delegates create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Delegate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gmail_users_settings_delegates_create(
        &self,
        args: &GmailUsersSettingsDelegatesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Delegate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_settings_delegates_create_builder(
            &self.http_client,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_settings_delegates_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users settings delegates delete.
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
    pub fn gmail_users_settings_delegates_delete(
        &self,
        args: &GmailUsersSettingsDelegatesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_settings_delegates_delete_builder(
            &self.http_client,
            &args.userId,
            &args.delegateEmail,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_settings_delegates_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users settings delegates get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Delegate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gmail_users_settings_delegates_get(
        &self,
        args: &GmailUsersSettingsDelegatesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Delegate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_settings_delegates_get_builder(
            &self.http_client,
            &args.userId,
            &args.delegateEmail,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_settings_delegates_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users settings delegates list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDelegatesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gmail_users_settings_delegates_list(
        &self,
        args: &GmailUsersSettingsDelegatesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDelegatesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_settings_delegates_list_builder(
            &self.http_client,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_settings_delegates_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users settings filters create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Filter result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gmail_users_settings_filters_create(
        &self,
        args: &GmailUsersSettingsFiltersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Filter, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_settings_filters_create_builder(
            &self.http_client,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_settings_filters_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users settings filters delete.
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
    pub fn gmail_users_settings_filters_delete(
        &self,
        args: &GmailUsersSettingsFiltersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_settings_filters_delete_builder(
            &self.http_client,
            &args.userId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_settings_filters_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users settings filters get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Filter result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gmail_users_settings_filters_get(
        &self,
        args: &GmailUsersSettingsFiltersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Filter, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_settings_filters_get_builder(
            &self.http_client,
            &args.userId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_settings_filters_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users settings filters list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListFiltersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gmail_users_settings_filters_list(
        &self,
        args: &GmailUsersSettingsFiltersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListFiltersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_settings_filters_list_builder(
            &self.http_client,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_settings_filters_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users settings forwarding addresses create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ForwardingAddress result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gmail_users_settings_forwarding_addresses_create(
        &self,
        args: &GmailUsersSettingsForwardingAddressesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ForwardingAddress, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_settings_forwarding_addresses_create_builder(
            &self.http_client,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_settings_forwarding_addresses_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users settings forwarding addresses delete.
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
    pub fn gmail_users_settings_forwarding_addresses_delete(
        &self,
        args: &GmailUsersSettingsForwardingAddressesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_settings_forwarding_addresses_delete_builder(
            &self.http_client,
            &args.userId,
            &args.forwardingEmail,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_settings_forwarding_addresses_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users settings forwarding addresses get.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ForwardingAddress result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gmail_users_settings_forwarding_addresses_get(
        &self,
        args: &GmailUsersSettingsForwardingAddressesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ForwardingAddress, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_settings_forwarding_addresses_get_builder(
            &self.http_client,
            &args.userId,
            &args.forwardingEmail,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_settings_forwarding_addresses_get_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users settings forwarding addresses list.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListForwardingAddressesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gmail_users_settings_forwarding_addresses_list(
        &self,
        args: &GmailUsersSettingsForwardingAddressesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListForwardingAddressesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_settings_forwarding_addresses_list_builder(
            &self.http_client,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_settings_forwarding_addresses_list_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users settings send as create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SendAs result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gmail_users_settings_send_as_create(
        &self,
        args: &GmailUsersSettingsSendAsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SendAs, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_settings_send_as_create_builder(
            &self.http_client,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_settings_send_as_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users settings send as delete.
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
    pub fn gmail_users_settings_send_as_delete(
        &self,
        args: &GmailUsersSettingsSendAsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_settings_send_as_delete_builder(
            &self.http_client,
            &args.userId,
            &args.sendAsEmail,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_settings_send_as_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users settings send as get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SendAs result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gmail_users_settings_send_as_get(
        &self,
        args: &GmailUsersSettingsSendAsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SendAs, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_settings_send_as_get_builder(
            &self.http_client,
            &args.userId,
            &args.sendAsEmail,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_settings_send_as_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users settings send as list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListSendAsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gmail_users_settings_send_as_list(
        &self,
        args: &GmailUsersSettingsSendAsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSendAsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_settings_send_as_list_builder(
            &self.http_client,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_settings_send_as_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users settings send as patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SendAs result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gmail_users_settings_send_as_patch(
        &self,
        args: &GmailUsersSettingsSendAsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SendAs, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_settings_send_as_patch_builder(
            &self.http_client,
            &args.userId,
            &args.sendAsEmail,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_settings_send_as_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users settings send as update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SendAs result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gmail_users_settings_send_as_update(
        &self,
        args: &GmailUsersSettingsSendAsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SendAs, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_settings_send_as_update_builder(
            &self.http_client,
            &args.userId,
            &args.sendAsEmail,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_settings_send_as_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users settings send as verify.
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
    pub fn gmail_users_settings_send_as_verify(
        &self,
        args: &GmailUsersSettingsSendAsVerifyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_settings_send_as_verify_builder(
            &self.http_client,
            &args.userId,
            &args.sendAsEmail,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_settings_send_as_verify_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users settings send as smime info delete.
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
    pub fn gmail_users_settings_send_as_smime_info_delete(
        &self,
        args: &GmailUsersSettingsSendAsSmimeInfoDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_settings_send_as_smime_info_delete_builder(
            &self.http_client,
            &args.userId,
            &args.sendAsEmail,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_settings_send_as_smime_info_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users settings send as smime info get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SmimeInfo result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gmail_users_settings_send_as_smime_info_get(
        &self,
        args: &GmailUsersSettingsSendAsSmimeInfoGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SmimeInfo, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_settings_send_as_smime_info_get_builder(
            &self.http_client,
            &args.userId,
            &args.sendAsEmail,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_settings_send_as_smime_info_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users settings send as smime info insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SmimeInfo result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gmail_users_settings_send_as_smime_info_insert(
        &self,
        args: &GmailUsersSettingsSendAsSmimeInfoInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SmimeInfo, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_settings_send_as_smime_info_insert_builder(
            &self.http_client,
            &args.userId,
            &args.sendAsEmail,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_settings_send_as_smime_info_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users settings send as smime info list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListSmimeInfoResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gmail_users_settings_send_as_smime_info_list(
        &self,
        args: &GmailUsersSettingsSendAsSmimeInfoListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSmimeInfoResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_settings_send_as_smime_info_list_builder(
            &self.http_client,
            &args.userId,
            &args.sendAsEmail,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_settings_send_as_smime_info_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users settings send as smime info set default.
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
    pub fn gmail_users_settings_send_as_smime_info_set_default(
        &self,
        args: &GmailUsersSettingsSendAsSmimeInfoSetDefaultArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_settings_send_as_smime_info_set_default_builder(
            &self.http_client,
            &args.userId,
            &args.sendAsEmail,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_settings_send_as_smime_info_set_default_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users threads delete.
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
    pub fn gmail_users_threads_delete(
        &self,
        args: &GmailUsersThreadsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_threads_delete_builder(
            &self.http_client,
            &args.userId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_threads_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users threads get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Thread result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gmail_users_threads_get(
        &self,
        args: &GmailUsersThreadsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Thread, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_threads_get_builder(
            &self.http_client,
            &args.userId,
            &args.id,
            &args.format,
            &args.metadataHeaders,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_threads_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users threads list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListThreadsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gmail_users_threads_list(
        &self,
        args: &GmailUsersThreadsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListThreadsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_threads_list_builder(
            &self.http_client,
            &args.userId,
            &args.includeSpamTrash,
            &args.labelIds,
            &args.maxResults,
            &args.pageToken,
            &args.q,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_threads_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users threads modify.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Thread result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gmail_users_threads_modify(
        &self,
        args: &GmailUsersThreadsModifyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Thread, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_threads_modify_builder(
            &self.http_client,
            &args.userId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_threads_modify_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users threads trash.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Thread result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gmail_users_threads_trash(
        &self,
        args: &GmailUsersThreadsTrashArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Thread, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_threads_trash_builder(
            &self.http_client,
            &args.userId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_threads_trash_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmail users threads untrash.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Thread result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gmail_users_threads_untrash(
        &self,
        args: &GmailUsersThreadsUntrashArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Thread, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmail_users_threads_untrash_builder(
            &self.http_client,
            &args.userId,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = gmail_users_threads_untrash_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
