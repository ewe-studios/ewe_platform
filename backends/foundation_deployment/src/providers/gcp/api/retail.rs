//! RetailProvider - State-aware retail API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       retail API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::retail::{
    retail_projects_locations_catalogs_export_analytics_metrics_builder, retail_projects_locations_catalogs_export_analytics_metrics_task,
    retail_projects_locations_catalogs_patch_builder, retail_projects_locations_catalogs_patch_task,
    retail_projects_locations_catalogs_set_default_branch_builder, retail_projects_locations_catalogs_set_default_branch_task,
    retail_projects_locations_catalogs_update_attributes_config_builder, retail_projects_locations_catalogs_update_attributes_config_task,
    retail_projects_locations_catalogs_update_completion_config_builder, retail_projects_locations_catalogs_update_completion_config_task,
    retail_projects_locations_catalogs_update_conversational_search_customization_config_builder, retail_projects_locations_catalogs_update_conversational_search_customization_config_task,
    retail_projects_locations_catalogs_update_generative_question_builder, retail_projects_locations_catalogs_update_generative_question_task,
    retail_projects_locations_catalogs_update_generative_question_feature_builder, retail_projects_locations_catalogs_update_generative_question_feature_task,
    retail_projects_locations_catalogs_attributes_config_add_catalog_attribute_builder, retail_projects_locations_catalogs_attributes_config_add_catalog_attribute_task,
    retail_projects_locations_catalogs_attributes_config_remove_catalog_attribute_builder, retail_projects_locations_catalogs_attributes_config_remove_catalog_attribute_task,
    retail_projects_locations_catalogs_attributes_config_replace_catalog_attribute_builder, retail_projects_locations_catalogs_attributes_config_replace_catalog_attribute_task,
    retail_projects_locations_catalogs_branches_products_add_fulfillment_places_builder, retail_projects_locations_catalogs_branches_products_add_fulfillment_places_task,
    retail_projects_locations_catalogs_branches_products_add_local_inventories_builder, retail_projects_locations_catalogs_branches_products_add_local_inventories_task,
    retail_projects_locations_catalogs_branches_products_create_builder, retail_projects_locations_catalogs_branches_products_create_task,
    retail_projects_locations_catalogs_branches_products_delete_builder, retail_projects_locations_catalogs_branches_products_delete_task,
    retail_projects_locations_catalogs_branches_products_import_builder, retail_projects_locations_catalogs_branches_products_import_task,
    retail_projects_locations_catalogs_branches_products_patch_builder, retail_projects_locations_catalogs_branches_products_patch_task,
    retail_projects_locations_catalogs_branches_products_purge_builder, retail_projects_locations_catalogs_branches_products_purge_task,
    retail_projects_locations_catalogs_branches_products_remove_fulfillment_places_builder, retail_projects_locations_catalogs_branches_products_remove_fulfillment_places_task,
    retail_projects_locations_catalogs_branches_products_remove_local_inventories_builder, retail_projects_locations_catalogs_branches_products_remove_local_inventories_task,
    retail_projects_locations_catalogs_branches_products_set_inventory_builder, retail_projects_locations_catalogs_branches_products_set_inventory_task,
    retail_projects_locations_catalogs_completion_data_import_builder, retail_projects_locations_catalogs_completion_data_import_task,
    retail_projects_locations_catalogs_controls_create_builder, retail_projects_locations_catalogs_controls_create_task,
    retail_projects_locations_catalogs_controls_delete_builder, retail_projects_locations_catalogs_controls_delete_task,
    retail_projects_locations_catalogs_controls_patch_builder, retail_projects_locations_catalogs_controls_patch_task,
    retail_projects_locations_catalogs_generative_question_batch_update_builder, retail_projects_locations_catalogs_generative_question_batch_update_task,
    retail_projects_locations_catalogs_models_create_builder, retail_projects_locations_catalogs_models_create_task,
    retail_projects_locations_catalogs_models_delete_builder, retail_projects_locations_catalogs_models_delete_task,
    retail_projects_locations_catalogs_models_patch_builder, retail_projects_locations_catalogs_models_patch_task,
    retail_projects_locations_catalogs_models_pause_builder, retail_projects_locations_catalogs_models_pause_task,
    retail_projects_locations_catalogs_models_resume_builder, retail_projects_locations_catalogs_models_resume_task,
    retail_projects_locations_catalogs_models_tune_builder, retail_projects_locations_catalogs_models_tune_task,
    retail_projects_locations_catalogs_placements_conversational_search_builder, retail_projects_locations_catalogs_placements_conversational_search_task,
    retail_projects_locations_catalogs_placements_predict_builder, retail_projects_locations_catalogs_placements_predict_task,
    retail_projects_locations_catalogs_placements_search_builder, retail_projects_locations_catalogs_placements_search_task,
    retail_projects_locations_catalogs_serving_configs_add_control_builder, retail_projects_locations_catalogs_serving_configs_add_control_task,
    retail_projects_locations_catalogs_serving_configs_conversational_search_builder, retail_projects_locations_catalogs_serving_configs_conversational_search_task,
    retail_projects_locations_catalogs_serving_configs_create_builder, retail_projects_locations_catalogs_serving_configs_create_task,
    retail_projects_locations_catalogs_serving_configs_delete_builder, retail_projects_locations_catalogs_serving_configs_delete_task,
    retail_projects_locations_catalogs_serving_configs_patch_builder, retail_projects_locations_catalogs_serving_configs_patch_task,
    retail_projects_locations_catalogs_serving_configs_predict_builder, retail_projects_locations_catalogs_serving_configs_predict_task,
    retail_projects_locations_catalogs_serving_configs_remove_control_builder, retail_projects_locations_catalogs_serving_configs_remove_control_task,
    retail_projects_locations_catalogs_serving_configs_search_builder, retail_projects_locations_catalogs_serving_configs_search_task,
    retail_projects_locations_catalogs_user_events_collect_builder, retail_projects_locations_catalogs_user_events_collect_task,
    retail_projects_locations_catalogs_user_events_import_builder, retail_projects_locations_catalogs_user_events_import_task,
    retail_projects_locations_catalogs_user_events_purge_builder, retail_projects_locations_catalogs_user_events_purge_task,
    retail_projects_locations_catalogs_user_events_rejoin_builder, retail_projects_locations_catalogs_user_events_rejoin_task,
    retail_projects_locations_catalogs_user_events_write_builder, retail_projects_locations_catalogs_user_events_write_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::retail::GoogleApiHttpBody;
use crate::providers::gcp::clients::retail::GoogleCloudRetailV2AttributesConfig;
use crate::providers::gcp::clients::retail::GoogleCloudRetailV2BatchUpdateGenerativeQuestionConfigsResponse;
use crate::providers::gcp::clients::retail::GoogleCloudRetailV2Catalog;
use crate::providers::gcp::clients::retail::GoogleCloudRetailV2CompletionConfig;
use crate::providers::gcp::clients::retail::GoogleCloudRetailV2Control;
use crate::providers::gcp::clients::retail::GoogleCloudRetailV2ConversationalSearchCustomizationConfig;
use crate::providers::gcp::clients::retail::GoogleCloudRetailV2ConversationalSearchResponse;
use crate::providers::gcp::clients::retail::GoogleCloudRetailV2GenerativeQuestionConfig;
use crate::providers::gcp::clients::retail::GoogleCloudRetailV2GenerativeQuestionsFeatureConfig;
use crate::providers::gcp::clients::retail::GoogleCloudRetailV2Model;
use crate::providers::gcp::clients::retail::GoogleCloudRetailV2PredictResponse;
use crate::providers::gcp::clients::retail::GoogleCloudRetailV2Product;
use crate::providers::gcp::clients::retail::GoogleCloudRetailV2SearchResponse;
use crate::providers::gcp::clients::retail::GoogleCloudRetailV2ServingConfig;
use crate::providers::gcp::clients::retail::GoogleCloudRetailV2UserEvent;
use crate::providers::gcp::clients::retail::GoogleLongrunningOperation;
use crate::providers::gcp::clients::retail::GoogleProtobufEmpty;
use crate::providers::gcp::clients::retail::RetailProjectsLocationsCatalogsAttributesConfigAddCatalogAttributeArgs;
use crate::providers::gcp::clients::retail::RetailProjectsLocationsCatalogsAttributesConfigRemoveCatalogAttributeArgs;
use crate::providers::gcp::clients::retail::RetailProjectsLocationsCatalogsAttributesConfigReplaceCatalogAttributeArgs;
use crate::providers::gcp::clients::retail::RetailProjectsLocationsCatalogsBranchesProductsAddFulfillmentPlacesArgs;
use crate::providers::gcp::clients::retail::RetailProjectsLocationsCatalogsBranchesProductsAddLocalInventoriesArgs;
use crate::providers::gcp::clients::retail::RetailProjectsLocationsCatalogsBranchesProductsCreateArgs;
use crate::providers::gcp::clients::retail::RetailProjectsLocationsCatalogsBranchesProductsDeleteArgs;
use crate::providers::gcp::clients::retail::RetailProjectsLocationsCatalogsBranchesProductsImportArgs;
use crate::providers::gcp::clients::retail::RetailProjectsLocationsCatalogsBranchesProductsPatchArgs;
use crate::providers::gcp::clients::retail::RetailProjectsLocationsCatalogsBranchesProductsPurgeArgs;
use crate::providers::gcp::clients::retail::RetailProjectsLocationsCatalogsBranchesProductsRemoveFulfillmentPlacesArgs;
use crate::providers::gcp::clients::retail::RetailProjectsLocationsCatalogsBranchesProductsRemoveLocalInventoriesArgs;
use crate::providers::gcp::clients::retail::RetailProjectsLocationsCatalogsBranchesProductsSetInventoryArgs;
use crate::providers::gcp::clients::retail::RetailProjectsLocationsCatalogsCompletionDataImportArgs;
use crate::providers::gcp::clients::retail::RetailProjectsLocationsCatalogsControlsCreateArgs;
use crate::providers::gcp::clients::retail::RetailProjectsLocationsCatalogsControlsDeleteArgs;
use crate::providers::gcp::clients::retail::RetailProjectsLocationsCatalogsControlsPatchArgs;
use crate::providers::gcp::clients::retail::RetailProjectsLocationsCatalogsExportAnalyticsMetricsArgs;
use crate::providers::gcp::clients::retail::RetailProjectsLocationsCatalogsGenerativeQuestionBatchUpdateArgs;
use crate::providers::gcp::clients::retail::RetailProjectsLocationsCatalogsModelsCreateArgs;
use crate::providers::gcp::clients::retail::RetailProjectsLocationsCatalogsModelsDeleteArgs;
use crate::providers::gcp::clients::retail::RetailProjectsLocationsCatalogsModelsPatchArgs;
use crate::providers::gcp::clients::retail::RetailProjectsLocationsCatalogsModelsPauseArgs;
use crate::providers::gcp::clients::retail::RetailProjectsLocationsCatalogsModelsResumeArgs;
use crate::providers::gcp::clients::retail::RetailProjectsLocationsCatalogsModelsTuneArgs;
use crate::providers::gcp::clients::retail::RetailProjectsLocationsCatalogsPatchArgs;
use crate::providers::gcp::clients::retail::RetailProjectsLocationsCatalogsPlacementsConversationalSearchArgs;
use crate::providers::gcp::clients::retail::RetailProjectsLocationsCatalogsPlacementsPredictArgs;
use crate::providers::gcp::clients::retail::RetailProjectsLocationsCatalogsPlacementsSearchArgs;
use crate::providers::gcp::clients::retail::RetailProjectsLocationsCatalogsServingConfigsAddControlArgs;
use crate::providers::gcp::clients::retail::RetailProjectsLocationsCatalogsServingConfigsConversationalSearchArgs;
use crate::providers::gcp::clients::retail::RetailProjectsLocationsCatalogsServingConfigsCreateArgs;
use crate::providers::gcp::clients::retail::RetailProjectsLocationsCatalogsServingConfigsDeleteArgs;
use crate::providers::gcp::clients::retail::RetailProjectsLocationsCatalogsServingConfigsPatchArgs;
use crate::providers::gcp::clients::retail::RetailProjectsLocationsCatalogsServingConfigsPredictArgs;
use crate::providers::gcp::clients::retail::RetailProjectsLocationsCatalogsServingConfigsRemoveControlArgs;
use crate::providers::gcp::clients::retail::RetailProjectsLocationsCatalogsServingConfigsSearchArgs;
use crate::providers::gcp::clients::retail::RetailProjectsLocationsCatalogsSetDefaultBranchArgs;
use crate::providers::gcp::clients::retail::RetailProjectsLocationsCatalogsUpdateAttributesConfigArgs;
use crate::providers::gcp::clients::retail::RetailProjectsLocationsCatalogsUpdateCompletionConfigArgs;
use crate::providers::gcp::clients::retail::RetailProjectsLocationsCatalogsUpdateConversationalSearchCustomizationConfigArgs;
use crate::providers::gcp::clients::retail::RetailProjectsLocationsCatalogsUpdateGenerativeQuestionArgs;
use crate::providers::gcp::clients::retail::RetailProjectsLocationsCatalogsUpdateGenerativeQuestionFeatureArgs;
use crate::providers::gcp::clients::retail::RetailProjectsLocationsCatalogsUserEventsCollectArgs;
use crate::providers::gcp::clients::retail::RetailProjectsLocationsCatalogsUserEventsImportArgs;
use crate::providers::gcp::clients::retail::RetailProjectsLocationsCatalogsUserEventsPurgeArgs;
use crate::providers::gcp::clients::retail::RetailProjectsLocationsCatalogsUserEventsRejoinArgs;
use crate::providers::gcp::clients::retail::RetailProjectsLocationsCatalogsUserEventsWriteArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// RetailProvider with automatic state tracking.
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
/// let provider = RetailProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct RetailProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> RetailProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new RetailProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Retail projects locations catalogs export analytics metrics.
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
    pub fn retail_projects_locations_catalogs_export_analytics_metrics(
        &self,
        args: &RetailProjectsLocationsCatalogsExportAnalyticsMetricsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = retail_projects_locations_catalogs_export_analytics_metrics_builder(
            &self.http_client,
            &args.catalog,
        )
        .map_err(ProviderError::Api)?;

        let task = retail_projects_locations_catalogs_export_analytics_metrics_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Retail projects locations catalogs patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRetailV2Catalog result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn retail_projects_locations_catalogs_patch(
        &self,
        args: &RetailProjectsLocationsCatalogsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRetailV2Catalog, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = retail_projects_locations_catalogs_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = retail_projects_locations_catalogs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Retail projects locations catalogs set default branch.
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
    pub fn retail_projects_locations_catalogs_set_default_branch(
        &self,
        args: &RetailProjectsLocationsCatalogsSetDefaultBranchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = retail_projects_locations_catalogs_set_default_branch_builder(
            &self.http_client,
            &args.catalog,
        )
        .map_err(ProviderError::Api)?;

        let task = retail_projects_locations_catalogs_set_default_branch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Retail projects locations catalogs update attributes config.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRetailV2AttributesConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn retail_projects_locations_catalogs_update_attributes_config(
        &self,
        args: &RetailProjectsLocationsCatalogsUpdateAttributesConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRetailV2AttributesConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = retail_projects_locations_catalogs_update_attributes_config_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = retail_projects_locations_catalogs_update_attributes_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Retail projects locations catalogs update completion config.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRetailV2CompletionConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn retail_projects_locations_catalogs_update_completion_config(
        &self,
        args: &RetailProjectsLocationsCatalogsUpdateCompletionConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRetailV2CompletionConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = retail_projects_locations_catalogs_update_completion_config_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = retail_projects_locations_catalogs_update_completion_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Retail projects locations catalogs update conversational search customization config.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRetailV2ConversationalSearchCustomizationConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn retail_projects_locations_catalogs_update_conversational_search_customization_config(
        &self,
        args: &RetailProjectsLocationsCatalogsUpdateConversationalSearchCustomizationConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRetailV2ConversationalSearchCustomizationConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = retail_projects_locations_catalogs_update_conversational_search_customization_config_builder(
            &self.http_client,
            &args.catalog,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = retail_projects_locations_catalogs_update_conversational_search_customization_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Retail projects locations catalogs update generative question.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRetailV2GenerativeQuestionConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn retail_projects_locations_catalogs_update_generative_question(
        &self,
        args: &RetailProjectsLocationsCatalogsUpdateGenerativeQuestionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRetailV2GenerativeQuestionConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = retail_projects_locations_catalogs_update_generative_question_builder(
            &self.http_client,
            &args.catalog,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = retail_projects_locations_catalogs_update_generative_question_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Retail projects locations catalogs update generative question feature.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRetailV2GenerativeQuestionsFeatureConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn retail_projects_locations_catalogs_update_generative_question_feature(
        &self,
        args: &RetailProjectsLocationsCatalogsUpdateGenerativeQuestionFeatureArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRetailV2GenerativeQuestionsFeatureConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = retail_projects_locations_catalogs_update_generative_question_feature_builder(
            &self.http_client,
            &args.catalog,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = retail_projects_locations_catalogs_update_generative_question_feature_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Retail projects locations catalogs attributes config add catalog attribute.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRetailV2AttributesConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn retail_projects_locations_catalogs_attributes_config_add_catalog_attribute(
        &self,
        args: &RetailProjectsLocationsCatalogsAttributesConfigAddCatalogAttributeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRetailV2AttributesConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = retail_projects_locations_catalogs_attributes_config_add_catalog_attribute_builder(
            &self.http_client,
            &args.attributesConfig,
        )
        .map_err(ProviderError::Api)?;

        let task = retail_projects_locations_catalogs_attributes_config_add_catalog_attribute_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Retail projects locations catalogs attributes config remove catalog attribute.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRetailV2AttributesConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn retail_projects_locations_catalogs_attributes_config_remove_catalog_attribute(
        &self,
        args: &RetailProjectsLocationsCatalogsAttributesConfigRemoveCatalogAttributeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRetailV2AttributesConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = retail_projects_locations_catalogs_attributes_config_remove_catalog_attribute_builder(
            &self.http_client,
            &args.attributesConfig,
        )
        .map_err(ProviderError::Api)?;

        let task = retail_projects_locations_catalogs_attributes_config_remove_catalog_attribute_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Retail projects locations catalogs attributes config replace catalog attribute.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRetailV2AttributesConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn retail_projects_locations_catalogs_attributes_config_replace_catalog_attribute(
        &self,
        args: &RetailProjectsLocationsCatalogsAttributesConfigReplaceCatalogAttributeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRetailV2AttributesConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = retail_projects_locations_catalogs_attributes_config_replace_catalog_attribute_builder(
            &self.http_client,
            &args.attributesConfig,
        )
        .map_err(ProviderError::Api)?;

        let task = retail_projects_locations_catalogs_attributes_config_replace_catalog_attribute_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Retail projects locations catalogs branches products add fulfillment places.
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
    pub fn retail_projects_locations_catalogs_branches_products_add_fulfillment_places(
        &self,
        args: &RetailProjectsLocationsCatalogsBranchesProductsAddFulfillmentPlacesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = retail_projects_locations_catalogs_branches_products_add_fulfillment_places_builder(
            &self.http_client,
            &args.product,
        )
        .map_err(ProviderError::Api)?;

        let task = retail_projects_locations_catalogs_branches_products_add_fulfillment_places_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Retail projects locations catalogs branches products add local inventories.
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
    pub fn retail_projects_locations_catalogs_branches_products_add_local_inventories(
        &self,
        args: &RetailProjectsLocationsCatalogsBranchesProductsAddLocalInventoriesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = retail_projects_locations_catalogs_branches_products_add_local_inventories_builder(
            &self.http_client,
            &args.product,
        )
        .map_err(ProviderError::Api)?;

        let task = retail_projects_locations_catalogs_branches_products_add_local_inventories_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Retail projects locations catalogs branches products create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRetailV2Product result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn retail_projects_locations_catalogs_branches_products_create(
        &self,
        args: &RetailProjectsLocationsCatalogsBranchesProductsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRetailV2Product, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = retail_projects_locations_catalogs_branches_products_create_builder(
            &self.http_client,
            &args.parent,
            &args.productId,
        )
        .map_err(ProviderError::Api)?;

        let task = retail_projects_locations_catalogs_branches_products_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Retail projects locations catalogs branches products delete.
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
    pub fn retail_projects_locations_catalogs_branches_products_delete(
        &self,
        args: &RetailProjectsLocationsCatalogsBranchesProductsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = retail_projects_locations_catalogs_branches_products_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = retail_projects_locations_catalogs_branches_products_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Retail projects locations catalogs branches products import.
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
    pub fn retail_projects_locations_catalogs_branches_products_import(
        &self,
        args: &RetailProjectsLocationsCatalogsBranchesProductsImportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = retail_projects_locations_catalogs_branches_products_import_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = retail_projects_locations_catalogs_branches_products_import_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Retail projects locations catalogs branches products patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRetailV2Product result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn retail_projects_locations_catalogs_branches_products_patch(
        &self,
        args: &RetailProjectsLocationsCatalogsBranchesProductsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRetailV2Product, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = retail_projects_locations_catalogs_branches_products_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = retail_projects_locations_catalogs_branches_products_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Retail projects locations catalogs branches products purge.
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
    pub fn retail_projects_locations_catalogs_branches_products_purge(
        &self,
        args: &RetailProjectsLocationsCatalogsBranchesProductsPurgeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = retail_projects_locations_catalogs_branches_products_purge_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = retail_projects_locations_catalogs_branches_products_purge_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Retail projects locations catalogs branches products remove fulfillment places.
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
    pub fn retail_projects_locations_catalogs_branches_products_remove_fulfillment_places(
        &self,
        args: &RetailProjectsLocationsCatalogsBranchesProductsRemoveFulfillmentPlacesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = retail_projects_locations_catalogs_branches_products_remove_fulfillment_places_builder(
            &self.http_client,
            &args.product,
        )
        .map_err(ProviderError::Api)?;

        let task = retail_projects_locations_catalogs_branches_products_remove_fulfillment_places_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Retail projects locations catalogs branches products remove local inventories.
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
    pub fn retail_projects_locations_catalogs_branches_products_remove_local_inventories(
        &self,
        args: &RetailProjectsLocationsCatalogsBranchesProductsRemoveLocalInventoriesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = retail_projects_locations_catalogs_branches_products_remove_local_inventories_builder(
            &self.http_client,
            &args.product,
        )
        .map_err(ProviderError::Api)?;

        let task = retail_projects_locations_catalogs_branches_products_remove_local_inventories_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Retail projects locations catalogs branches products set inventory.
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
    pub fn retail_projects_locations_catalogs_branches_products_set_inventory(
        &self,
        args: &RetailProjectsLocationsCatalogsBranchesProductsSetInventoryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = retail_projects_locations_catalogs_branches_products_set_inventory_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = retail_projects_locations_catalogs_branches_products_set_inventory_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Retail projects locations catalogs completion data import.
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
    pub fn retail_projects_locations_catalogs_completion_data_import(
        &self,
        args: &RetailProjectsLocationsCatalogsCompletionDataImportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = retail_projects_locations_catalogs_completion_data_import_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = retail_projects_locations_catalogs_completion_data_import_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Retail projects locations catalogs controls create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRetailV2Control result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn retail_projects_locations_catalogs_controls_create(
        &self,
        args: &RetailProjectsLocationsCatalogsControlsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRetailV2Control, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = retail_projects_locations_catalogs_controls_create_builder(
            &self.http_client,
            &args.parent,
            &args.controlId,
        )
        .map_err(ProviderError::Api)?;

        let task = retail_projects_locations_catalogs_controls_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Retail projects locations catalogs controls delete.
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
    pub fn retail_projects_locations_catalogs_controls_delete(
        &self,
        args: &RetailProjectsLocationsCatalogsControlsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = retail_projects_locations_catalogs_controls_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = retail_projects_locations_catalogs_controls_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Retail projects locations catalogs controls patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRetailV2Control result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn retail_projects_locations_catalogs_controls_patch(
        &self,
        args: &RetailProjectsLocationsCatalogsControlsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRetailV2Control, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = retail_projects_locations_catalogs_controls_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = retail_projects_locations_catalogs_controls_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Retail projects locations catalogs generative question batch update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRetailV2BatchUpdateGenerativeQuestionConfigsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn retail_projects_locations_catalogs_generative_question_batch_update(
        &self,
        args: &RetailProjectsLocationsCatalogsGenerativeQuestionBatchUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRetailV2BatchUpdateGenerativeQuestionConfigsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = retail_projects_locations_catalogs_generative_question_batch_update_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = retail_projects_locations_catalogs_generative_question_batch_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Retail projects locations catalogs models create.
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
    pub fn retail_projects_locations_catalogs_models_create(
        &self,
        args: &RetailProjectsLocationsCatalogsModelsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = retail_projects_locations_catalogs_models_create_builder(
            &self.http_client,
            &args.parent,
            &args.dryRun,
        )
        .map_err(ProviderError::Api)?;

        let task = retail_projects_locations_catalogs_models_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Retail projects locations catalogs models delete.
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
    pub fn retail_projects_locations_catalogs_models_delete(
        &self,
        args: &RetailProjectsLocationsCatalogsModelsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = retail_projects_locations_catalogs_models_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = retail_projects_locations_catalogs_models_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Retail projects locations catalogs models patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRetailV2Model result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn retail_projects_locations_catalogs_models_patch(
        &self,
        args: &RetailProjectsLocationsCatalogsModelsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRetailV2Model, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = retail_projects_locations_catalogs_models_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = retail_projects_locations_catalogs_models_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Retail projects locations catalogs models pause.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRetailV2Model result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn retail_projects_locations_catalogs_models_pause(
        &self,
        args: &RetailProjectsLocationsCatalogsModelsPauseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRetailV2Model, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = retail_projects_locations_catalogs_models_pause_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = retail_projects_locations_catalogs_models_pause_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Retail projects locations catalogs models resume.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRetailV2Model result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn retail_projects_locations_catalogs_models_resume(
        &self,
        args: &RetailProjectsLocationsCatalogsModelsResumeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRetailV2Model, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = retail_projects_locations_catalogs_models_resume_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = retail_projects_locations_catalogs_models_resume_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Retail projects locations catalogs models tune.
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
    pub fn retail_projects_locations_catalogs_models_tune(
        &self,
        args: &RetailProjectsLocationsCatalogsModelsTuneArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = retail_projects_locations_catalogs_models_tune_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = retail_projects_locations_catalogs_models_tune_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Retail projects locations catalogs placements conversational search.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRetailV2ConversationalSearchResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn retail_projects_locations_catalogs_placements_conversational_search(
        &self,
        args: &RetailProjectsLocationsCatalogsPlacementsConversationalSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRetailV2ConversationalSearchResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = retail_projects_locations_catalogs_placements_conversational_search_builder(
            &self.http_client,
            &args.placement,
        )
        .map_err(ProviderError::Api)?;

        let task = retail_projects_locations_catalogs_placements_conversational_search_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Retail projects locations catalogs placements predict.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRetailV2PredictResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn retail_projects_locations_catalogs_placements_predict(
        &self,
        args: &RetailProjectsLocationsCatalogsPlacementsPredictArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRetailV2PredictResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = retail_projects_locations_catalogs_placements_predict_builder(
            &self.http_client,
            &args.placement,
        )
        .map_err(ProviderError::Api)?;

        let task = retail_projects_locations_catalogs_placements_predict_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Retail projects locations catalogs placements search.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRetailV2SearchResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn retail_projects_locations_catalogs_placements_search(
        &self,
        args: &RetailProjectsLocationsCatalogsPlacementsSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRetailV2SearchResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = retail_projects_locations_catalogs_placements_search_builder(
            &self.http_client,
            &args.placement,
        )
        .map_err(ProviderError::Api)?;

        let task = retail_projects_locations_catalogs_placements_search_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Retail projects locations catalogs serving configs add control.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRetailV2ServingConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn retail_projects_locations_catalogs_serving_configs_add_control(
        &self,
        args: &RetailProjectsLocationsCatalogsServingConfigsAddControlArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRetailV2ServingConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = retail_projects_locations_catalogs_serving_configs_add_control_builder(
            &self.http_client,
            &args.servingConfig,
        )
        .map_err(ProviderError::Api)?;

        let task = retail_projects_locations_catalogs_serving_configs_add_control_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Retail projects locations catalogs serving configs conversational search.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRetailV2ConversationalSearchResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn retail_projects_locations_catalogs_serving_configs_conversational_search(
        &self,
        args: &RetailProjectsLocationsCatalogsServingConfigsConversationalSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRetailV2ConversationalSearchResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = retail_projects_locations_catalogs_serving_configs_conversational_search_builder(
            &self.http_client,
            &args.placement,
        )
        .map_err(ProviderError::Api)?;

        let task = retail_projects_locations_catalogs_serving_configs_conversational_search_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Retail projects locations catalogs serving configs create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRetailV2ServingConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn retail_projects_locations_catalogs_serving_configs_create(
        &self,
        args: &RetailProjectsLocationsCatalogsServingConfigsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRetailV2ServingConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = retail_projects_locations_catalogs_serving_configs_create_builder(
            &self.http_client,
            &args.parent,
            &args.servingConfigId,
        )
        .map_err(ProviderError::Api)?;

        let task = retail_projects_locations_catalogs_serving_configs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Retail projects locations catalogs serving configs delete.
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
    pub fn retail_projects_locations_catalogs_serving_configs_delete(
        &self,
        args: &RetailProjectsLocationsCatalogsServingConfigsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = retail_projects_locations_catalogs_serving_configs_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = retail_projects_locations_catalogs_serving_configs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Retail projects locations catalogs serving configs patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRetailV2ServingConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn retail_projects_locations_catalogs_serving_configs_patch(
        &self,
        args: &RetailProjectsLocationsCatalogsServingConfigsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRetailV2ServingConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = retail_projects_locations_catalogs_serving_configs_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = retail_projects_locations_catalogs_serving_configs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Retail projects locations catalogs serving configs predict.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRetailV2PredictResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn retail_projects_locations_catalogs_serving_configs_predict(
        &self,
        args: &RetailProjectsLocationsCatalogsServingConfigsPredictArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRetailV2PredictResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = retail_projects_locations_catalogs_serving_configs_predict_builder(
            &self.http_client,
            &args.placement,
        )
        .map_err(ProviderError::Api)?;

        let task = retail_projects_locations_catalogs_serving_configs_predict_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Retail projects locations catalogs serving configs remove control.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRetailV2ServingConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn retail_projects_locations_catalogs_serving_configs_remove_control(
        &self,
        args: &RetailProjectsLocationsCatalogsServingConfigsRemoveControlArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRetailV2ServingConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = retail_projects_locations_catalogs_serving_configs_remove_control_builder(
            &self.http_client,
            &args.servingConfig,
        )
        .map_err(ProviderError::Api)?;

        let task = retail_projects_locations_catalogs_serving_configs_remove_control_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Retail projects locations catalogs serving configs search.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRetailV2SearchResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn retail_projects_locations_catalogs_serving_configs_search(
        &self,
        args: &RetailProjectsLocationsCatalogsServingConfigsSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRetailV2SearchResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = retail_projects_locations_catalogs_serving_configs_search_builder(
            &self.http_client,
            &args.placement,
        )
        .map_err(ProviderError::Api)?;

        let task = retail_projects_locations_catalogs_serving_configs_search_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Retail projects locations catalogs user events collect.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleApiHttpBody result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn retail_projects_locations_catalogs_user_events_collect(
        &self,
        args: &RetailProjectsLocationsCatalogsUserEventsCollectArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleApiHttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = retail_projects_locations_catalogs_user_events_collect_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = retail_projects_locations_catalogs_user_events_collect_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Retail projects locations catalogs user events import.
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
    pub fn retail_projects_locations_catalogs_user_events_import(
        &self,
        args: &RetailProjectsLocationsCatalogsUserEventsImportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = retail_projects_locations_catalogs_user_events_import_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = retail_projects_locations_catalogs_user_events_import_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Retail projects locations catalogs user events purge.
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
    pub fn retail_projects_locations_catalogs_user_events_purge(
        &self,
        args: &RetailProjectsLocationsCatalogsUserEventsPurgeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = retail_projects_locations_catalogs_user_events_purge_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = retail_projects_locations_catalogs_user_events_purge_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Retail projects locations catalogs user events rejoin.
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
    pub fn retail_projects_locations_catalogs_user_events_rejoin(
        &self,
        args: &RetailProjectsLocationsCatalogsUserEventsRejoinArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = retail_projects_locations_catalogs_user_events_rejoin_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = retail_projects_locations_catalogs_user_events_rejoin_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Retail projects locations catalogs user events write.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRetailV2UserEvent result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn retail_projects_locations_catalogs_user_events_write(
        &self,
        args: &RetailProjectsLocationsCatalogsUserEventsWriteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRetailV2UserEvent, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = retail_projects_locations_catalogs_user_events_write_builder(
            &self.http_client,
            &args.parent,
            &args.writeAsync,
        )
        .map_err(ProviderError::Api)?;

        let task = retail_projects_locations_catalogs_user_events_write_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
