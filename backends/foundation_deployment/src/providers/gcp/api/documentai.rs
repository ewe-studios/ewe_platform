//! DocumentaiProvider - State-aware documentai API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       documentai API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::documentai::{
    documentai_operations_delete_builder, documentai_operations_delete_task,
    documentai_projects_locations_operations_cancel_builder, documentai_projects_locations_operations_cancel_task,
    documentai_projects_locations_processors_batch_process_builder, documentai_projects_locations_processors_batch_process_task,
    documentai_projects_locations_processors_create_builder, documentai_projects_locations_processors_create_task,
    documentai_projects_locations_processors_delete_builder, documentai_projects_locations_processors_delete_task,
    documentai_projects_locations_processors_disable_builder, documentai_projects_locations_processors_disable_task,
    documentai_projects_locations_processors_enable_builder, documentai_projects_locations_processors_enable_task,
    documentai_projects_locations_processors_process_builder, documentai_projects_locations_processors_process_task,
    documentai_projects_locations_processors_set_default_processor_version_builder, documentai_projects_locations_processors_set_default_processor_version_task,
    documentai_projects_locations_processors_human_review_config_review_document_builder, documentai_projects_locations_processors_human_review_config_review_document_task,
    documentai_projects_locations_processors_processor_versions_batch_process_builder, documentai_projects_locations_processors_processor_versions_batch_process_task,
    documentai_projects_locations_processors_processor_versions_delete_builder, documentai_projects_locations_processors_processor_versions_delete_task,
    documentai_projects_locations_processors_processor_versions_deploy_builder, documentai_projects_locations_processors_processor_versions_deploy_task,
    documentai_projects_locations_processors_processor_versions_evaluate_processor_version_builder, documentai_projects_locations_processors_processor_versions_evaluate_processor_version_task,
    documentai_projects_locations_processors_processor_versions_process_builder, documentai_projects_locations_processors_processor_versions_process_task,
    documentai_projects_locations_processors_processor_versions_train_builder, documentai_projects_locations_processors_processor_versions_train_task,
    documentai_projects_locations_processors_processor_versions_undeploy_builder, documentai_projects_locations_processors_processor_versions_undeploy_task,
    documentai_projects_locations_schemas_create_builder, documentai_projects_locations_schemas_create_task,
    documentai_projects_locations_schemas_delete_builder, documentai_projects_locations_schemas_delete_task,
    documentai_projects_locations_schemas_patch_builder, documentai_projects_locations_schemas_patch_task,
    documentai_projects_locations_schemas_schema_versions_create_builder, documentai_projects_locations_schemas_schema_versions_create_task,
    documentai_projects_locations_schemas_schema_versions_delete_builder, documentai_projects_locations_schemas_schema_versions_delete_task,
    documentai_projects_locations_schemas_schema_versions_generate_builder, documentai_projects_locations_schemas_schema_versions_generate_task,
    documentai_projects_locations_schemas_schema_versions_patch_builder, documentai_projects_locations_schemas_schema_versions_patch_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::documentai::GoogleCloudDocumentaiV1GenerateSchemaVersionResponse;
use crate::providers::gcp::clients::documentai::GoogleCloudDocumentaiV1NextSchema;
use crate::providers::gcp::clients::documentai::GoogleCloudDocumentaiV1ProcessResponse;
use crate::providers::gcp::clients::documentai::GoogleCloudDocumentaiV1Processor;
use crate::providers::gcp::clients::documentai::GoogleCloudDocumentaiV1SchemaVersion;
use crate::providers::gcp::clients::documentai::GoogleLongrunningOperation;
use crate::providers::gcp::clients::documentai::GoogleProtobufEmpty;
use crate::providers::gcp::clients::documentai::DocumentaiOperationsDeleteArgs;
use crate::providers::gcp::clients::documentai::DocumentaiProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::documentai::DocumentaiProjectsLocationsProcessorsBatchProcessArgs;
use crate::providers::gcp::clients::documentai::DocumentaiProjectsLocationsProcessorsCreateArgs;
use crate::providers::gcp::clients::documentai::DocumentaiProjectsLocationsProcessorsDeleteArgs;
use crate::providers::gcp::clients::documentai::DocumentaiProjectsLocationsProcessorsDisableArgs;
use crate::providers::gcp::clients::documentai::DocumentaiProjectsLocationsProcessorsEnableArgs;
use crate::providers::gcp::clients::documentai::DocumentaiProjectsLocationsProcessorsHumanReviewConfigReviewDocumentArgs;
use crate::providers::gcp::clients::documentai::DocumentaiProjectsLocationsProcessorsProcessArgs;
use crate::providers::gcp::clients::documentai::DocumentaiProjectsLocationsProcessorsProcessorVersionsBatchProcessArgs;
use crate::providers::gcp::clients::documentai::DocumentaiProjectsLocationsProcessorsProcessorVersionsDeleteArgs;
use crate::providers::gcp::clients::documentai::DocumentaiProjectsLocationsProcessorsProcessorVersionsDeployArgs;
use crate::providers::gcp::clients::documentai::DocumentaiProjectsLocationsProcessorsProcessorVersionsEvaluateProcessorVersionArgs;
use crate::providers::gcp::clients::documentai::DocumentaiProjectsLocationsProcessorsProcessorVersionsProcessArgs;
use crate::providers::gcp::clients::documentai::DocumentaiProjectsLocationsProcessorsProcessorVersionsTrainArgs;
use crate::providers::gcp::clients::documentai::DocumentaiProjectsLocationsProcessorsProcessorVersionsUndeployArgs;
use crate::providers::gcp::clients::documentai::DocumentaiProjectsLocationsProcessorsSetDefaultProcessorVersionArgs;
use crate::providers::gcp::clients::documentai::DocumentaiProjectsLocationsSchemasCreateArgs;
use crate::providers::gcp::clients::documentai::DocumentaiProjectsLocationsSchemasDeleteArgs;
use crate::providers::gcp::clients::documentai::DocumentaiProjectsLocationsSchemasPatchArgs;
use crate::providers::gcp::clients::documentai::DocumentaiProjectsLocationsSchemasSchemaVersionsCreateArgs;
use crate::providers::gcp::clients::documentai::DocumentaiProjectsLocationsSchemasSchemaVersionsDeleteArgs;
use crate::providers::gcp::clients::documentai::DocumentaiProjectsLocationsSchemasSchemaVersionsGenerateArgs;
use crate::providers::gcp::clients::documentai::DocumentaiProjectsLocationsSchemasSchemaVersionsPatchArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// DocumentaiProvider with automatic state tracking.
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
/// let provider = DocumentaiProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct DocumentaiProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> DocumentaiProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new DocumentaiProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Documentai operations delete.
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
    pub fn documentai_operations_delete(
        &self,
        args: &DocumentaiOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = documentai_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = documentai_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Documentai projects locations operations cancel.
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
    pub fn documentai_projects_locations_operations_cancel(
        &self,
        args: &DocumentaiProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = documentai_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = documentai_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Documentai projects locations processors batch process.
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
    pub fn documentai_projects_locations_processors_batch_process(
        &self,
        args: &DocumentaiProjectsLocationsProcessorsBatchProcessArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = documentai_projects_locations_processors_batch_process_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = documentai_projects_locations_processors_batch_process_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Documentai projects locations processors create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDocumentaiV1Processor result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn documentai_projects_locations_processors_create(
        &self,
        args: &DocumentaiProjectsLocationsProcessorsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDocumentaiV1Processor, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = documentai_projects_locations_processors_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = documentai_projects_locations_processors_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Documentai projects locations processors delete.
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
    pub fn documentai_projects_locations_processors_delete(
        &self,
        args: &DocumentaiProjectsLocationsProcessorsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = documentai_projects_locations_processors_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = documentai_projects_locations_processors_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Documentai projects locations processors disable.
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
    pub fn documentai_projects_locations_processors_disable(
        &self,
        args: &DocumentaiProjectsLocationsProcessorsDisableArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = documentai_projects_locations_processors_disable_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = documentai_projects_locations_processors_disable_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Documentai projects locations processors enable.
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
    pub fn documentai_projects_locations_processors_enable(
        &self,
        args: &DocumentaiProjectsLocationsProcessorsEnableArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = documentai_projects_locations_processors_enable_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = documentai_projects_locations_processors_enable_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Documentai projects locations processors process.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDocumentaiV1ProcessResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn documentai_projects_locations_processors_process(
        &self,
        args: &DocumentaiProjectsLocationsProcessorsProcessArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDocumentaiV1ProcessResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = documentai_projects_locations_processors_process_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = documentai_projects_locations_processors_process_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Documentai projects locations processors set default processor version.
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
    pub fn documentai_projects_locations_processors_set_default_processor_version(
        &self,
        args: &DocumentaiProjectsLocationsProcessorsSetDefaultProcessorVersionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = documentai_projects_locations_processors_set_default_processor_version_builder(
            &self.http_client,
            &args.processor,
        )
        .map_err(ProviderError::Api)?;

        let task = documentai_projects_locations_processors_set_default_processor_version_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Documentai projects locations processors human review config review document.
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
    pub fn documentai_projects_locations_processors_human_review_config_review_document(
        &self,
        args: &DocumentaiProjectsLocationsProcessorsHumanReviewConfigReviewDocumentArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = documentai_projects_locations_processors_human_review_config_review_document_builder(
            &self.http_client,
            &args.humanReviewConfig,
        )
        .map_err(ProviderError::Api)?;

        let task = documentai_projects_locations_processors_human_review_config_review_document_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Documentai projects locations processors processor versions batch process.
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
    pub fn documentai_projects_locations_processors_processor_versions_batch_process(
        &self,
        args: &DocumentaiProjectsLocationsProcessorsProcessorVersionsBatchProcessArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = documentai_projects_locations_processors_processor_versions_batch_process_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = documentai_projects_locations_processors_processor_versions_batch_process_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Documentai projects locations processors processor versions delete.
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
    pub fn documentai_projects_locations_processors_processor_versions_delete(
        &self,
        args: &DocumentaiProjectsLocationsProcessorsProcessorVersionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = documentai_projects_locations_processors_processor_versions_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = documentai_projects_locations_processors_processor_versions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Documentai projects locations processors processor versions deploy.
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
    pub fn documentai_projects_locations_processors_processor_versions_deploy(
        &self,
        args: &DocumentaiProjectsLocationsProcessorsProcessorVersionsDeployArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = documentai_projects_locations_processors_processor_versions_deploy_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = documentai_projects_locations_processors_processor_versions_deploy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Documentai projects locations processors processor versions evaluate processor version.
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
    pub fn documentai_projects_locations_processors_processor_versions_evaluate_processor_version(
        &self,
        args: &DocumentaiProjectsLocationsProcessorsProcessorVersionsEvaluateProcessorVersionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = documentai_projects_locations_processors_processor_versions_evaluate_processor_version_builder(
            &self.http_client,
            &args.processorVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = documentai_projects_locations_processors_processor_versions_evaluate_processor_version_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Documentai projects locations processors processor versions process.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDocumentaiV1ProcessResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn documentai_projects_locations_processors_processor_versions_process(
        &self,
        args: &DocumentaiProjectsLocationsProcessorsProcessorVersionsProcessArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDocumentaiV1ProcessResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = documentai_projects_locations_processors_processor_versions_process_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = documentai_projects_locations_processors_processor_versions_process_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Documentai projects locations processors processor versions train.
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
    pub fn documentai_projects_locations_processors_processor_versions_train(
        &self,
        args: &DocumentaiProjectsLocationsProcessorsProcessorVersionsTrainArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = documentai_projects_locations_processors_processor_versions_train_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = documentai_projects_locations_processors_processor_versions_train_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Documentai projects locations processors processor versions undeploy.
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
    pub fn documentai_projects_locations_processors_processor_versions_undeploy(
        &self,
        args: &DocumentaiProjectsLocationsProcessorsProcessorVersionsUndeployArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = documentai_projects_locations_processors_processor_versions_undeploy_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = documentai_projects_locations_processors_processor_versions_undeploy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Documentai projects locations schemas create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDocumentaiV1NextSchema result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn documentai_projects_locations_schemas_create(
        &self,
        args: &DocumentaiProjectsLocationsSchemasCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDocumentaiV1NextSchema, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = documentai_projects_locations_schemas_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = documentai_projects_locations_schemas_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Documentai projects locations schemas delete.
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
    pub fn documentai_projects_locations_schemas_delete(
        &self,
        args: &DocumentaiProjectsLocationsSchemasDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = documentai_projects_locations_schemas_delete_builder(
            &self.http_client,
            &args.name,
            &args.force,
        )
        .map_err(ProviderError::Api)?;

        let task = documentai_projects_locations_schemas_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Documentai projects locations schemas patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDocumentaiV1NextSchema result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn documentai_projects_locations_schemas_patch(
        &self,
        args: &DocumentaiProjectsLocationsSchemasPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDocumentaiV1NextSchema, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = documentai_projects_locations_schemas_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = documentai_projects_locations_schemas_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Documentai projects locations schemas schema versions create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDocumentaiV1SchemaVersion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn documentai_projects_locations_schemas_schema_versions_create(
        &self,
        args: &DocumentaiProjectsLocationsSchemasSchemaVersionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDocumentaiV1SchemaVersion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = documentai_projects_locations_schemas_schema_versions_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = documentai_projects_locations_schemas_schema_versions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Documentai projects locations schemas schema versions delete.
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
    pub fn documentai_projects_locations_schemas_schema_versions_delete(
        &self,
        args: &DocumentaiProjectsLocationsSchemasSchemaVersionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = documentai_projects_locations_schemas_schema_versions_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = documentai_projects_locations_schemas_schema_versions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Documentai projects locations schemas schema versions generate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDocumentaiV1GenerateSchemaVersionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn documentai_projects_locations_schemas_schema_versions_generate(
        &self,
        args: &DocumentaiProjectsLocationsSchemasSchemaVersionsGenerateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDocumentaiV1GenerateSchemaVersionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = documentai_projects_locations_schemas_schema_versions_generate_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = documentai_projects_locations_schemas_schema_versions_generate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Documentai projects locations schemas schema versions patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDocumentaiV1SchemaVersion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn documentai_projects_locations_schemas_schema_versions_patch(
        &self,
        args: &DocumentaiProjectsLocationsSchemasSchemaVersionsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDocumentaiV1SchemaVersion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = documentai_projects_locations_schemas_schema_versions_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = documentai_projects_locations_schemas_schema_versions_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
