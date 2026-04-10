//! TranslateProvider - State-aware translate API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       translate API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::translate::{
    translate_projects_detect_language_builder, translate_projects_detect_language_task,
    translate_projects_romanize_text_builder, translate_projects_romanize_text_task,
    translate_projects_translate_text_builder, translate_projects_translate_text_task,
    translate_projects_locations_adaptive_mt_translate_builder, translate_projects_locations_adaptive_mt_translate_task,
    translate_projects_locations_batch_translate_document_builder, translate_projects_locations_batch_translate_document_task,
    translate_projects_locations_batch_translate_text_builder, translate_projects_locations_batch_translate_text_task,
    translate_projects_locations_detect_language_builder, translate_projects_locations_detect_language_task,
    translate_projects_locations_refine_text_builder, translate_projects_locations_refine_text_task,
    translate_projects_locations_romanize_text_builder, translate_projects_locations_romanize_text_task,
    translate_projects_locations_translate_document_builder, translate_projects_locations_translate_document_task,
    translate_projects_locations_translate_text_builder, translate_projects_locations_translate_text_task,
    translate_projects_locations_adaptive_mt_datasets_create_builder, translate_projects_locations_adaptive_mt_datasets_create_task,
    translate_projects_locations_adaptive_mt_datasets_delete_builder, translate_projects_locations_adaptive_mt_datasets_delete_task,
    translate_projects_locations_adaptive_mt_datasets_import_adaptive_mt_file_builder, translate_projects_locations_adaptive_mt_datasets_import_adaptive_mt_file_task,
    translate_projects_locations_adaptive_mt_datasets_adaptive_mt_files_delete_builder, translate_projects_locations_adaptive_mt_datasets_adaptive_mt_files_delete_task,
    translate_projects_locations_datasets_create_builder, translate_projects_locations_datasets_create_task,
    translate_projects_locations_datasets_delete_builder, translate_projects_locations_datasets_delete_task,
    translate_projects_locations_datasets_export_data_builder, translate_projects_locations_datasets_export_data_task,
    translate_projects_locations_datasets_import_data_builder, translate_projects_locations_datasets_import_data_task,
    translate_projects_locations_glossaries_create_builder, translate_projects_locations_glossaries_create_task,
    translate_projects_locations_glossaries_delete_builder, translate_projects_locations_glossaries_delete_task,
    translate_projects_locations_glossaries_patch_builder, translate_projects_locations_glossaries_patch_task,
    translate_projects_locations_glossaries_glossary_entries_create_builder, translate_projects_locations_glossaries_glossary_entries_create_task,
    translate_projects_locations_glossaries_glossary_entries_delete_builder, translate_projects_locations_glossaries_glossary_entries_delete_task,
    translate_projects_locations_glossaries_glossary_entries_patch_builder, translate_projects_locations_glossaries_glossary_entries_patch_task,
    translate_projects_locations_models_create_builder, translate_projects_locations_models_create_task,
    translate_projects_locations_models_delete_builder, translate_projects_locations_models_delete_task,
    translate_projects_locations_operations_cancel_builder, translate_projects_locations_operations_cancel_task,
    translate_projects_locations_operations_delete_builder, translate_projects_locations_operations_delete_task,
    translate_projects_locations_operations_wait_builder, translate_projects_locations_operations_wait_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::translate::AdaptiveMtDataset;
use crate::providers::gcp::clients::translate::AdaptiveMtTranslateResponse;
use crate::providers::gcp::clients::translate::DetectLanguageResponse;
use crate::providers::gcp::clients::translate::Empty;
use crate::providers::gcp::clients::translate::GlossaryEntry;
use crate::providers::gcp::clients::translate::ImportAdaptiveMtFileResponse;
use crate::providers::gcp::clients::translate::Operation;
use crate::providers::gcp::clients::translate::RefineTextResponse;
use crate::providers::gcp::clients::translate::RomanizeTextResponse;
use crate::providers::gcp::clients::translate::TranslateDocumentResponse;
use crate::providers::gcp::clients::translate::TranslateTextResponse;
use crate::providers::gcp::clients::translate::TranslateProjectsDetectLanguageArgs;
use crate::providers::gcp::clients::translate::TranslateProjectsLocationsAdaptiveMtDatasetsAdaptiveMtFilesDeleteArgs;
use crate::providers::gcp::clients::translate::TranslateProjectsLocationsAdaptiveMtDatasetsCreateArgs;
use crate::providers::gcp::clients::translate::TranslateProjectsLocationsAdaptiveMtDatasetsDeleteArgs;
use crate::providers::gcp::clients::translate::TranslateProjectsLocationsAdaptiveMtDatasetsImportAdaptiveMtFileArgs;
use crate::providers::gcp::clients::translate::TranslateProjectsLocationsAdaptiveMtTranslateArgs;
use crate::providers::gcp::clients::translate::TranslateProjectsLocationsBatchTranslateDocumentArgs;
use crate::providers::gcp::clients::translate::TranslateProjectsLocationsBatchTranslateTextArgs;
use crate::providers::gcp::clients::translate::TranslateProjectsLocationsDatasetsCreateArgs;
use crate::providers::gcp::clients::translate::TranslateProjectsLocationsDatasetsDeleteArgs;
use crate::providers::gcp::clients::translate::TranslateProjectsLocationsDatasetsExportDataArgs;
use crate::providers::gcp::clients::translate::TranslateProjectsLocationsDatasetsImportDataArgs;
use crate::providers::gcp::clients::translate::TranslateProjectsLocationsDetectLanguageArgs;
use crate::providers::gcp::clients::translate::TranslateProjectsLocationsGlossariesCreateArgs;
use crate::providers::gcp::clients::translate::TranslateProjectsLocationsGlossariesDeleteArgs;
use crate::providers::gcp::clients::translate::TranslateProjectsLocationsGlossariesGlossaryEntriesCreateArgs;
use crate::providers::gcp::clients::translate::TranslateProjectsLocationsGlossariesGlossaryEntriesDeleteArgs;
use crate::providers::gcp::clients::translate::TranslateProjectsLocationsGlossariesGlossaryEntriesPatchArgs;
use crate::providers::gcp::clients::translate::TranslateProjectsLocationsGlossariesPatchArgs;
use crate::providers::gcp::clients::translate::TranslateProjectsLocationsModelsCreateArgs;
use crate::providers::gcp::clients::translate::TranslateProjectsLocationsModelsDeleteArgs;
use crate::providers::gcp::clients::translate::TranslateProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::translate::TranslateProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::translate::TranslateProjectsLocationsOperationsWaitArgs;
use crate::providers::gcp::clients::translate::TranslateProjectsLocationsRefineTextArgs;
use crate::providers::gcp::clients::translate::TranslateProjectsLocationsRomanizeTextArgs;
use crate::providers::gcp::clients::translate::TranslateProjectsLocationsTranslateDocumentArgs;
use crate::providers::gcp::clients::translate::TranslateProjectsLocationsTranslateTextArgs;
use crate::providers::gcp::clients::translate::TranslateProjectsRomanizeTextArgs;
use crate::providers::gcp::clients::translate::TranslateProjectsTranslateTextArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// TranslateProvider with automatic state tracking.
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
/// let provider = TranslateProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct TranslateProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> TranslateProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new TranslateProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Translate projects detect language.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DetectLanguageResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn translate_projects_detect_language(
        &self,
        args: &TranslateProjectsDetectLanguageArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DetectLanguageResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = translate_projects_detect_language_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = translate_projects_detect_language_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Translate projects romanize text.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RomanizeTextResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn translate_projects_romanize_text(
        &self,
        args: &TranslateProjectsRomanizeTextArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RomanizeTextResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = translate_projects_romanize_text_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = translate_projects_romanize_text_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Translate projects translate text.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TranslateTextResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn translate_projects_translate_text(
        &self,
        args: &TranslateProjectsTranslateTextArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TranslateTextResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = translate_projects_translate_text_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = translate_projects_translate_text_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Translate projects locations adaptive mt translate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AdaptiveMtTranslateResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn translate_projects_locations_adaptive_mt_translate(
        &self,
        args: &TranslateProjectsLocationsAdaptiveMtTranslateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AdaptiveMtTranslateResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = translate_projects_locations_adaptive_mt_translate_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = translate_projects_locations_adaptive_mt_translate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Translate projects locations batch translate document.
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
    pub fn translate_projects_locations_batch_translate_document(
        &self,
        args: &TranslateProjectsLocationsBatchTranslateDocumentArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = translate_projects_locations_batch_translate_document_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = translate_projects_locations_batch_translate_document_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Translate projects locations batch translate text.
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
    pub fn translate_projects_locations_batch_translate_text(
        &self,
        args: &TranslateProjectsLocationsBatchTranslateTextArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = translate_projects_locations_batch_translate_text_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = translate_projects_locations_batch_translate_text_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Translate projects locations detect language.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DetectLanguageResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn translate_projects_locations_detect_language(
        &self,
        args: &TranslateProjectsLocationsDetectLanguageArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DetectLanguageResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = translate_projects_locations_detect_language_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = translate_projects_locations_detect_language_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Translate projects locations refine text.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RefineTextResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn translate_projects_locations_refine_text(
        &self,
        args: &TranslateProjectsLocationsRefineTextArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RefineTextResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = translate_projects_locations_refine_text_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = translate_projects_locations_refine_text_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Translate projects locations romanize text.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RomanizeTextResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn translate_projects_locations_romanize_text(
        &self,
        args: &TranslateProjectsLocationsRomanizeTextArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RomanizeTextResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = translate_projects_locations_romanize_text_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = translate_projects_locations_romanize_text_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Translate projects locations translate document.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TranslateDocumentResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn translate_projects_locations_translate_document(
        &self,
        args: &TranslateProjectsLocationsTranslateDocumentArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TranslateDocumentResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = translate_projects_locations_translate_document_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = translate_projects_locations_translate_document_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Translate projects locations translate text.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TranslateTextResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn translate_projects_locations_translate_text(
        &self,
        args: &TranslateProjectsLocationsTranslateTextArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TranslateTextResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = translate_projects_locations_translate_text_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = translate_projects_locations_translate_text_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Translate projects locations adaptive mt datasets create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AdaptiveMtDataset result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn translate_projects_locations_adaptive_mt_datasets_create(
        &self,
        args: &TranslateProjectsLocationsAdaptiveMtDatasetsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AdaptiveMtDataset, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = translate_projects_locations_adaptive_mt_datasets_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = translate_projects_locations_adaptive_mt_datasets_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Translate projects locations adaptive mt datasets delete.
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
    pub fn translate_projects_locations_adaptive_mt_datasets_delete(
        &self,
        args: &TranslateProjectsLocationsAdaptiveMtDatasetsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = translate_projects_locations_adaptive_mt_datasets_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = translate_projects_locations_adaptive_mt_datasets_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Translate projects locations adaptive mt datasets import adaptive mt file.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ImportAdaptiveMtFileResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn translate_projects_locations_adaptive_mt_datasets_import_adaptive_mt_file(
        &self,
        args: &TranslateProjectsLocationsAdaptiveMtDatasetsImportAdaptiveMtFileArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ImportAdaptiveMtFileResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = translate_projects_locations_adaptive_mt_datasets_import_adaptive_mt_file_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = translate_projects_locations_adaptive_mt_datasets_import_adaptive_mt_file_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Translate projects locations adaptive mt datasets adaptive mt files delete.
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
    pub fn translate_projects_locations_adaptive_mt_datasets_adaptive_mt_files_delete(
        &self,
        args: &TranslateProjectsLocationsAdaptiveMtDatasetsAdaptiveMtFilesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = translate_projects_locations_adaptive_mt_datasets_adaptive_mt_files_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = translate_projects_locations_adaptive_mt_datasets_adaptive_mt_files_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Translate projects locations datasets create.
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
    pub fn translate_projects_locations_datasets_create(
        &self,
        args: &TranslateProjectsLocationsDatasetsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = translate_projects_locations_datasets_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = translate_projects_locations_datasets_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Translate projects locations datasets delete.
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
    pub fn translate_projects_locations_datasets_delete(
        &self,
        args: &TranslateProjectsLocationsDatasetsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = translate_projects_locations_datasets_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = translate_projects_locations_datasets_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Translate projects locations datasets export data.
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
    pub fn translate_projects_locations_datasets_export_data(
        &self,
        args: &TranslateProjectsLocationsDatasetsExportDataArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = translate_projects_locations_datasets_export_data_builder(
            &self.http_client,
            &args.dataset,
        )
        .map_err(ProviderError::Api)?;

        let task = translate_projects_locations_datasets_export_data_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Translate projects locations datasets import data.
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
    pub fn translate_projects_locations_datasets_import_data(
        &self,
        args: &TranslateProjectsLocationsDatasetsImportDataArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = translate_projects_locations_datasets_import_data_builder(
            &self.http_client,
            &args.dataset,
        )
        .map_err(ProviderError::Api)?;

        let task = translate_projects_locations_datasets_import_data_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Translate projects locations glossaries create.
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
    pub fn translate_projects_locations_glossaries_create(
        &self,
        args: &TranslateProjectsLocationsGlossariesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = translate_projects_locations_glossaries_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = translate_projects_locations_glossaries_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Translate projects locations glossaries delete.
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
    pub fn translate_projects_locations_glossaries_delete(
        &self,
        args: &TranslateProjectsLocationsGlossariesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = translate_projects_locations_glossaries_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = translate_projects_locations_glossaries_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Translate projects locations glossaries patch.
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
    pub fn translate_projects_locations_glossaries_patch(
        &self,
        args: &TranslateProjectsLocationsGlossariesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = translate_projects_locations_glossaries_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = translate_projects_locations_glossaries_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Translate projects locations glossaries glossary entries create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GlossaryEntry result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn translate_projects_locations_glossaries_glossary_entries_create(
        &self,
        args: &TranslateProjectsLocationsGlossariesGlossaryEntriesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GlossaryEntry, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = translate_projects_locations_glossaries_glossary_entries_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = translate_projects_locations_glossaries_glossary_entries_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Translate projects locations glossaries glossary entries delete.
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
    pub fn translate_projects_locations_glossaries_glossary_entries_delete(
        &self,
        args: &TranslateProjectsLocationsGlossariesGlossaryEntriesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = translate_projects_locations_glossaries_glossary_entries_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = translate_projects_locations_glossaries_glossary_entries_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Translate projects locations glossaries glossary entries patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GlossaryEntry result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn translate_projects_locations_glossaries_glossary_entries_patch(
        &self,
        args: &TranslateProjectsLocationsGlossariesGlossaryEntriesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GlossaryEntry, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = translate_projects_locations_glossaries_glossary_entries_patch_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = translate_projects_locations_glossaries_glossary_entries_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Translate projects locations models create.
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
    pub fn translate_projects_locations_models_create(
        &self,
        args: &TranslateProjectsLocationsModelsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = translate_projects_locations_models_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = translate_projects_locations_models_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Translate projects locations models delete.
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
    pub fn translate_projects_locations_models_delete(
        &self,
        args: &TranslateProjectsLocationsModelsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = translate_projects_locations_models_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = translate_projects_locations_models_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Translate projects locations operations cancel.
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
    pub fn translate_projects_locations_operations_cancel(
        &self,
        args: &TranslateProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = translate_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = translate_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Translate projects locations operations delete.
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
    pub fn translate_projects_locations_operations_delete(
        &self,
        args: &TranslateProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = translate_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = translate_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Translate projects locations operations wait.
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
    pub fn translate_projects_locations_operations_wait(
        &self,
        args: &TranslateProjectsLocationsOperationsWaitArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = translate_projects_locations_operations_wait_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = translate_projects_locations_operations_wait_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
