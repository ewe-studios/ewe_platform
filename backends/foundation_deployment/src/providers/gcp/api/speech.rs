//! SpeechProvider - State-aware speech API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       speech API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::speech::{
    speech_operations_get_builder, speech_operations_get_task,
    speech_operations_list_builder, speech_operations_list_task,
    speech_projects_locations_custom_classes_create_builder, speech_projects_locations_custom_classes_create_task,
    speech_projects_locations_custom_classes_delete_builder, speech_projects_locations_custom_classes_delete_task,
    speech_projects_locations_custom_classes_get_builder, speech_projects_locations_custom_classes_get_task,
    speech_projects_locations_custom_classes_list_builder, speech_projects_locations_custom_classes_list_task,
    speech_projects_locations_custom_classes_patch_builder, speech_projects_locations_custom_classes_patch_task,
    speech_projects_locations_phrase_sets_create_builder, speech_projects_locations_phrase_sets_create_task,
    speech_projects_locations_phrase_sets_delete_builder, speech_projects_locations_phrase_sets_delete_task,
    speech_projects_locations_phrase_sets_get_builder, speech_projects_locations_phrase_sets_get_task,
    speech_projects_locations_phrase_sets_list_builder, speech_projects_locations_phrase_sets_list_task,
    speech_projects_locations_phrase_sets_patch_builder, speech_projects_locations_phrase_sets_patch_task,
    speech_speech_longrunningrecognize_builder, speech_speech_longrunningrecognize_task,
    speech_speech_recognize_builder, speech_speech_recognize_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::speech::CustomClass;
use crate::providers::gcp::clients::speech::Empty;
use crate::providers::gcp::clients::speech::ListCustomClassesResponse;
use crate::providers::gcp::clients::speech::ListOperationsResponse;
use crate::providers::gcp::clients::speech::ListPhraseSetResponse;
use crate::providers::gcp::clients::speech::Operation;
use crate::providers::gcp::clients::speech::PhraseSet;
use crate::providers::gcp::clients::speech::RecognizeResponse;
use crate::providers::gcp::clients::speech::SpeechOperationsGetArgs;
use crate::providers::gcp::clients::speech::SpeechOperationsListArgs;
use crate::providers::gcp::clients::speech::SpeechProjectsLocationsCustomClassesCreateArgs;
use crate::providers::gcp::clients::speech::SpeechProjectsLocationsCustomClassesDeleteArgs;
use crate::providers::gcp::clients::speech::SpeechProjectsLocationsCustomClassesGetArgs;
use crate::providers::gcp::clients::speech::SpeechProjectsLocationsCustomClassesListArgs;
use crate::providers::gcp::clients::speech::SpeechProjectsLocationsCustomClassesPatchArgs;
use crate::providers::gcp::clients::speech::SpeechProjectsLocationsPhraseSetsCreateArgs;
use crate::providers::gcp::clients::speech::SpeechProjectsLocationsPhraseSetsDeleteArgs;
use crate::providers::gcp::clients::speech::SpeechProjectsLocationsPhraseSetsGetArgs;
use crate::providers::gcp::clients::speech::SpeechProjectsLocationsPhraseSetsListArgs;
use crate::providers::gcp::clients::speech::SpeechProjectsLocationsPhraseSetsPatchArgs;
use crate::providers::gcp::clients::speech::SpeechSpeechLongrunningrecognizeArgs;
use crate::providers::gcp::clients::speech::SpeechSpeechRecognizeArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// SpeechProvider with automatic state tracking.
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
/// let provider = SpeechProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct SpeechProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> SpeechProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new SpeechProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Speech operations get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn speech_operations_get(
        &self,
        args: &SpeechOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = speech_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = speech_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Speech operations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListOperationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn speech_operations_list(
        &self,
        args: &SpeechOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = speech_operations_list_builder(
            &self.http_client,
            &args.filter,
            &args.name,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = speech_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Speech projects locations custom classes create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CustomClass result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn speech_projects_locations_custom_classes_create(
        &self,
        args: &SpeechProjectsLocationsCustomClassesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CustomClass, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = speech_projects_locations_custom_classes_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = speech_projects_locations_custom_classes_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Speech projects locations custom classes delete.
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
    pub fn speech_projects_locations_custom_classes_delete(
        &self,
        args: &SpeechProjectsLocationsCustomClassesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = speech_projects_locations_custom_classes_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = speech_projects_locations_custom_classes_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Speech projects locations custom classes get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CustomClass result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn speech_projects_locations_custom_classes_get(
        &self,
        args: &SpeechProjectsLocationsCustomClassesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CustomClass, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = speech_projects_locations_custom_classes_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = speech_projects_locations_custom_classes_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Speech projects locations custom classes list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCustomClassesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn speech_projects_locations_custom_classes_list(
        &self,
        args: &SpeechProjectsLocationsCustomClassesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCustomClassesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = speech_projects_locations_custom_classes_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = speech_projects_locations_custom_classes_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Speech projects locations custom classes patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CustomClass result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn speech_projects_locations_custom_classes_patch(
        &self,
        args: &SpeechProjectsLocationsCustomClassesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CustomClass, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = speech_projects_locations_custom_classes_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = speech_projects_locations_custom_classes_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Speech projects locations phrase sets create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PhraseSet result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn speech_projects_locations_phrase_sets_create(
        &self,
        args: &SpeechProjectsLocationsPhraseSetsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PhraseSet, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = speech_projects_locations_phrase_sets_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = speech_projects_locations_phrase_sets_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Speech projects locations phrase sets delete.
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
    pub fn speech_projects_locations_phrase_sets_delete(
        &self,
        args: &SpeechProjectsLocationsPhraseSetsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = speech_projects_locations_phrase_sets_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = speech_projects_locations_phrase_sets_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Speech projects locations phrase sets get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PhraseSet result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn speech_projects_locations_phrase_sets_get(
        &self,
        args: &SpeechProjectsLocationsPhraseSetsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PhraseSet, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = speech_projects_locations_phrase_sets_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = speech_projects_locations_phrase_sets_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Speech projects locations phrase sets list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListPhraseSetResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn speech_projects_locations_phrase_sets_list(
        &self,
        args: &SpeechProjectsLocationsPhraseSetsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListPhraseSetResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = speech_projects_locations_phrase_sets_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = speech_projects_locations_phrase_sets_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Speech projects locations phrase sets patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PhraseSet result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn speech_projects_locations_phrase_sets_patch(
        &self,
        args: &SpeechProjectsLocationsPhraseSetsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PhraseSet, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = speech_projects_locations_phrase_sets_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = speech_projects_locations_phrase_sets_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Speech speech longrunningrecognize.
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
    pub fn speech_speech_longrunningrecognize(
        &self,
        args: &SpeechSpeechLongrunningrecognizeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = speech_speech_longrunningrecognize_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = speech_speech_longrunningrecognize_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Speech speech recognize.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RecognizeResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn speech_speech_recognize(
        &self,
        args: &SpeechSpeechRecognizeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RecognizeResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = speech_speech_recognize_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = speech_speech_recognize_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
