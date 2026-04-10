//! RecommendationengineProvider - State-aware recommendationengine API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       recommendationengine API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::recommendationengine::{
    recommendationengine_projects_locations_catalogs_patch_builder, recommendationengine_projects_locations_catalogs_patch_task,
    recommendationengine_projects_locations_catalogs_catalog_items_create_builder, recommendationengine_projects_locations_catalogs_catalog_items_create_task,
    recommendationengine_projects_locations_catalogs_catalog_items_delete_builder, recommendationengine_projects_locations_catalogs_catalog_items_delete_task,
    recommendationengine_projects_locations_catalogs_catalog_items_import_builder, recommendationengine_projects_locations_catalogs_catalog_items_import_task,
    recommendationengine_projects_locations_catalogs_catalog_items_patch_builder, recommendationengine_projects_locations_catalogs_catalog_items_patch_task,
    recommendationengine_projects_locations_catalogs_event_stores_placements_predict_builder, recommendationengine_projects_locations_catalogs_event_stores_placements_predict_task,
    recommendationengine_projects_locations_catalogs_event_stores_prediction_api_key_registrations_create_builder, recommendationengine_projects_locations_catalogs_event_stores_prediction_api_key_registrations_create_task,
    recommendationengine_projects_locations_catalogs_event_stores_prediction_api_key_registrations_delete_builder, recommendationengine_projects_locations_catalogs_event_stores_prediction_api_key_registrations_delete_task,
    recommendationengine_projects_locations_catalogs_event_stores_user_events_import_builder, recommendationengine_projects_locations_catalogs_event_stores_user_events_import_task,
    recommendationengine_projects_locations_catalogs_event_stores_user_events_purge_builder, recommendationengine_projects_locations_catalogs_event_stores_user_events_purge_task,
    recommendationengine_projects_locations_catalogs_event_stores_user_events_rejoin_builder, recommendationengine_projects_locations_catalogs_event_stores_user_events_rejoin_task,
    recommendationengine_projects_locations_catalogs_event_stores_user_events_write_builder, recommendationengine_projects_locations_catalogs_event_stores_user_events_write_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::recommendationengine::GoogleCloudRecommendationengineV1beta1Catalog;
use crate::providers::gcp::clients::recommendationengine::GoogleCloudRecommendationengineV1beta1CatalogItem;
use crate::providers::gcp::clients::recommendationengine::GoogleCloudRecommendationengineV1beta1PredictResponse;
use crate::providers::gcp::clients::recommendationengine::GoogleCloudRecommendationengineV1beta1PredictionApiKeyRegistration;
use crate::providers::gcp::clients::recommendationengine::GoogleCloudRecommendationengineV1beta1UserEvent;
use crate::providers::gcp::clients::recommendationengine::GoogleLongrunningOperation;
use crate::providers::gcp::clients::recommendationengine::GoogleProtobufEmpty;
use crate::providers::gcp::clients::recommendationengine::RecommendationengineProjectsLocationsCatalogsCatalogItemsCreateArgs;
use crate::providers::gcp::clients::recommendationengine::RecommendationengineProjectsLocationsCatalogsCatalogItemsDeleteArgs;
use crate::providers::gcp::clients::recommendationengine::RecommendationengineProjectsLocationsCatalogsCatalogItemsImportArgs;
use crate::providers::gcp::clients::recommendationengine::RecommendationengineProjectsLocationsCatalogsCatalogItemsPatchArgs;
use crate::providers::gcp::clients::recommendationengine::RecommendationengineProjectsLocationsCatalogsEventStoresPlacementsPredictArgs;
use crate::providers::gcp::clients::recommendationengine::RecommendationengineProjectsLocationsCatalogsEventStoresPredictionApiKeyRegistrationsCreateArgs;
use crate::providers::gcp::clients::recommendationengine::RecommendationengineProjectsLocationsCatalogsEventStoresPredictionApiKeyRegistrationsDeleteArgs;
use crate::providers::gcp::clients::recommendationengine::RecommendationengineProjectsLocationsCatalogsEventStoresUserEventsImportArgs;
use crate::providers::gcp::clients::recommendationengine::RecommendationengineProjectsLocationsCatalogsEventStoresUserEventsPurgeArgs;
use crate::providers::gcp::clients::recommendationengine::RecommendationengineProjectsLocationsCatalogsEventStoresUserEventsRejoinArgs;
use crate::providers::gcp::clients::recommendationengine::RecommendationengineProjectsLocationsCatalogsEventStoresUserEventsWriteArgs;
use crate::providers::gcp::clients::recommendationengine::RecommendationengineProjectsLocationsCatalogsPatchArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// RecommendationengineProvider with automatic state tracking.
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
/// let provider = RecommendationengineProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct RecommendationengineProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> RecommendationengineProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new RecommendationengineProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Recommendationengine projects locations catalogs patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecommendationengineV1beta1Catalog result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn recommendationengine_projects_locations_catalogs_patch(
        &self,
        args: &RecommendationengineProjectsLocationsCatalogsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommendationengineV1beta1Catalog, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommendationengine_projects_locations_catalogs_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = recommendationengine_projects_locations_catalogs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recommendationengine projects locations catalogs catalog items create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecommendationengineV1beta1CatalogItem result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn recommendationengine_projects_locations_catalogs_catalog_items_create(
        &self,
        args: &RecommendationengineProjectsLocationsCatalogsCatalogItemsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommendationengineV1beta1CatalogItem, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommendationengine_projects_locations_catalogs_catalog_items_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = recommendationengine_projects_locations_catalogs_catalog_items_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recommendationengine projects locations catalogs catalog items delete.
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
    pub fn recommendationengine_projects_locations_catalogs_catalog_items_delete(
        &self,
        args: &RecommendationengineProjectsLocationsCatalogsCatalogItemsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommendationengine_projects_locations_catalogs_catalog_items_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = recommendationengine_projects_locations_catalogs_catalog_items_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recommendationengine projects locations catalogs catalog items import.
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
    pub fn recommendationengine_projects_locations_catalogs_catalog_items_import(
        &self,
        args: &RecommendationengineProjectsLocationsCatalogsCatalogItemsImportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommendationengine_projects_locations_catalogs_catalog_items_import_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = recommendationengine_projects_locations_catalogs_catalog_items_import_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recommendationengine projects locations catalogs catalog items patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecommendationengineV1beta1CatalogItem result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn recommendationengine_projects_locations_catalogs_catalog_items_patch(
        &self,
        args: &RecommendationengineProjectsLocationsCatalogsCatalogItemsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommendationengineV1beta1CatalogItem, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommendationengine_projects_locations_catalogs_catalog_items_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = recommendationengine_projects_locations_catalogs_catalog_items_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recommendationengine projects locations catalogs event stores placements predict.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecommendationengineV1beta1PredictResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn recommendationengine_projects_locations_catalogs_event_stores_placements_predict(
        &self,
        args: &RecommendationengineProjectsLocationsCatalogsEventStoresPlacementsPredictArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommendationengineV1beta1PredictResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommendationengine_projects_locations_catalogs_event_stores_placements_predict_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = recommendationengine_projects_locations_catalogs_event_stores_placements_predict_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recommendationengine projects locations catalogs event stores prediction api key registrations create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecommendationengineV1beta1PredictionApiKeyRegistration result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn recommendationengine_projects_locations_catalogs_event_stores_prediction_api_key_registrations_create(
        &self,
        args: &RecommendationengineProjectsLocationsCatalogsEventStoresPredictionApiKeyRegistrationsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommendationengineV1beta1PredictionApiKeyRegistration, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommendationengine_projects_locations_catalogs_event_stores_prediction_api_key_registrations_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = recommendationengine_projects_locations_catalogs_event_stores_prediction_api_key_registrations_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recommendationengine projects locations catalogs event stores prediction api key registrations delete.
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
    pub fn recommendationengine_projects_locations_catalogs_event_stores_prediction_api_key_registrations_delete(
        &self,
        args: &RecommendationengineProjectsLocationsCatalogsEventStoresPredictionApiKeyRegistrationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommendationengine_projects_locations_catalogs_event_stores_prediction_api_key_registrations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = recommendationengine_projects_locations_catalogs_event_stores_prediction_api_key_registrations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recommendationengine projects locations catalogs event stores user events import.
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
    pub fn recommendationengine_projects_locations_catalogs_event_stores_user_events_import(
        &self,
        args: &RecommendationengineProjectsLocationsCatalogsEventStoresUserEventsImportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommendationengine_projects_locations_catalogs_event_stores_user_events_import_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = recommendationengine_projects_locations_catalogs_event_stores_user_events_import_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recommendationengine projects locations catalogs event stores user events purge.
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
    pub fn recommendationengine_projects_locations_catalogs_event_stores_user_events_purge(
        &self,
        args: &RecommendationengineProjectsLocationsCatalogsEventStoresUserEventsPurgeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommendationengine_projects_locations_catalogs_event_stores_user_events_purge_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = recommendationengine_projects_locations_catalogs_event_stores_user_events_purge_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recommendationengine projects locations catalogs event stores user events rejoin.
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
    pub fn recommendationengine_projects_locations_catalogs_event_stores_user_events_rejoin(
        &self,
        args: &RecommendationengineProjectsLocationsCatalogsEventStoresUserEventsRejoinArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommendationengine_projects_locations_catalogs_event_stores_user_events_rejoin_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = recommendationengine_projects_locations_catalogs_event_stores_user_events_rejoin_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recommendationengine projects locations catalogs event stores user events write.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecommendationengineV1beta1UserEvent result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn recommendationengine_projects_locations_catalogs_event_stores_user_events_write(
        &self,
        args: &RecommendationengineProjectsLocationsCatalogsEventStoresUserEventsWriteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommendationengineV1beta1UserEvent, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommendationengine_projects_locations_catalogs_event_stores_user_events_write_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = recommendationengine_projects_locations_catalogs_event_stores_user_events_write_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
