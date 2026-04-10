//! VisionProvider - State-aware vision API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       vision API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::vision::{
    vision_files_annotate_builder, vision_files_annotate_task,
    vision_files_async_batch_annotate_builder, vision_files_async_batch_annotate_task,
    vision_images_annotate_builder, vision_images_annotate_task,
    vision_images_async_batch_annotate_builder, vision_images_async_batch_annotate_task,
    vision_locations_operations_get_builder, vision_locations_operations_get_task,
    vision_operations_cancel_builder, vision_operations_cancel_task,
    vision_operations_delete_builder, vision_operations_delete_task,
    vision_operations_get_builder, vision_operations_get_task,
    vision_operations_list_builder, vision_operations_list_task,
    vision_projects_files_annotate_builder, vision_projects_files_annotate_task,
    vision_projects_files_async_batch_annotate_builder, vision_projects_files_async_batch_annotate_task,
    vision_projects_images_annotate_builder, vision_projects_images_annotate_task,
    vision_projects_images_async_batch_annotate_builder, vision_projects_images_async_batch_annotate_task,
    vision_projects_locations_files_annotate_builder, vision_projects_locations_files_annotate_task,
    vision_projects_locations_files_async_batch_annotate_builder, vision_projects_locations_files_async_batch_annotate_task,
    vision_projects_locations_images_annotate_builder, vision_projects_locations_images_annotate_task,
    vision_projects_locations_images_async_batch_annotate_builder, vision_projects_locations_images_async_batch_annotate_task,
    vision_projects_locations_operations_get_builder, vision_projects_locations_operations_get_task,
    vision_projects_locations_product_sets_add_product_builder, vision_projects_locations_product_sets_add_product_task,
    vision_projects_locations_product_sets_create_builder, vision_projects_locations_product_sets_create_task,
    vision_projects_locations_product_sets_delete_builder, vision_projects_locations_product_sets_delete_task,
    vision_projects_locations_product_sets_get_builder, vision_projects_locations_product_sets_get_task,
    vision_projects_locations_product_sets_import_builder, vision_projects_locations_product_sets_import_task,
    vision_projects_locations_product_sets_list_builder, vision_projects_locations_product_sets_list_task,
    vision_projects_locations_product_sets_patch_builder, vision_projects_locations_product_sets_patch_task,
    vision_projects_locations_product_sets_remove_product_builder, vision_projects_locations_product_sets_remove_product_task,
    vision_projects_locations_product_sets_products_list_builder, vision_projects_locations_product_sets_products_list_task,
    vision_projects_locations_products_create_builder, vision_projects_locations_products_create_task,
    vision_projects_locations_products_delete_builder, vision_projects_locations_products_delete_task,
    vision_projects_locations_products_get_builder, vision_projects_locations_products_get_task,
    vision_projects_locations_products_list_builder, vision_projects_locations_products_list_task,
    vision_projects_locations_products_patch_builder, vision_projects_locations_products_patch_task,
    vision_projects_locations_products_purge_builder, vision_projects_locations_products_purge_task,
    vision_projects_locations_products_reference_images_create_builder, vision_projects_locations_products_reference_images_create_task,
    vision_projects_locations_products_reference_images_delete_builder, vision_projects_locations_products_reference_images_delete_task,
    vision_projects_locations_products_reference_images_get_builder, vision_projects_locations_products_reference_images_get_task,
    vision_projects_locations_products_reference_images_list_builder, vision_projects_locations_products_reference_images_list_task,
    vision_projects_operations_get_builder, vision_projects_operations_get_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::vision::BatchAnnotateFilesResponse;
use crate::providers::gcp::clients::vision::BatchAnnotateImagesResponse;
use crate::providers::gcp::clients::vision::Empty;
use crate::providers::gcp::clients::vision::ListOperationsResponse;
use crate::providers::gcp::clients::vision::ListProductSetsResponse;
use crate::providers::gcp::clients::vision::ListProductsInProductSetResponse;
use crate::providers::gcp::clients::vision::ListProductsResponse;
use crate::providers::gcp::clients::vision::ListReferenceImagesResponse;
use crate::providers::gcp::clients::vision::Operation;
use crate::providers::gcp::clients::vision::Product;
use crate::providers::gcp::clients::vision::ProductSet;
use crate::providers::gcp::clients::vision::ReferenceImage;
use crate::providers::gcp::clients::vision::VisionFilesAnnotateArgs;
use crate::providers::gcp::clients::vision::VisionFilesAsyncBatchAnnotateArgs;
use crate::providers::gcp::clients::vision::VisionImagesAnnotateArgs;
use crate::providers::gcp::clients::vision::VisionImagesAsyncBatchAnnotateArgs;
use crate::providers::gcp::clients::vision::VisionLocationsOperationsGetArgs;
use crate::providers::gcp::clients::vision::VisionOperationsCancelArgs;
use crate::providers::gcp::clients::vision::VisionOperationsDeleteArgs;
use crate::providers::gcp::clients::vision::VisionOperationsGetArgs;
use crate::providers::gcp::clients::vision::VisionOperationsListArgs;
use crate::providers::gcp::clients::vision::VisionProjectsFilesAnnotateArgs;
use crate::providers::gcp::clients::vision::VisionProjectsFilesAsyncBatchAnnotateArgs;
use crate::providers::gcp::clients::vision::VisionProjectsImagesAnnotateArgs;
use crate::providers::gcp::clients::vision::VisionProjectsImagesAsyncBatchAnnotateArgs;
use crate::providers::gcp::clients::vision::VisionProjectsLocationsFilesAnnotateArgs;
use crate::providers::gcp::clients::vision::VisionProjectsLocationsFilesAsyncBatchAnnotateArgs;
use crate::providers::gcp::clients::vision::VisionProjectsLocationsImagesAnnotateArgs;
use crate::providers::gcp::clients::vision::VisionProjectsLocationsImagesAsyncBatchAnnotateArgs;
use crate::providers::gcp::clients::vision::VisionProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::vision::VisionProjectsLocationsProductSetsAddProductArgs;
use crate::providers::gcp::clients::vision::VisionProjectsLocationsProductSetsCreateArgs;
use crate::providers::gcp::clients::vision::VisionProjectsLocationsProductSetsDeleteArgs;
use crate::providers::gcp::clients::vision::VisionProjectsLocationsProductSetsGetArgs;
use crate::providers::gcp::clients::vision::VisionProjectsLocationsProductSetsImportArgs;
use crate::providers::gcp::clients::vision::VisionProjectsLocationsProductSetsListArgs;
use crate::providers::gcp::clients::vision::VisionProjectsLocationsProductSetsPatchArgs;
use crate::providers::gcp::clients::vision::VisionProjectsLocationsProductSetsProductsListArgs;
use crate::providers::gcp::clients::vision::VisionProjectsLocationsProductSetsRemoveProductArgs;
use crate::providers::gcp::clients::vision::VisionProjectsLocationsProductsCreateArgs;
use crate::providers::gcp::clients::vision::VisionProjectsLocationsProductsDeleteArgs;
use crate::providers::gcp::clients::vision::VisionProjectsLocationsProductsGetArgs;
use crate::providers::gcp::clients::vision::VisionProjectsLocationsProductsListArgs;
use crate::providers::gcp::clients::vision::VisionProjectsLocationsProductsPatchArgs;
use crate::providers::gcp::clients::vision::VisionProjectsLocationsProductsPurgeArgs;
use crate::providers::gcp::clients::vision::VisionProjectsLocationsProductsReferenceImagesCreateArgs;
use crate::providers::gcp::clients::vision::VisionProjectsLocationsProductsReferenceImagesDeleteArgs;
use crate::providers::gcp::clients::vision::VisionProjectsLocationsProductsReferenceImagesGetArgs;
use crate::providers::gcp::clients::vision::VisionProjectsLocationsProductsReferenceImagesListArgs;
use crate::providers::gcp::clients::vision::VisionProjectsOperationsGetArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// VisionProvider with automatic state tracking.
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
/// let provider = VisionProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct VisionProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> VisionProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new VisionProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Vision files annotate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchAnnotateFilesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vision_files_annotate(
        &self,
        args: &VisionFilesAnnotateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchAnnotateFilesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vision_files_annotate_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = vision_files_annotate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vision files async batch annotate.
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
    pub fn vision_files_async_batch_annotate(
        &self,
        args: &VisionFilesAsyncBatchAnnotateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vision_files_async_batch_annotate_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = vision_files_async_batch_annotate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vision images annotate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchAnnotateImagesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vision_images_annotate(
        &self,
        args: &VisionImagesAnnotateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchAnnotateImagesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vision_images_annotate_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = vision_images_annotate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vision images async batch annotate.
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
    pub fn vision_images_async_batch_annotate(
        &self,
        args: &VisionImagesAsyncBatchAnnotateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vision_images_async_batch_annotate_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = vision_images_async_batch_annotate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vision locations operations get.
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
    pub fn vision_locations_operations_get(
        &self,
        args: &VisionLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vision_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vision_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vision operations cancel.
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
    pub fn vision_operations_cancel(
        &self,
        args: &VisionOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vision_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vision_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vision operations delete.
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
    pub fn vision_operations_delete(
        &self,
        args: &VisionOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vision_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vision_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vision operations get.
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
    pub fn vision_operations_get(
        &self,
        args: &VisionOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vision_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vision_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vision operations list.
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
    pub fn vision_operations_list(
        &self,
        args: &VisionOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vision_operations_list_builder(
            &self.http_client,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = vision_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vision projects files annotate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchAnnotateFilesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vision_projects_files_annotate(
        &self,
        args: &VisionProjectsFilesAnnotateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchAnnotateFilesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vision_projects_files_annotate_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = vision_projects_files_annotate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vision projects files async batch annotate.
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
    pub fn vision_projects_files_async_batch_annotate(
        &self,
        args: &VisionProjectsFilesAsyncBatchAnnotateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vision_projects_files_async_batch_annotate_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = vision_projects_files_async_batch_annotate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vision projects images annotate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchAnnotateImagesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vision_projects_images_annotate(
        &self,
        args: &VisionProjectsImagesAnnotateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchAnnotateImagesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vision_projects_images_annotate_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = vision_projects_images_annotate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vision projects images async batch annotate.
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
    pub fn vision_projects_images_async_batch_annotate(
        &self,
        args: &VisionProjectsImagesAsyncBatchAnnotateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vision_projects_images_async_batch_annotate_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = vision_projects_images_async_batch_annotate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vision projects locations files annotate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchAnnotateFilesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vision_projects_locations_files_annotate(
        &self,
        args: &VisionProjectsLocationsFilesAnnotateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchAnnotateFilesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vision_projects_locations_files_annotate_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = vision_projects_locations_files_annotate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vision projects locations files async batch annotate.
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
    pub fn vision_projects_locations_files_async_batch_annotate(
        &self,
        args: &VisionProjectsLocationsFilesAsyncBatchAnnotateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vision_projects_locations_files_async_batch_annotate_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = vision_projects_locations_files_async_batch_annotate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vision projects locations images annotate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchAnnotateImagesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vision_projects_locations_images_annotate(
        &self,
        args: &VisionProjectsLocationsImagesAnnotateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchAnnotateImagesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vision_projects_locations_images_annotate_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = vision_projects_locations_images_annotate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vision projects locations images async batch annotate.
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
    pub fn vision_projects_locations_images_async_batch_annotate(
        &self,
        args: &VisionProjectsLocationsImagesAsyncBatchAnnotateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vision_projects_locations_images_async_batch_annotate_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = vision_projects_locations_images_async_batch_annotate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vision projects locations operations get.
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
    pub fn vision_projects_locations_operations_get(
        &self,
        args: &VisionProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vision_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vision_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vision projects locations product sets add product.
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
    pub fn vision_projects_locations_product_sets_add_product(
        &self,
        args: &VisionProjectsLocationsProductSetsAddProductArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vision_projects_locations_product_sets_add_product_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vision_projects_locations_product_sets_add_product_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vision projects locations product sets create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProductSet result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vision_projects_locations_product_sets_create(
        &self,
        args: &VisionProjectsLocationsProductSetsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProductSet, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vision_projects_locations_product_sets_create_builder(
            &self.http_client,
            &args.parent,
            &args.productSetId,
        )
        .map_err(ProviderError::Api)?;

        let task = vision_projects_locations_product_sets_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vision projects locations product sets delete.
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
    pub fn vision_projects_locations_product_sets_delete(
        &self,
        args: &VisionProjectsLocationsProductSetsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vision_projects_locations_product_sets_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vision_projects_locations_product_sets_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vision projects locations product sets get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProductSet result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vision_projects_locations_product_sets_get(
        &self,
        args: &VisionProjectsLocationsProductSetsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProductSet, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vision_projects_locations_product_sets_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vision_projects_locations_product_sets_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vision projects locations product sets import.
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
    pub fn vision_projects_locations_product_sets_import(
        &self,
        args: &VisionProjectsLocationsProductSetsImportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vision_projects_locations_product_sets_import_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = vision_projects_locations_product_sets_import_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vision projects locations product sets list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListProductSetsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vision_projects_locations_product_sets_list(
        &self,
        args: &VisionProjectsLocationsProductSetsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListProductSetsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vision_projects_locations_product_sets_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = vision_projects_locations_product_sets_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vision projects locations product sets patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProductSet result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vision_projects_locations_product_sets_patch(
        &self,
        args: &VisionProjectsLocationsProductSetsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProductSet, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vision_projects_locations_product_sets_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = vision_projects_locations_product_sets_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vision projects locations product sets remove product.
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
    pub fn vision_projects_locations_product_sets_remove_product(
        &self,
        args: &VisionProjectsLocationsProductSetsRemoveProductArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vision_projects_locations_product_sets_remove_product_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vision_projects_locations_product_sets_remove_product_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vision projects locations product sets products list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListProductsInProductSetResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vision_projects_locations_product_sets_products_list(
        &self,
        args: &VisionProjectsLocationsProductSetsProductsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListProductsInProductSetResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vision_projects_locations_product_sets_products_list_builder(
            &self.http_client,
            &args.name,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = vision_projects_locations_product_sets_products_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vision projects locations products create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Product result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vision_projects_locations_products_create(
        &self,
        args: &VisionProjectsLocationsProductsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Product, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vision_projects_locations_products_create_builder(
            &self.http_client,
            &args.parent,
            &args.productId,
        )
        .map_err(ProviderError::Api)?;

        let task = vision_projects_locations_products_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vision projects locations products delete.
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
    pub fn vision_projects_locations_products_delete(
        &self,
        args: &VisionProjectsLocationsProductsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vision_projects_locations_products_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vision_projects_locations_products_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vision projects locations products get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Product result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vision_projects_locations_products_get(
        &self,
        args: &VisionProjectsLocationsProductsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Product, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vision_projects_locations_products_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vision_projects_locations_products_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vision projects locations products list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListProductsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vision_projects_locations_products_list(
        &self,
        args: &VisionProjectsLocationsProductsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListProductsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vision_projects_locations_products_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = vision_projects_locations_products_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vision projects locations products patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Product result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vision_projects_locations_products_patch(
        &self,
        args: &VisionProjectsLocationsProductsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Product, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vision_projects_locations_products_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = vision_projects_locations_products_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vision projects locations products purge.
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
    pub fn vision_projects_locations_products_purge(
        &self,
        args: &VisionProjectsLocationsProductsPurgeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vision_projects_locations_products_purge_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = vision_projects_locations_products_purge_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vision projects locations products reference images create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ReferenceImage result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vision_projects_locations_products_reference_images_create(
        &self,
        args: &VisionProjectsLocationsProductsReferenceImagesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ReferenceImage, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vision_projects_locations_products_reference_images_create_builder(
            &self.http_client,
            &args.parent,
            &args.referenceImageId,
        )
        .map_err(ProviderError::Api)?;

        let task = vision_projects_locations_products_reference_images_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vision projects locations products reference images delete.
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
    pub fn vision_projects_locations_products_reference_images_delete(
        &self,
        args: &VisionProjectsLocationsProductsReferenceImagesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vision_projects_locations_products_reference_images_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vision_projects_locations_products_reference_images_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vision projects locations products reference images get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ReferenceImage result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vision_projects_locations_products_reference_images_get(
        &self,
        args: &VisionProjectsLocationsProductsReferenceImagesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ReferenceImage, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vision_projects_locations_products_reference_images_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vision_projects_locations_products_reference_images_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vision projects locations products reference images list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListReferenceImagesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vision_projects_locations_products_reference_images_list(
        &self,
        args: &VisionProjectsLocationsProductsReferenceImagesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListReferenceImagesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vision_projects_locations_products_reference_images_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = vision_projects_locations_products_reference_images_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vision projects operations get.
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
    pub fn vision_projects_operations_get(
        &self,
        args: &VisionProjectsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vision_projects_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vision_projects_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
