//! LanguageProvider - State-aware language API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       language API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::language::{
    language_documents_analyze_entities_builder, language_documents_analyze_entities_task,
    language_documents_analyze_sentiment_builder, language_documents_analyze_sentiment_task,
    language_documents_annotate_text_builder, language_documents_annotate_text_task,
    language_documents_classify_text_builder, language_documents_classify_text_task,
    language_documents_moderate_text_builder, language_documents_moderate_text_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::language::AnalyzeEntitiesResponse;
use crate::providers::gcp::clients::language::AnalyzeSentimentResponse;
use crate::providers::gcp::clients::language::AnnotateTextResponse;
use crate::providers::gcp::clients::language::ClassifyTextResponse;
use crate::providers::gcp::clients::language::ModerateTextResponse;
use crate::providers::gcp::clients::language::LanguageDocumentsAnalyzeEntitiesArgs;
use crate::providers::gcp::clients::language::LanguageDocumentsAnalyzeSentimentArgs;
use crate::providers::gcp::clients::language::LanguageDocumentsAnnotateTextArgs;
use crate::providers::gcp::clients::language::LanguageDocumentsClassifyTextArgs;
use crate::providers::gcp::clients::language::LanguageDocumentsModerateTextArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// LanguageProvider with automatic state tracking.
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
/// let provider = LanguageProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct LanguageProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> LanguageProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new LanguageProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Language documents analyze entities.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AnalyzeEntitiesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn language_documents_analyze_entities(
        &self,
        args: &LanguageDocumentsAnalyzeEntitiesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AnalyzeEntitiesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = language_documents_analyze_entities_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = language_documents_analyze_entities_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Language documents analyze sentiment.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AnalyzeSentimentResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn language_documents_analyze_sentiment(
        &self,
        args: &LanguageDocumentsAnalyzeSentimentArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AnalyzeSentimentResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = language_documents_analyze_sentiment_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = language_documents_analyze_sentiment_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Language documents annotate text.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AnnotateTextResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn language_documents_annotate_text(
        &self,
        args: &LanguageDocumentsAnnotateTextArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AnnotateTextResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = language_documents_annotate_text_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = language_documents_annotate_text_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Language documents classify text.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ClassifyTextResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn language_documents_classify_text(
        &self,
        args: &LanguageDocumentsClassifyTextArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ClassifyTextResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = language_documents_classify_text_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = language_documents_classify_text_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Language documents moderate text.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ModerateTextResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn language_documents_moderate_text(
        &self,
        args: &LanguageDocumentsModerateTextArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ModerateTextResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = language_documents_moderate_text_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = language_documents_moderate_text_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
