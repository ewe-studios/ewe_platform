//! RapidmigrationassessmentProvider - State-aware rapidmigrationassessment API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       rapidmigrationassessment API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::rapidmigrationassessment::{
    rapidmigrationassessment_projects_locations_get_builder, rapidmigrationassessment_projects_locations_get_task,
    rapidmigrationassessment_projects_locations_list_builder, rapidmigrationassessment_projects_locations_list_task,
    rapidmigrationassessment_projects_locations_annotations_create_builder, rapidmigrationassessment_projects_locations_annotations_create_task,
    rapidmigrationassessment_projects_locations_annotations_get_builder, rapidmigrationassessment_projects_locations_annotations_get_task,
    rapidmigrationassessment_projects_locations_collectors_create_builder, rapidmigrationassessment_projects_locations_collectors_create_task,
    rapidmigrationassessment_projects_locations_collectors_delete_builder, rapidmigrationassessment_projects_locations_collectors_delete_task,
    rapidmigrationassessment_projects_locations_collectors_get_builder, rapidmigrationassessment_projects_locations_collectors_get_task,
    rapidmigrationassessment_projects_locations_collectors_list_builder, rapidmigrationassessment_projects_locations_collectors_list_task,
    rapidmigrationassessment_projects_locations_collectors_patch_builder, rapidmigrationassessment_projects_locations_collectors_patch_task,
    rapidmigrationassessment_projects_locations_collectors_pause_builder, rapidmigrationassessment_projects_locations_collectors_pause_task,
    rapidmigrationassessment_projects_locations_collectors_register_builder, rapidmigrationassessment_projects_locations_collectors_register_task,
    rapidmigrationassessment_projects_locations_collectors_resume_builder, rapidmigrationassessment_projects_locations_collectors_resume_task,
    rapidmigrationassessment_projects_locations_operations_cancel_builder, rapidmigrationassessment_projects_locations_operations_cancel_task,
    rapidmigrationassessment_projects_locations_operations_delete_builder, rapidmigrationassessment_projects_locations_operations_delete_task,
    rapidmigrationassessment_projects_locations_operations_get_builder, rapidmigrationassessment_projects_locations_operations_get_task,
    rapidmigrationassessment_projects_locations_operations_list_builder, rapidmigrationassessment_projects_locations_operations_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::rapidmigrationassessment::Annotation;
use crate::providers::gcp::clients::rapidmigrationassessment::Collector;
use crate::providers::gcp::clients::rapidmigrationassessment::Empty;
use crate::providers::gcp::clients::rapidmigrationassessment::ListCollectorsResponse;
use crate::providers::gcp::clients::rapidmigrationassessment::ListLocationsResponse;
use crate::providers::gcp::clients::rapidmigrationassessment::ListOperationsResponse;
use crate::providers::gcp::clients::rapidmigrationassessment::Location;
use crate::providers::gcp::clients::rapidmigrationassessment::Operation;
use crate::providers::gcp::clients::rapidmigrationassessment::RapidmigrationassessmentProjectsLocationsAnnotationsCreateArgs;
use crate::providers::gcp::clients::rapidmigrationassessment::RapidmigrationassessmentProjectsLocationsAnnotationsGetArgs;
use crate::providers::gcp::clients::rapidmigrationassessment::RapidmigrationassessmentProjectsLocationsCollectorsCreateArgs;
use crate::providers::gcp::clients::rapidmigrationassessment::RapidmigrationassessmentProjectsLocationsCollectorsDeleteArgs;
use crate::providers::gcp::clients::rapidmigrationassessment::RapidmigrationassessmentProjectsLocationsCollectorsGetArgs;
use crate::providers::gcp::clients::rapidmigrationassessment::RapidmigrationassessmentProjectsLocationsCollectorsListArgs;
use crate::providers::gcp::clients::rapidmigrationassessment::RapidmigrationassessmentProjectsLocationsCollectorsPatchArgs;
use crate::providers::gcp::clients::rapidmigrationassessment::RapidmigrationassessmentProjectsLocationsCollectorsPauseArgs;
use crate::providers::gcp::clients::rapidmigrationassessment::RapidmigrationassessmentProjectsLocationsCollectorsRegisterArgs;
use crate::providers::gcp::clients::rapidmigrationassessment::RapidmigrationassessmentProjectsLocationsCollectorsResumeArgs;
use crate::providers::gcp::clients::rapidmigrationassessment::RapidmigrationassessmentProjectsLocationsGetArgs;
use crate::providers::gcp::clients::rapidmigrationassessment::RapidmigrationassessmentProjectsLocationsListArgs;
use crate::providers::gcp::clients::rapidmigrationassessment::RapidmigrationassessmentProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::rapidmigrationassessment::RapidmigrationassessmentProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::rapidmigrationassessment::RapidmigrationassessmentProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::rapidmigrationassessment::RapidmigrationassessmentProjectsLocationsOperationsListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// RapidmigrationassessmentProvider with automatic state tracking.
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
/// let provider = RapidmigrationassessmentProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct RapidmigrationassessmentProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> RapidmigrationassessmentProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new RapidmigrationassessmentProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Rapidmigrationassessment projects locations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Location result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn rapidmigrationassessment_projects_locations_get(
        &self,
        args: &RapidmigrationassessmentProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = rapidmigrationassessment_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = rapidmigrationassessment_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Rapidmigrationassessment projects locations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListLocationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn rapidmigrationassessment_projects_locations_list(
        &self,
        args: &RapidmigrationassessmentProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = rapidmigrationassessment_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = rapidmigrationassessment_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Rapidmigrationassessment projects locations annotations create.
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
    pub fn rapidmigrationassessment_projects_locations_annotations_create(
        &self,
        args: &RapidmigrationassessmentProjectsLocationsAnnotationsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = rapidmigrationassessment_projects_locations_annotations_create_builder(
            &self.http_client,
            &args.parent,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = rapidmigrationassessment_projects_locations_annotations_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Rapidmigrationassessment projects locations annotations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Annotation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn rapidmigrationassessment_projects_locations_annotations_get(
        &self,
        args: &RapidmigrationassessmentProjectsLocationsAnnotationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Annotation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = rapidmigrationassessment_projects_locations_annotations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = rapidmigrationassessment_projects_locations_annotations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Rapidmigrationassessment projects locations collectors create.
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
    pub fn rapidmigrationassessment_projects_locations_collectors_create(
        &self,
        args: &RapidmigrationassessmentProjectsLocationsCollectorsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = rapidmigrationassessment_projects_locations_collectors_create_builder(
            &self.http_client,
            &args.parent,
            &args.collectorId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = rapidmigrationassessment_projects_locations_collectors_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Rapidmigrationassessment projects locations collectors delete.
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
    pub fn rapidmigrationassessment_projects_locations_collectors_delete(
        &self,
        args: &RapidmigrationassessmentProjectsLocationsCollectorsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = rapidmigrationassessment_projects_locations_collectors_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = rapidmigrationassessment_projects_locations_collectors_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Rapidmigrationassessment projects locations collectors get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Collector result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn rapidmigrationassessment_projects_locations_collectors_get(
        &self,
        args: &RapidmigrationassessmentProjectsLocationsCollectorsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Collector, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = rapidmigrationassessment_projects_locations_collectors_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = rapidmigrationassessment_projects_locations_collectors_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Rapidmigrationassessment projects locations collectors list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCollectorsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn rapidmigrationassessment_projects_locations_collectors_list(
        &self,
        args: &RapidmigrationassessmentProjectsLocationsCollectorsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCollectorsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = rapidmigrationassessment_projects_locations_collectors_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = rapidmigrationassessment_projects_locations_collectors_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Rapidmigrationassessment projects locations collectors patch.
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
    pub fn rapidmigrationassessment_projects_locations_collectors_patch(
        &self,
        args: &RapidmigrationassessmentProjectsLocationsCollectorsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = rapidmigrationassessment_projects_locations_collectors_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = rapidmigrationassessment_projects_locations_collectors_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Rapidmigrationassessment projects locations collectors pause.
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
    pub fn rapidmigrationassessment_projects_locations_collectors_pause(
        &self,
        args: &RapidmigrationassessmentProjectsLocationsCollectorsPauseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = rapidmigrationassessment_projects_locations_collectors_pause_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = rapidmigrationassessment_projects_locations_collectors_pause_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Rapidmigrationassessment projects locations collectors register.
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
    pub fn rapidmigrationassessment_projects_locations_collectors_register(
        &self,
        args: &RapidmigrationassessmentProjectsLocationsCollectorsRegisterArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = rapidmigrationassessment_projects_locations_collectors_register_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = rapidmigrationassessment_projects_locations_collectors_register_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Rapidmigrationassessment projects locations collectors resume.
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
    pub fn rapidmigrationassessment_projects_locations_collectors_resume(
        &self,
        args: &RapidmigrationassessmentProjectsLocationsCollectorsResumeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = rapidmigrationassessment_projects_locations_collectors_resume_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = rapidmigrationassessment_projects_locations_collectors_resume_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Rapidmigrationassessment projects locations operations cancel.
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
    pub fn rapidmigrationassessment_projects_locations_operations_cancel(
        &self,
        args: &RapidmigrationassessmentProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = rapidmigrationassessment_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = rapidmigrationassessment_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Rapidmigrationassessment projects locations operations delete.
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
    pub fn rapidmigrationassessment_projects_locations_operations_delete(
        &self,
        args: &RapidmigrationassessmentProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = rapidmigrationassessment_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = rapidmigrationassessment_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Rapidmigrationassessment projects locations operations get.
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
    pub fn rapidmigrationassessment_projects_locations_operations_get(
        &self,
        args: &RapidmigrationassessmentProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = rapidmigrationassessment_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = rapidmigrationassessment_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Rapidmigrationassessment projects locations operations list.
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
    pub fn rapidmigrationassessment_projects_locations_operations_list(
        &self,
        args: &RapidmigrationassessmentProjectsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = rapidmigrationassessment_projects_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = rapidmigrationassessment_projects_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
