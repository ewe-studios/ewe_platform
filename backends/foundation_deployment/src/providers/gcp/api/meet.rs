//! MeetProvider - State-aware meet API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       meet API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::meet::{
    meet_conference_records_get_builder, meet_conference_records_get_task,
    meet_conference_records_list_builder, meet_conference_records_list_task,
    meet_conference_records_participants_get_builder, meet_conference_records_participants_get_task,
    meet_conference_records_participants_list_builder, meet_conference_records_participants_list_task,
    meet_conference_records_participants_participant_sessions_get_builder, meet_conference_records_participants_participant_sessions_get_task,
    meet_conference_records_participants_participant_sessions_list_builder, meet_conference_records_participants_participant_sessions_list_task,
    meet_conference_records_recordings_get_builder, meet_conference_records_recordings_get_task,
    meet_conference_records_recordings_list_builder, meet_conference_records_recordings_list_task,
    meet_conference_records_smart_notes_get_builder, meet_conference_records_smart_notes_get_task,
    meet_conference_records_smart_notes_list_builder, meet_conference_records_smart_notes_list_task,
    meet_conference_records_transcripts_get_builder, meet_conference_records_transcripts_get_task,
    meet_conference_records_transcripts_list_builder, meet_conference_records_transcripts_list_task,
    meet_conference_records_transcripts_entries_get_builder, meet_conference_records_transcripts_entries_get_task,
    meet_conference_records_transcripts_entries_list_builder, meet_conference_records_transcripts_entries_list_task,
    meet_spaces_create_builder, meet_spaces_create_task,
    meet_spaces_end_active_conference_builder, meet_spaces_end_active_conference_task,
    meet_spaces_get_builder, meet_spaces_get_task,
    meet_spaces_patch_builder, meet_spaces_patch_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::meet::ConferenceRecord;
use crate::providers::gcp::clients::meet::Empty;
use crate::providers::gcp::clients::meet::ListConferenceRecordsResponse;
use crate::providers::gcp::clients::meet::ListParticipantSessionsResponse;
use crate::providers::gcp::clients::meet::ListParticipantsResponse;
use crate::providers::gcp::clients::meet::ListRecordingsResponse;
use crate::providers::gcp::clients::meet::ListSmartNotesResponse;
use crate::providers::gcp::clients::meet::ListTranscriptEntriesResponse;
use crate::providers::gcp::clients::meet::ListTranscriptsResponse;
use crate::providers::gcp::clients::meet::Participant;
use crate::providers::gcp::clients::meet::ParticipantSession;
use crate::providers::gcp::clients::meet::Recording;
use crate::providers::gcp::clients::meet::SmartNote;
use crate::providers::gcp::clients::meet::Space;
use crate::providers::gcp::clients::meet::Transcript;
use crate::providers::gcp::clients::meet::TranscriptEntry;
use crate::providers::gcp::clients::meet::MeetConferenceRecordsGetArgs;
use crate::providers::gcp::clients::meet::MeetConferenceRecordsListArgs;
use crate::providers::gcp::clients::meet::MeetConferenceRecordsParticipantsGetArgs;
use crate::providers::gcp::clients::meet::MeetConferenceRecordsParticipantsListArgs;
use crate::providers::gcp::clients::meet::MeetConferenceRecordsParticipantsParticipantSessionsGetArgs;
use crate::providers::gcp::clients::meet::MeetConferenceRecordsParticipantsParticipantSessionsListArgs;
use crate::providers::gcp::clients::meet::MeetConferenceRecordsRecordingsGetArgs;
use crate::providers::gcp::clients::meet::MeetConferenceRecordsRecordingsListArgs;
use crate::providers::gcp::clients::meet::MeetConferenceRecordsSmartNotesGetArgs;
use crate::providers::gcp::clients::meet::MeetConferenceRecordsSmartNotesListArgs;
use crate::providers::gcp::clients::meet::MeetConferenceRecordsTranscriptsEntriesGetArgs;
use crate::providers::gcp::clients::meet::MeetConferenceRecordsTranscriptsEntriesListArgs;
use crate::providers::gcp::clients::meet::MeetConferenceRecordsTranscriptsGetArgs;
use crate::providers::gcp::clients::meet::MeetConferenceRecordsTranscriptsListArgs;
use crate::providers::gcp::clients::meet::MeetSpacesEndActiveConferenceArgs;
use crate::providers::gcp::clients::meet::MeetSpacesGetArgs;
use crate::providers::gcp::clients::meet::MeetSpacesPatchArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// MeetProvider with automatic state tracking.
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
/// let provider = MeetProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct MeetProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> MeetProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new MeetProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new MeetProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Meet conference records get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ConferenceRecord result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn meet_conference_records_get(
        &self,
        args: &MeetConferenceRecordsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ConferenceRecord, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = meet_conference_records_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = meet_conference_records_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Meet conference records list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListConferenceRecordsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn meet_conference_records_list(
        &self,
        args: &MeetConferenceRecordsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListConferenceRecordsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = meet_conference_records_list_builder(
            &self.http_client,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = meet_conference_records_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Meet conference records participants get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Participant result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn meet_conference_records_participants_get(
        &self,
        args: &MeetConferenceRecordsParticipantsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Participant, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = meet_conference_records_participants_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = meet_conference_records_participants_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Meet conference records participants list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListParticipantsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn meet_conference_records_participants_list(
        &self,
        args: &MeetConferenceRecordsParticipantsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListParticipantsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = meet_conference_records_participants_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = meet_conference_records_participants_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Meet conference records participants participant sessions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ParticipantSession result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn meet_conference_records_participants_participant_sessions_get(
        &self,
        args: &MeetConferenceRecordsParticipantsParticipantSessionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ParticipantSession, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = meet_conference_records_participants_participant_sessions_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = meet_conference_records_participants_participant_sessions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Meet conference records participants participant sessions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListParticipantSessionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn meet_conference_records_participants_participant_sessions_list(
        &self,
        args: &MeetConferenceRecordsParticipantsParticipantSessionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListParticipantSessionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = meet_conference_records_participants_participant_sessions_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = meet_conference_records_participants_participant_sessions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Meet conference records recordings get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Recording result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn meet_conference_records_recordings_get(
        &self,
        args: &MeetConferenceRecordsRecordingsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Recording, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = meet_conference_records_recordings_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = meet_conference_records_recordings_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Meet conference records recordings list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListRecordingsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn meet_conference_records_recordings_list(
        &self,
        args: &MeetConferenceRecordsRecordingsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListRecordingsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = meet_conference_records_recordings_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = meet_conference_records_recordings_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Meet conference records smart notes get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SmartNote result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn meet_conference_records_smart_notes_get(
        &self,
        args: &MeetConferenceRecordsSmartNotesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SmartNote, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = meet_conference_records_smart_notes_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = meet_conference_records_smart_notes_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Meet conference records smart notes list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListSmartNotesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn meet_conference_records_smart_notes_list(
        &self,
        args: &MeetConferenceRecordsSmartNotesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSmartNotesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = meet_conference_records_smart_notes_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = meet_conference_records_smart_notes_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Meet conference records transcripts get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Transcript result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn meet_conference_records_transcripts_get(
        &self,
        args: &MeetConferenceRecordsTranscriptsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Transcript, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = meet_conference_records_transcripts_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = meet_conference_records_transcripts_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Meet conference records transcripts list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListTranscriptsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn meet_conference_records_transcripts_list(
        &self,
        args: &MeetConferenceRecordsTranscriptsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListTranscriptsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = meet_conference_records_transcripts_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = meet_conference_records_transcripts_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Meet conference records transcripts entries get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TranscriptEntry result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn meet_conference_records_transcripts_entries_get(
        &self,
        args: &MeetConferenceRecordsTranscriptsEntriesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TranscriptEntry, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = meet_conference_records_transcripts_entries_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = meet_conference_records_transcripts_entries_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Meet conference records transcripts entries list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListTranscriptEntriesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn meet_conference_records_transcripts_entries_list(
        &self,
        args: &MeetConferenceRecordsTranscriptsEntriesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListTranscriptEntriesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = meet_conference_records_transcripts_entries_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = meet_conference_records_transcripts_entries_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Meet spaces create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Space result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn meet_spaces_create(
        &self,
        args: &MeetSpacesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Space, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = meet_spaces_create_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = meet_spaces_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Meet spaces end active conference.
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
    pub fn meet_spaces_end_active_conference(
        &self,
        args: &MeetSpacesEndActiveConferenceArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = meet_spaces_end_active_conference_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = meet_spaces_end_active_conference_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Meet spaces get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Space result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn meet_spaces_get(
        &self,
        args: &MeetSpacesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Space, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = meet_spaces_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = meet_spaces_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Meet spaces patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Space result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn meet_spaces_patch(
        &self,
        args: &MeetSpacesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Space, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = meet_spaces_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = meet_spaces_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
