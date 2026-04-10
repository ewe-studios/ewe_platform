//! RecommenderProvider - State-aware recommender API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       recommender API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::recommender::{
    recommender_billing_accounts_locations_insight_types_update_config_builder, recommender_billing_accounts_locations_insight_types_update_config_task,
    recommender_billing_accounts_locations_insight_types_insights_mark_accepted_builder, recommender_billing_accounts_locations_insight_types_insights_mark_accepted_task,
    recommender_billing_accounts_locations_recommenders_update_config_builder, recommender_billing_accounts_locations_recommenders_update_config_task,
    recommender_billing_accounts_locations_recommenders_recommendations_mark_claimed_builder, recommender_billing_accounts_locations_recommenders_recommendations_mark_claimed_task,
    recommender_billing_accounts_locations_recommenders_recommendations_mark_dismissed_builder, recommender_billing_accounts_locations_recommenders_recommendations_mark_dismissed_task,
    recommender_billing_accounts_locations_recommenders_recommendations_mark_failed_builder, recommender_billing_accounts_locations_recommenders_recommendations_mark_failed_task,
    recommender_billing_accounts_locations_recommenders_recommendations_mark_succeeded_builder, recommender_billing_accounts_locations_recommenders_recommendations_mark_succeeded_task,
    recommender_folders_locations_insight_types_insights_mark_accepted_builder, recommender_folders_locations_insight_types_insights_mark_accepted_task,
    recommender_folders_locations_recommenders_recommendations_mark_claimed_builder, recommender_folders_locations_recommenders_recommendations_mark_claimed_task,
    recommender_folders_locations_recommenders_recommendations_mark_dismissed_builder, recommender_folders_locations_recommenders_recommendations_mark_dismissed_task,
    recommender_folders_locations_recommenders_recommendations_mark_failed_builder, recommender_folders_locations_recommenders_recommendations_mark_failed_task,
    recommender_folders_locations_recommenders_recommendations_mark_succeeded_builder, recommender_folders_locations_recommenders_recommendations_mark_succeeded_task,
    recommender_organizations_locations_insight_types_update_config_builder, recommender_organizations_locations_insight_types_update_config_task,
    recommender_organizations_locations_insight_types_insights_mark_accepted_builder, recommender_organizations_locations_insight_types_insights_mark_accepted_task,
    recommender_organizations_locations_recommenders_update_config_builder, recommender_organizations_locations_recommenders_update_config_task,
    recommender_organizations_locations_recommenders_recommendations_mark_claimed_builder, recommender_organizations_locations_recommenders_recommendations_mark_claimed_task,
    recommender_organizations_locations_recommenders_recommendations_mark_dismissed_builder, recommender_organizations_locations_recommenders_recommendations_mark_dismissed_task,
    recommender_organizations_locations_recommenders_recommendations_mark_failed_builder, recommender_organizations_locations_recommenders_recommendations_mark_failed_task,
    recommender_organizations_locations_recommenders_recommendations_mark_succeeded_builder, recommender_organizations_locations_recommenders_recommendations_mark_succeeded_task,
    recommender_projects_locations_insight_types_update_config_builder, recommender_projects_locations_insight_types_update_config_task,
    recommender_projects_locations_insight_types_insights_mark_accepted_builder, recommender_projects_locations_insight_types_insights_mark_accepted_task,
    recommender_projects_locations_recommenders_update_config_builder, recommender_projects_locations_recommenders_update_config_task,
    recommender_projects_locations_recommenders_recommendations_mark_claimed_builder, recommender_projects_locations_recommenders_recommendations_mark_claimed_task,
    recommender_projects_locations_recommenders_recommendations_mark_dismissed_builder, recommender_projects_locations_recommenders_recommendations_mark_dismissed_task,
    recommender_projects_locations_recommenders_recommendations_mark_failed_builder, recommender_projects_locations_recommenders_recommendations_mark_failed_task,
    recommender_projects_locations_recommenders_recommendations_mark_succeeded_builder, recommender_projects_locations_recommenders_recommendations_mark_succeeded_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::recommender::GoogleCloudRecommenderV1Insight;
use crate::providers::gcp::clients::recommender::GoogleCloudRecommenderV1InsightTypeConfig;
use crate::providers::gcp::clients::recommender::GoogleCloudRecommenderV1Recommendation;
use crate::providers::gcp::clients::recommender::GoogleCloudRecommenderV1RecommenderConfig;
use crate::providers::gcp::clients::recommender::RecommenderBillingAccountsLocationsInsightTypesInsightsMarkAcceptedArgs;
use crate::providers::gcp::clients::recommender::RecommenderBillingAccountsLocationsInsightTypesUpdateConfigArgs;
use crate::providers::gcp::clients::recommender::RecommenderBillingAccountsLocationsRecommendersRecommendationsMarkClaimedArgs;
use crate::providers::gcp::clients::recommender::RecommenderBillingAccountsLocationsRecommendersRecommendationsMarkDismissedArgs;
use crate::providers::gcp::clients::recommender::RecommenderBillingAccountsLocationsRecommendersRecommendationsMarkFailedArgs;
use crate::providers::gcp::clients::recommender::RecommenderBillingAccountsLocationsRecommendersRecommendationsMarkSucceededArgs;
use crate::providers::gcp::clients::recommender::RecommenderBillingAccountsLocationsRecommendersUpdateConfigArgs;
use crate::providers::gcp::clients::recommender::RecommenderFoldersLocationsInsightTypesInsightsMarkAcceptedArgs;
use crate::providers::gcp::clients::recommender::RecommenderFoldersLocationsRecommendersRecommendationsMarkClaimedArgs;
use crate::providers::gcp::clients::recommender::RecommenderFoldersLocationsRecommendersRecommendationsMarkDismissedArgs;
use crate::providers::gcp::clients::recommender::RecommenderFoldersLocationsRecommendersRecommendationsMarkFailedArgs;
use crate::providers::gcp::clients::recommender::RecommenderFoldersLocationsRecommendersRecommendationsMarkSucceededArgs;
use crate::providers::gcp::clients::recommender::RecommenderOrganizationsLocationsInsightTypesInsightsMarkAcceptedArgs;
use crate::providers::gcp::clients::recommender::RecommenderOrganizationsLocationsInsightTypesUpdateConfigArgs;
use crate::providers::gcp::clients::recommender::RecommenderOrganizationsLocationsRecommendersRecommendationsMarkClaimedArgs;
use crate::providers::gcp::clients::recommender::RecommenderOrganizationsLocationsRecommendersRecommendationsMarkDismissedArgs;
use crate::providers::gcp::clients::recommender::RecommenderOrganizationsLocationsRecommendersRecommendationsMarkFailedArgs;
use crate::providers::gcp::clients::recommender::RecommenderOrganizationsLocationsRecommendersRecommendationsMarkSucceededArgs;
use crate::providers::gcp::clients::recommender::RecommenderOrganizationsLocationsRecommendersUpdateConfigArgs;
use crate::providers::gcp::clients::recommender::RecommenderProjectsLocationsInsightTypesInsightsMarkAcceptedArgs;
use crate::providers::gcp::clients::recommender::RecommenderProjectsLocationsInsightTypesUpdateConfigArgs;
use crate::providers::gcp::clients::recommender::RecommenderProjectsLocationsRecommendersRecommendationsMarkClaimedArgs;
use crate::providers::gcp::clients::recommender::RecommenderProjectsLocationsRecommendersRecommendationsMarkDismissedArgs;
use crate::providers::gcp::clients::recommender::RecommenderProjectsLocationsRecommendersRecommendationsMarkFailedArgs;
use crate::providers::gcp::clients::recommender::RecommenderProjectsLocationsRecommendersRecommendationsMarkSucceededArgs;
use crate::providers::gcp::clients::recommender::RecommenderProjectsLocationsRecommendersUpdateConfigArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// RecommenderProvider with automatic state tracking.
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
/// let provider = RecommenderProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct RecommenderProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> RecommenderProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new RecommenderProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Recommender billing accounts locations insight types update config.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecommenderV1InsightTypeConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn recommender_billing_accounts_locations_insight_types_update_config(
        &self,
        args: &RecommenderBillingAccountsLocationsInsightTypesUpdateConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommenderV1InsightTypeConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommender_billing_accounts_locations_insight_types_update_config_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = recommender_billing_accounts_locations_insight_types_update_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recommender billing accounts locations insight types insights mark accepted.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecommenderV1Insight result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn recommender_billing_accounts_locations_insight_types_insights_mark_accepted(
        &self,
        args: &RecommenderBillingAccountsLocationsInsightTypesInsightsMarkAcceptedArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommenderV1Insight, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommender_billing_accounts_locations_insight_types_insights_mark_accepted_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = recommender_billing_accounts_locations_insight_types_insights_mark_accepted_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recommender billing accounts locations recommenders update config.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecommenderV1RecommenderConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn recommender_billing_accounts_locations_recommenders_update_config(
        &self,
        args: &RecommenderBillingAccountsLocationsRecommendersUpdateConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommenderV1RecommenderConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommender_billing_accounts_locations_recommenders_update_config_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = recommender_billing_accounts_locations_recommenders_update_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recommender billing accounts locations recommenders recommendations mark claimed.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecommenderV1Recommendation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn recommender_billing_accounts_locations_recommenders_recommendations_mark_claimed(
        &self,
        args: &RecommenderBillingAccountsLocationsRecommendersRecommendationsMarkClaimedArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommenderV1Recommendation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommender_billing_accounts_locations_recommenders_recommendations_mark_claimed_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = recommender_billing_accounts_locations_recommenders_recommendations_mark_claimed_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recommender billing accounts locations recommenders recommendations mark dismissed.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecommenderV1Recommendation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn recommender_billing_accounts_locations_recommenders_recommendations_mark_dismissed(
        &self,
        args: &RecommenderBillingAccountsLocationsRecommendersRecommendationsMarkDismissedArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommenderV1Recommendation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommender_billing_accounts_locations_recommenders_recommendations_mark_dismissed_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = recommender_billing_accounts_locations_recommenders_recommendations_mark_dismissed_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recommender billing accounts locations recommenders recommendations mark failed.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecommenderV1Recommendation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn recommender_billing_accounts_locations_recommenders_recommendations_mark_failed(
        &self,
        args: &RecommenderBillingAccountsLocationsRecommendersRecommendationsMarkFailedArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommenderV1Recommendation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommender_billing_accounts_locations_recommenders_recommendations_mark_failed_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = recommender_billing_accounts_locations_recommenders_recommendations_mark_failed_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recommender billing accounts locations recommenders recommendations mark succeeded.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecommenderV1Recommendation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn recommender_billing_accounts_locations_recommenders_recommendations_mark_succeeded(
        &self,
        args: &RecommenderBillingAccountsLocationsRecommendersRecommendationsMarkSucceededArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommenderV1Recommendation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommender_billing_accounts_locations_recommenders_recommendations_mark_succeeded_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = recommender_billing_accounts_locations_recommenders_recommendations_mark_succeeded_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recommender folders locations insight types insights mark accepted.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecommenderV1Insight result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn recommender_folders_locations_insight_types_insights_mark_accepted(
        &self,
        args: &RecommenderFoldersLocationsInsightTypesInsightsMarkAcceptedArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommenderV1Insight, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommender_folders_locations_insight_types_insights_mark_accepted_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = recommender_folders_locations_insight_types_insights_mark_accepted_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recommender folders locations recommenders recommendations mark claimed.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecommenderV1Recommendation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn recommender_folders_locations_recommenders_recommendations_mark_claimed(
        &self,
        args: &RecommenderFoldersLocationsRecommendersRecommendationsMarkClaimedArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommenderV1Recommendation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommender_folders_locations_recommenders_recommendations_mark_claimed_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = recommender_folders_locations_recommenders_recommendations_mark_claimed_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recommender folders locations recommenders recommendations mark dismissed.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecommenderV1Recommendation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn recommender_folders_locations_recommenders_recommendations_mark_dismissed(
        &self,
        args: &RecommenderFoldersLocationsRecommendersRecommendationsMarkDismissedArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommenderV1Recommendation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommender_folders_locations_recommenders_recommendations_mark_dismissed_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = recommender_folders_locations_recommenders_recommendations_mark_dismissed_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recommender folders locations recommenders recommendations mark failed.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecommenderV1Recommendation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn recommender_folders_locations_recommenders_recommendations_mark_failed(
        &self,
        args: &RecommenderFoldersLocationsRecommendersRecommendationsMarkFailedArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommenderV1Recommendation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommender_folders_locations_recommenders_recommendations_mark_failed_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = recommender_folders_locations_recommenders_recommendations_mark_failed_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recommender folders locations recommenders recommendations mark succeeded.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecommenderV1Recommendation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn recommender_folders_locations_recommenders_recommendations_mark_succeeded(
        &self,
        args: &RecommenderFoldersLocationsRecommendersRecommendationsMarkSucceededArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommenderV1Recommendation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommender_folders_locations_recommenders_recommendations_mark_succeeded_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = recommender_folders_locations_recommenders_recommendations_mark_succeeded_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recommender organizations locations insight types update config.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecommenderV1InsightTypeConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn recommender_organizations_locations_insight_types_update_config(
        &self,
        args: &RecommenderOrganizationsLocationsInsightTypesUpdateConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommenderV1InsightTypeConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommender_organizations_locations_insight_types_update_config_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = recommender_organizations_locations_insight_types_update_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recommender organizations locations insight types insights mark accepted.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecommenderV1Insight result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn recommender_organizations_locations_insight_types_insights_mark_accepted(
        &self,
        args: &RecommenderOrganizationsLocationsInsightTypesInsightsMarkAcceptedArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommenderV1Insight, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommender_organizations_locations_insight_types_insights_mark_accepted_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = recommender_organizations_locations_insight_types_insights_mark_accepted_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recommender organizations locations recommenders update config.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecommenderV1RecommenderConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn recommender_organizations_locations_recommenders_update_config(
        &self,
        args: &RecommenderOrganizationsLocationsRecommendersUpdateConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommenderV1RecommenderConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommender_organizations_locations_recommenders_update_config_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = recommender_organizations_locations_recommenders_update_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recommender organizations locations recommenders recommendations mark claimed.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecommenderV1Recommendation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn recommender_organizations_locations_recommenders_recommendations_mark_claimed(
        &self,
        args: &RecommenderOrganizationsLocationsRecommendersRecommendationsMarkClaimedArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommenderV1Recommendation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommender_organizations_locations_recommenders_recommendations_mark_claimed_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = recommender_organizations_locations_recommenders_recommendations_mark_claimed_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recommender organizations locations recommenders recommendations mark dismissed.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecommenderV1Recommendation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn recommender_organizations_locations_recommenders_recommendations_mark_dismissed(
        &self,
        args: &RecommenderOrganizationsLocationsRecommendersRecommendationsMarkDismissedArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommenderV1Recommendation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommender_organizations_locations_recommenders_recommendations_mark_dismissed_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = recommender_organizations_locations_recommenders_recommendations_mark_dismissed_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recommender organizations locations recommenders recommendations mark failed.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecommenderV1Recommendation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn recommender_organizations_locations_recommenders_recommendations_mark_failed(
        &self,
        args: &RecommenderOrganizationsLocationsRecommendersRecommendationsMarkFailedArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommenderV1Recommendation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommender_organizations_locations_recommenders_recommendations_mark_failed_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = recommender_organizations_locations_recommenders_recommendations_mark_failed_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recommender organizations locations recommenders recommendations mark succeeded.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecommenderV1Recommendation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn recommender_organizations_locations_recommenders_recommendations_mark_succeeded(
        &self,
        args: &RecommenderOrganizationsLocationsRecommendersRecommendationsMarkSucceededArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommenderV1Recommendation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommender_organizations_locations_recommenders_recommendations_mark_succeeded_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = recommender_organizations_locations_recommenders_recommendations_mark_succeeded_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recommender projects locations insight types update config.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecommenderV1InsightTypeConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn recommender_projects_locations_insight_types_update_config(
        &self,
        args: &RecommenderProjectsLocationsInsightTypesUpdateConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommenderV1InsightTypeConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommender_projects_locations_insight_types_update_config_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = recommender_projects_locations_insight_types_update_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recommender projects locations insight types insights mark accepted.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecommenderV1Insight result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn recommender_projects_locations_insight_types_insights_mark_accepted(
        &self,
        args: &RecommenderProjectsLocationsInsightTypesInsightsMarkAcceptedArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommenderV1Insight, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommender_projects_locations_insight_types_insights_mark_accepted_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = recommender_projects_locations_insight_types_insights_mark_accepted_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recommender projects locations recommenders update config.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecommenderV1RecommenderConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn recommender_projects_locations_recommenders_update_config(
        &self,
        args: &RecommenderProjectsLocationsRecommendersUpdateConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommenderV1RecommenderConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommender_projects_locations_recommenders_update_config_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = recommender_projects_locations_recommenders_update_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recommender projects locations recommenders recommendations mark claimed.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecommenderV1Recommendation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn recommender_projects_locations_recommenders_recommendations_mark_claimed(
        &self,
        args: &RecommenderProjectsLocationsRecommendersRecommendationsMarkClaimedArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommenderV1Recommendation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommender_projects_locations_recommenders_recommendations_mark_claimed_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = recommender_projects_locations_recommenders_recommendations_mark_claimed_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recommender projects locations recommenders recommendations mark dismissed.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecommenderV1Recommendation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn recommender_projects_locations_recommenders_recommendations_mark_dismissed(
        &self,
        args: &RecommenderProjectsLocationsRecommendersRecommendationsMarkDismissedArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommenderV1Recommendation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommender_projects_locations_recommenders_recommendations_mark_dismissed_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = recommender_projects_locations_recommenders_recommendations_mark_dismissed_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recommender projects locations recommenders recommendations mark failed.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecommenderV1Recommendation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn recommender_projects_locations_recommenders_recommendations_mark_failed(
        &self,
        args: &RecommenderProjectsLocationsRecommendersRecommendationsMarkFailedArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommenderV1Recommendation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommender_projects_locations_recommenders_recommendations_mark_failed_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = recommender_projects_locations_recommenders_recommendations_mark_failed_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recommender projects locations recommenders recommendations mark succeeded.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecommenderV1Recommendation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn recommender_projects_locations_recommenders_recommendations_mark_succeeded(
        &self,
        args: &RecommenderProjectsLocationsRecommendersRecommendationsMarkSucceededArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommenderV1Recommendation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommender_projects_locations_recommenders_recommendations_mark_succeeded_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = recommender_projects_locations_recommenders_recommendations_mark_succeeded_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
