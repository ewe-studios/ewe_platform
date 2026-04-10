//! DatalabelingProvider - State-aware datalabeling API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       datalabeling API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::datalabeling::{
    datalabeling_projects_annotation_spec_sets_create_builder, datalabeling_projects_annotation_spec_sets_create_task,
    datalabeling_projects_annotation_spec_sets_delete_builder, datalabeling_projects_annotation_spec_sets_delete_task,
    datalabeling_projects_datasets_create_builder, datalabeling_projects_datasets_create_task,
    datalabeling_projects_datasets_delete_builder, datalabeling_projects_datasets_delete_task,
    datalabeling_projects_datasets_export_data_builder, datalabeling_projects_datasets_export_data_task,
    datalabeling_projects_datasets_import_data_builder, datalabeling_projects_datasets_import_data_task,
    datalabeling_projects_datasets_annotated_datasets_delete_builder, datalabeling_projects_datasets_annotated_datasets_delete_task,
    datalabeling_projects_datasets_annotated_datasets_feedback_threads_delete_builder, datalabeling_projects_datasets_annotated_datasets_feedback_threads_delete_task,
    datalabeling_projects_datasets_annotated_datasets_feedback_threads_feedback_messages_create_builder, datalabeling_projects_datasets_annotated_datasets_feedback_threads_feedback_messages_create_task,
    datalabeling_projects_datasets_annotated_datasets_feedback_threads_feedback_messages_delete_builder, datalabeling_projects_datasets_annotated_datasets_feedback_threads_feedback_messages_delete_task,
    datalabeling_projects_datasets_evaluations_example_comparisons_search_builder, datalabeling_projects_datasets_evaluations_example_comparisons_search_task,
    datalabeling_projects_datasets_image_label_builder, datalabeling_projects_datasets_image_label_task,
    datalabeling_projects_datasets_text_label_builder, datalabeling_projects_datasets_text_label_task,
    datalabeling_projects_datasets_video_label_builder, datalabeling_projects_datasets_video_label_task,
    datalabeling_projects_evaluation_jobs_create_builder, datalabeling_projects_evaluation_jobs_create_task,
    datalabeling_projects_evaluation_jobs_delete_builder, datalabeling_projects_evaluation_jobs_delete_task,
    datalabeling_projects_evaluation_jobs_patch_builder, datalabeling_projects_evaluation_jobs_patch_task,
    datalabeling_projects_evaluation_jobs_pause_builder, datalabeling_projects_evaluation_jobs_pause_task,
    datalabeling_projects_evaluation_jobs_resume_builder, datalabeling_projects_evaluation_jobs_resume_task,
    datalabeling_projects_instructions_create_builder, datalabeling_projects_instructions_create_task,
    datalabeling_projects_instructions_delete_builder, datalabeling_projects_instructions_delete_task,
    datalabeling_projects_operations_delete_builder, datalabeling_projects_operations_delete_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::datalabeling::GoogleCloudDatalabelingV1beta1AnnotationSpecSet;
use crate::providers::gcp::clients::datalabeling::GoogleCloudDatalabelingV1beta1Dataset;
use crate::providers::gcp::clients::datalabeling::GoogleCloudDatalabelingV1beta1EvaluationJob;
use crate::providers::gcp::clients::datalabeling::GoogleCloudDatalabelingV1beta1SearchExampleComparisonsResponse;
use crate::providers::gcp::clients::datalabeling::GoogleLongrunningOperation;
use crate::providers::gcp::clients::datalabeling::GoogleProtobufEmpty;
use crate::providers::gcp::clients::datalabeling::DatalabelingProjectsAnnotationSpecSetsCreateArgs;
use crate::providers::gcp::clients::datalabeling::DatalabelingProjectsAnnotationSpecSetsDeleteArgs;
use crate::providers::gcp::clients::datalabeling::DatalabelingProjectsDatasetsAnnotatedDatasetsDeleteArgs;
use crate::providers::gcp::clients::datalabeling::DatalabelingProjectsDatasetsAnnotatedDatasetsFeedbackThreadsDeleteArgs;
use crate::providers::gcp::clients::datalabeling::DatalabelingProjectsDatasetsAnnotatedDatasetsFeedbackThreadsFeedbackMessagesCreateArgs;
use crate::providers::gcp::clients::datalabeling::DatalabelingProjectsDatasetsAnnotatedDatasetsFeedbackThreadsFeedbackMessagesDeleteArgs;
use crate::providers::gcp::clients::datalabeling::DatalabelingProjectsDatasetsCreateArgs;
use crate::providers::gcp::clients::datalabeling::DatalabelingProjectsDatasetsDeleteArgs;
use crate::providers::gcp::clients::datalabeling::DatalabelingProjectsDatasetsEvaluationsExampleComparisonsSearchArgs;
use crate::providers::gcp::clients::datalabeling::DatalabelingProjectsDatasetsExportDataArgs;
use crate::providers::gcp::clients::datalabeling::DatalabelingProjectsDatasetsImageLabelArgs;
use crate::providers::gcp::clients::datalabeling::DatalabelingProjectsDatasetsImportDataArgs;
use crate::providers::gcp::clients::datalabeling::DatalabelingProjectsDatasetsTextLabelArgs;
use crate::providers::gcp::clients::datalabeling::DatalabelingProjectsDatasetsVideoLabelArgs;
use crate::providers::gcp::clients::datalabeling::DatalabelingProjectsEvaluationJobsCreateArgs;
use crate::providers::gcp::clients::datalabeling::DatalabelingProjectsEvaluationJobsDeleteArgs;
use crate::providers::gcp::clients::datalabeling::DatalabelingProjectsEvaluationJobsPatchArgs;
use crate::providers::gcp::clients::datalabeling::DatalabelingProjectsEvaluationJobsPauseArgs;
use crate::providers::gcp::clients::datalabeling::DatalabelingProjectsEvaluationJobsResumeArgs;
use crate::providers::gcp::clients::datalabeling::DatalabelingProjectsInstructionsCreateArgs;
use crate::providers::gcp::clients::datalabeling::DatalabelingProjectsInstructionsDeleteArgs;
use crate::providers::gcp::clients::datalabeling::DatalabelingProjectsOperationsDeleteArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// DatalabelingProvider with automatic state tracking.
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
/// let provider = DatalabelingProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct DatalabelingProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> DatalabelingProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new DatalabelingProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Datalabeling projects annotation spec sets create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatalabelingV1beta1AnnotationSpecSet result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datalabeling_projects_annotation_spec_sets_create(
        &self,
        args: &DatalabelingProjectsAnnotationSpecSetsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatalabelingV1beta1AnnotationSpecSet, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datalabeling_projects_annotation_spec_sets_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = datalabeling_projects_annotation_spec_sets_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datalabeling projects annotation spec sets delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datalabeling_projects_annotation_spec_sets_delete(
        &self,
        args: &DatalabelingProjectsAnnotationSpecSetsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datalabeling_projects_annotation_spec_sets_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datalabeling_projects_annotation_spec_sets_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datalabeling projects datasets create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatalabelingV1beta1Dataset result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datalabeling_projects_datasets_create(
        &self,
        args: &DatalabelingProjectsDatasetsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatalabelingV1beta1Dataset, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datalabeling_projects_datasets_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = datalabeling_projects_datasets_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datalabeling projects datasets delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datalabeling_projects_datasets_delete(
        &self,
        args: &DatalabelingProjectsDatasetsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datalabeling_projects_datasets_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datalabeling_projects_datasets_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datalabeling projects datasets export data.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datalabeling_projects_datasets_export_data(
        &self,
        args: &DatalabelingProjectsDatasetsExportDataArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datalabeling_projects_datasets_export_data_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datalabeling_projects_datasets_export_data_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datalabeling projects datasets import data.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datalabeling_projects_datasets_import_data(
        &self,
        args: &DatalabelingProjectsDatasetsImportDataArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datalabeling_projects_datasets_import_data_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datalabeling_projects_datasets_import_data_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datalabeling projects datasets annotated datasets delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datalabeling_projects_datasets_annotated_datasets_delete(
        &self,
        args: &DatalabelingProjectsDatasetsAnnotatedDatasetsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datalabeling_projects_datasets_annotated_datasets_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datalabeling_projects_datasets_annotated_datasets_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datalabeling projects datasets annotated datasets feedback threads delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datalabeling_projects_datasets_annotated_datasets_feedback_threads_delete(
        &self,
        args: &DatalabelingProjectsDatasetsAnnotatedDatasetsFeedbackThreadsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datalabeling_projects_datasets_annotated_datasets_feedback_threads_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datalabeling_projects_datasets_annotated_datasets_feedback_threads_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datalabeling projects datasets annotated datasets feedback threads feedback messages create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datalabeling_projects_datasets_annotated_datasets_feedback_threads_feedback_messages_create(
        &self,
        args: &DatalabelingProjectsDatasetsAnnotatedDatasetsFeedbackThreadsFeedbackMessagesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datalabeling_projects_datasets_annotated_datasets_feedback_threads_feedback_messages_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = datalabeling_projects_datasets_annotated_datasets_feedback_threads_feedback_messages_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datalabeling projects datasets annotated datasets feedback threads feedback messages delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datalabeling_projects_datasets_annotated_datasets_feedback_threads_feedback_messages_delete(
        &self,
        args: &DatalabelingProjectsDatasetsAnnotatedDatasetsFeedbackThreadsFeedbackMessagesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datalabeling_projects_datasets_annotated_datasets_feedback_threads_feedback_messages_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datalabeling_projects_datasets_annotated_datasets_feedback_threads_feedback_messages_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datalabeling projects datasets evaluations example comparisons search.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatalabelingV1beta1SearchExampleComparisonsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datalabeling_projects_datasets_evaluations_example_comparisons_search(
        &self,
        args: &DatalabelingProjectsDatasetsEvaluationsExampleComparisonsSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatalabelingV1beta1SearchExampleComparisonsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datalabeling_projects_datasets_evaluations_example_comparisons_search_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = datalabeling_projects_datasets_evaluations_example_comparisons_search_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datalabeling projects datasets image label.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datalabeling_projects_datasets_image_label(
        &self,
        args: &DatalabelingProjectsDatasetsImageLabelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datalabeling_projects_datasets_image_label_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = datalabeling_projects_datasets_image_label_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datalabeling projects datasets text label.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datalabeling_projects_datasets_text_label(
        &self,
        args: &DatalabelingProjectsDatasetsTextLabelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datalabeling_projects_datasets_text_label_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = datalabeling_projects_datasets_text_label_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datalabeling projects datasets video label.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datalabeling_projects_datasets_video_label(
        &self,
        args: &DatalabelingProjectsDatasetsVideoLabelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datalabeling_projects_datasets_video_label_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = datalabeling_projects_datasets_video_label_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datalabeling projects evaluation jobs create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatalabelingV1beta1EvaluationJob result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datalabeling_projects_evaluation_jobs_create(
        &self,
        args: &DatalabelingProjectsEvaluationJobsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatalabelingV1beta1EvaluationJob, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datalabeling_projects_evaluation_jobs_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = datalabeling_projects_evaluation_jobs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datalabeling projects evaluation jobs delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datalabeling_projects_evaluation_jobs_delete(
        &self,
        args: &DatalabelingProjectsEvaluationJobsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datalabeling_projects_evaluation_jobs_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datalabeling_projects_evaluation_jobs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datalabeling projects evaluation jobs patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatalabelingV1beta1EvaluationJob result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datalabeling_projects_evaluation_jobs_patch(
        &self,
        args: &DatalabelingProjectsEvaluationJobsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatalabelingV1beta1EvaluationJob, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datalabeling_projects_evaluation_jobs_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = datalabeling_projects_evaluation_jobs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datalabeling projects evaluation jobs pause.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datalabeling_projects_evaluation_jobs_pause(
        &self,
        args: &DatalabelingProjectsEvaluationJobsPauseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datalabeling_projects_evaluation_jobs_pause_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datalabeling_projects_evaluation_jobs_pause_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datalabeling projects evaluation jobs resume.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datalabeling_projects_evaluation_jobs_resume(
        &self,
        args: &DatalabelingProjectsEvaluationJobsResumeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datalabeling_projects_evaluation_jobs_resume_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datalabeling_projects_evaluation_jobs_resume_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datalabeling projects instructions create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datalabeling_projects_instructions_create(
        &self,
        args: &DatalabelingProjectsInstructionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datalabeling_projects_instructions_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = datalabeling_projects_instructions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datalabeling projects instructions delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datalabeling_projects_instructions_delete(
        &self,
        args: &DatalabelingProjectsInstructionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datalabeling_projects_instructions_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datalabeling_projects_instructions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datalabeling projects operations delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datalabeling_projects_operations_delete(
        &self,
        args: &DatalabelingProjectsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datalabeling_projects_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datalabeling_projects_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
