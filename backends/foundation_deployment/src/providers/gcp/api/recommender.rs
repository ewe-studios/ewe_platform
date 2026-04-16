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
    recommender_billing_accounts_locations_insight_types_get_config_builder, recommender_billing_accounts_locations_insight_types_get_config_task,
    recommender_billing_accounts_locations_insight_types_update_config_builder, recommender_billing_accounts_locations_insight_types_update_config_task,
    recommender_billing_accounts_locations_insight_types_insights_get_builder, recommender_billing_accounts_locations_insight_types_insights_get_task,
    recommender_billing_accounts_locations_insight_types_insights_list_builder, recommender_billing_accounts_locations_insight_types_insights_list_task,
    recommender_billing_accounts_locations_insight_types_insights_mark_accepted_builder, recommender_billing_accounts_locations_insight_types_insights_mark_accepted_task,
    recommender_billing_accounts_locations_recommenders_get_config_builder, recommender_billing_accounts_locations_recommenders_get_config_task,
    recommender_billing_accounts_locations_recommenders_update_config_builder, recommender_billing_accounts_locations_recommenders_update_config_task,
    recommender_billing_accounts_locations_recommenders_recommendations_get_builder, recommender_billing_accounts_locations_recommenders_recommendations_get_task,
    recommender_billing_accounts_locations_recommenders_recommendations_list_builder, recommender_billing_accounts_locations_recommenders_recommendations_list_task,
    recommender_billing_accounts_locations_recommenders_recommendations_mark_claimed_builder, recommender_billing_accounts_locations_recommenders_recommendations_mark_claimed_task,
    recommender_billing_accounts_locations_recommenders_recommendations_mark_dismissed_builder, recommender_billing_accounts_locations_recommenders_recommendations_mark_dismissed_task,
    recommender_billing_accounts_locations_recommenders_recommendations_mark_failed_builder, recommender_billing_accounts_locations_recommenders_recommendations_mark_failed_task,
    recommender_billing_accounts_locations_recommenders_recommendations_mark_succeeded_builder, recommender_billing_accounts_locations_recommenders_recommendations_mark_succeeded_task,
    recommender_folders_locations_insight_types_insights_get_builder, recommender_folders_locations_insight_types_insights_get_task,
    recommender_folders_locations_insight_types_insights_list_builder, recommender_folders_locations_insight_types_insights_list_task,
    recommender_folders_locations_insight_types_insights_mark_accepted_builder, recommender_folders_locations_insight_types_insights_mark_accepted_task,
    recommender_folders_locations_recommenders_recommendations_get_builder, recommender_folders_locations_recommenders_recommendations_get_task,
    recommender_folders_locations_recommenders_recommendations_list_builder, recommender_folders_locations_recommenders_recommendations_list_task,
    recommender_folders_locations_recommenders_recommendations_mark_claimed_builder, recommender_folders_locations_recommenders_recommendations_mark_claimed_task,
    recommender_folders_locations_recommenders_recommendations_mark_dismissed_builder, recommender_folders_locations_recommenders_recommendations_mark_dismissed_task,
    recommender_folders_locations_recommenders_recommendations_mark_failed_builder, recommender_folders_locations_recommenders_recommendations_mark_failed_task,
    recommender_folders_locations_recommenders_recommendations_mark_succeeded_builder, recommender_folders_locations_recommenders_recommendations_mark_succeeded_task,
    recommender_organizations_locations_insight_types_get_config_builder, recommender_organizations_locations_insight_types_get_config_task,
    recommender_organizations_locations_insight_types_update_config_builder, recommender_organizations_locations_insight_types_update_config_task,
    recommender_organizations_locations_insight_types_insights_get_builder, recommender_organizations_locations_insight_types_insights_get_task,
    recommender_organizations_locations_insight_types_insights_list_builder, recommender_organizations_locations_insight_types_insights_list_task,
    recommender_organizations_locations_insight_types_insights_mark_accepted_builder, recommender_organizations_locations_insight_types_insights_mark_accepted_task,
    recommender_organizations_locations_recommenders_get_config_builder, recommender_organizations_locations_recommenders_get_config_task,
    recommender_organizations_locations_recommenders_update_config_builder, recommender_organizations_locations_recommenders_update_config_task,
    recommender_organizations_locations_recommenders_recommendations_get_builder, recommender_organizations_locations_recommenders_recommendations_get_task,
    recommender_organizations_locations_recommenders_recommendations_list_builder, recommender_organizations_locations_recommenders_recommendations_list_task,
    recommender_organizations_locations_recommenders_recommendations_mark_claimed_builder, recommender_organizations_locations_recommenders_recommendations_mark_claimed_task,
    recommender_organizations_locations_recommenders_recommendations_mark_dismissed_builder, recommender_organizations_locations_recommenders_recommendations_mark_dismissed_task,
    recommender_organizations_locations_recommenders_recommendations_mark_failed_builder, recommender_organizations_locations_recommenders_recommendations_mark_failed_task,
    recommender_organizations_locations_recommenders_recommendations_mark_succeeded_builder, recommender_organizations_locations_recommenders_recommendations_mark_succeeded_task,
    recommender_projects_locations_insight_types_get_config_builder, recommender_projects_locations_insight_types_get_config_task,
    recommender_projects_locations_insight_types_update_config_builder, recommender_projects_locations_insight_types_update_config_task,
    recommender_projects_locations_insight_types_insights_get_builder, recommender_projects_locations_insight_types_insights_get_task,
    recommender_projects_locations_insight_types_insights_list_builder, recommender_projects_locations_insight_types_insights_list_task,
    recommender_projects_locations_insight_types_insights_mark_accepted_builder, recommender_projects_locations_insight_types_insights_mark_accepted_task,
    recommender_projects_locations_recommenders_get_config_builder, recommender_projects_locations_recommenders_get_config_task,
    recommender_projects_locations_recommenders_update_config_builder, recommender_projects_locations_recommenders_update_config_task,
    recommender_projects_locations_recommenders_recommendations_get_builder, recommender_projects_locations_recommenders_recommendations_get_task,
    recommender_projects_locations_recommenders_recommendations_list_builder, recommender_projects_locations_recommenders_recommendations_list_task,
    recommender_projects_locations_recommenders_recommendations_mark_claimed_builder, recommender_projects_locations_recommenders_recommendations_mark_claimed_task,
    recommender_projects_locations_recommenders_recommendations_mark_dismissed_builder, recommender_projects_locations_recommenders_recommendations_mark_dismissed_task,
    recommender_projects_locations_recommenders_recommendations_mark_failed_builder, recommender_projects_locations_recommenders_recommendations_mark_failed_task,
    recommender_projects_locations_recommenders_recommendations_mark_succeeded_builder, recommender_projects_locations_recommenders_recommendations_mark_succeeded_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::recommender::GoogleCloudRecommenderV1Insight;
use crate::providers::gcp::clients::recommender::GoogleCloudRecommenderV1InsightTypeConfig;
use crate::providers::gcp::clients::recommender::GoogleCloudRecommenderV1ListInsightsResponse;
use crate::providers::gcp::clients::recommender::GoogleCloudRecommenderV1ListRecommendationsResponse;
use crate::providers::gcp::clients::recommender::GoogleCloudRecommenderV1Recommendation;
use crate::providers::gcp::clients::recommender::GoogleCloudRecommenderV1RecommenderConfig;
use crate::providers::gcp::clients::recommender::RecommenderBillingAccountsLocationsInsightTypesGetConfigArgs;
use crate::providers::gcp::clients::recommender::RecommenderBillingAccountsLocationsInsightTypesInsightsGetArgs;
use crate::providers::gcp::clients::recommender::RecommenderBillingAccountsLocationsInsightTypesInsightsListArgs;
use crate::providers::gcp::clients::recommender::RecommenderBillingAccountsLocationsInsightTypesInsightsMarkAcceptedArgs;
use crate::providers::gcp::clients::recommender::RecommenderBillingAccountsLocationsInsightTypesUpdateConfigArgs;
use crate::providers::gcp::clients::recommender::RecommenderBillingAccountsLocationsRecommendersGetConfigArgs;
use crate::providers::gcp::clients::recommender::RecommenderBillingAccountsLocationsRecommendersRecommendationsGetArgs;
use crate::providers::gcp::clients::recommender::RecommenderBillingAccountsLocationsRecommendersRecommendationsListArgs;
use crate::providers::gcp::clients::recommender::RecommenderBillingAccountsLocationsRecommendersRecommendationsMarkClaimedArgs;
use crate::providers::gcp::clients::recommender::RecommenderBillingAccountsLocationsRecommendersRecommendationsMarkDismissedArgs;
use crate::providers::gcp::clients::recommender::RecommenderBillingAccountsLocationsRecommendersRecommendationsMarkFailedArgs;
use crate::providers::gcp::clients::recommender::RecommenderBillingAccountsLocationsRecommendersRecommendationsMarkSucceededArgs;
use crate::providers::gcp::clients::recommender::RecommenderBillingAccountsLocationsRecommendersUpdateConfigArgs;
use crate::providers::gcp::clients::recommender::RecommenderFoldersLocationsInsightTypesInsightsGetArgs;
use crate::providers::gcp::clients::recommender::RecommenderFoldersLocationsInsightTypesInsightsListArgs;
use crate::providers::gcp::clients::recommender::RecommenderFoldersLocationsInsightTypesInsightsMarkAcceptedArgs;
use crate::providers::gcp::clients::recommender::RecommenderFoldersLocationsRecommendersRecommendationsGetArgs;
use crate::providers::gcp::clients::recommender::RecommenderFoldersLocationsRecommendersRecommendationsListArgs;
use crate::providers::gcp::clients::recommender::RecommenderFoldersLocationsRecommendersRecommendationsMarkClaimedArgs;
use crate::providers::gcp::clients::recommender::RecommenderFoldersLocationsRecommendersRecommendationsMarkDismissedArgs;
use crate::providers::gcp::clients::recommender::RecommenderFoldersLocationsRecommendersRecommendationsMarkFailedArgs;
use crate::providers::gcp::clients::recommender::RecommenderFoldersLocationsRecommendersRecommendationsMarkSucceededArgs;
use crate::providers::gcp::clients::recommender::RecommenderOrganizationsLocationsInsightTypesGetConfigArgs;
use crate::providers::gcp::clients::recommender::RecommenderOrganizationsLocationsInsightTypesInsightsGetArgs;
use crate::providers::gcp::clients::recommender::RecommenderOrganizationsLocationsInsightTypesInsightsListArgs;
use crate::providers::gcp::clients::recommender::RecommenderOrganizationsLocationsInsightTypesInsightsMarkAcceptedArgs;
use crate::providers::gcp::clients::recommender::RecommenderOrganizationsLocationsInsightTypesUpdateConfigArgs;
use crate::providers::gcp::clients::recommender::RecommenderOrganizationsLocationsRecommendersGetConfigArgs;
use crate::providers::gcp::clients::recommender::RecommenderOrganizationsLocationsRecommendersRecommendationsGetArgs;
use crate::providers::gcp::clients::recommender::RecommenderOrganizationsLocationsRecommendersRecommendationsListArgs;
use crate::providers::gcp::clients::recommender::RecommenderOrganizationsLocationsRecommendersRecommendationsMarkClaimedArgs;
use crate::providers::gcp::clients::recommender::RecommenderOrganizationsLocationsRecommendersRecommendationsMarkDismissedArgs;
use crate::providers::gcp::clients::recommender::RecommenderOrganizationsLocationsRecommendersRecommendationsMarkFailedArgs;
use crate::providers::gcp::clients::recommender::RecommenderOrganizationsLocationsRecommendersRecommendationsMarkSucceededArgs;
use crate::providers::gcp::clients::recommender::RecommenderOrganizationsLocationsRecommendersUpdateConfigArgs;
use crate::providers::gcp::clients::recommender::RecommenderProjectsLocationsInsightTypesGetConfigArgs;
use crate::providers::gcp::clients::recommender::RecommenderProjectsLocationsInsightTypesInsightsGetArgs;
use crate::providers::gcp::clients::recommender::RecommenderProjectsLocationsInsightTypesInsightsListArgs;
use crate::providers::gcp::clients::recommender::RecommenderProjectsLocationsInsightTypesInsightsMarkAcceptedArgs;
use crate::providers::gcp::clients::recommender::RecommenderProjectsLocationsInsightTypesUpdateConfigArgs;
use crate::providers::gcp::clients::recommender::RecommenderProjectsLocationsRecommendersGetConfigArgs;
use crate::providers::gcp::clients::recommender::RecommenderProjectsLocationsRecommendersRecommendationsGetArgs;
use crate::providers::gcp::clients::recommender::RecommenderProjectsLocationsRecommendersRecommendationsListArgs;
use crate::providers::gcp::clients::recommender::RecommenderProjectsLocationsRecommendersRecommendationsMarkClaimedArgs;
use crate::providers::gcp::clients::recommender::RecommenderProjectsLocationsRecommendersRecommendationsMarkDismissedArgs;
use crate::providers::gcp::clients::recommender::RecommenderProjectsLocationsRecommendersRecommendationsMarkFailedArgs;
use crate::providers::gcp::clients::recommender::RecommenderProjectsLocationsRecommendersRecommendationsMarkSucceededArgs;
use crate::providers::gcp::clients::recommender::RecommenderProjectsLocationsRecommendersUpdateConfigArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// RecommenderProvider with automatic state tracking.
///
/// # Type Parameters
///
/// * `S` - StateStore implementation (FileStateStore, SqliteStateStore, etc.)
/// * `R` - DNS resolver type for HTTP client
///
/// # Example
///
/// ```rust
/// let state_store = FileStateStore::new("/path", "my-project", "dev");
/// let http_client = SimpleHttpClient::with_resolver(StaticSocketAddr::new(addr));
/// let client = ProviderClient::new("my-project", "dev", state_store, http_client);
/// let provider = RecommenderProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct RecommenderProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> RecommenderProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new RecommenderProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new RecommenderProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Recommender billing accounts locations insight types get config.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn recommender_billing_accounts_locations_insight_types_get_config(
        &self,
        args: &RecommenderBillingAccountsLocationsInsightTypesGetConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommenderV1InsightTypeConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommender_billing_accounts_locations_insight_types_get_config_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = recommender_billing_accounts_locations_insight_types_get_config_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Recommender billing accounts locations insight types insights get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn recommender_billing_accounts_locations_insight_types_insights_get(
        &self,
        args: &RecommenderBillingAccountsLocationsInsightTypesInsightsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommenderV1Insight, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommender_billing_accounts_locations_insight_types_insights_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = recommender_billing_accounts_locations_insight_types_insights_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recommender billing accounts locations insight types insights list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecommenderV1ListInsightsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn recommender_billing_accounts_locations_insight_types_insights_list(
        &self,
        args: &RecommenderBillingAccountsLocationsInsightTypesInsightsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommenderV1ListInsightsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommender_billing_accounts_locations_insight_types_insights_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = recommender_billing_accounts_locations_insight_types_insights_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Recommender billing accounts locations recommenders get config.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn recommender_billing_accounts_locations_recommenders_get_config(
        &self,
        args: &RecommenderBillingAccountsLocationsRecommendersGetConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommenderV1RecommenderConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommender_billing_accounts_locations_recommenders_get_config_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = recommender_billing_accounts_locations_recommenders_get_config_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Recommender billing accounts locations recommenders recommendations get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn recommender_billing_accounts_locations_recommenders_recommendations_get(
        &self,
        args: &RecommenderBillingAccountsLocationsRecommendersRecommendationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommenderV1Recommendation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommender_billing_accounts_locations_recommenders_recommendations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = recommender_billing_accounts_locations_recommenders_recommendations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recommender billing accounts locations recommenders recommendations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecommenderV1ListRecommendationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn recommender_billing_accounts_locations_recommenders_recommendations_list(
        &self,
        args: &RecommenderBillingAccountsLocationsRecommendersRecommendationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommenderV1ListRecommendationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommender_billing_accounts_locations_recommenders_recommendations_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = recommender_billing_accounts_locations_recommenders_recommendations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Recommender folders locations insight types insights get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn recommender_folders_locations_insight_types_insights_get(
        &self,
        args: &RecommenderFoldersLocationsInsightTypesInsightsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommenderV1Insight, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommender_folders_locations_insight_types_insights_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = recommender_folders_locations_insight_types_insights_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recommender folders locations insight types insights list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecommenderV1ListInsightsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn recommender_folders_locations_insight_types_insights_list(
        &self,
        args: &RecommenderFoldersLocationsInsightTypesInsightsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommenderV1ListInsightsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommender_folders_locations_insight_types_insights_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = recommender_folders_locations_insight_types_insights_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Recommender folders locations recommenders recommendations get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn recommender_folders_locations_recommenders_recommendations_get(
        &self,
        args: &RecommenderFoldersLocationsRecommendersRecommendationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommenderV1Recommendation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommender_folders_locations_recommenders_recommendations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = recommender_folders_locations_recommenders_recommendations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recommender folders locations recommenders recommendations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecommenderV1ListRecommendationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn recommender_folders_locations_recommenders_recommendations_list(
        &self,
        args: &RecommenderFoldersLocationsRecommendersRecommendationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommenderV1ListRecommendationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommender_folders_locations_recommenders_recommendations_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = recommender_folders_locations_recommenders_recommendations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Recommender organizations locations insight types get config.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn recommender_organizations_locations_insight_types_get_config(
        &self,
        args: &RecommenderOrganizationsLocationsInsightTypesGetConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommenderV1InsightTypeConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommender_organizations_locations_insight_types_get_config_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = recommender_organizations_locations_insight_types_get_config_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Recommender organizations locations insight types insights get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn recommender_organizations_locations_insight_types_insights_get(
        &self,
        args: &RecommenderOrganizationsLocationsInsightTypesInsightsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommenderV1Insight, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommender_organizations_locations_insight_types_insights_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = recommender_organizations_locations_insight_types_insights_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recommender organizations locations insight types insights list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecommenderV1ListInsightsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn recommender_organizations_locations_insight_types_insights_list(
        &self,
        args: &RecommenderOrganizationsLocationsInsightTypesInsightsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommenderV1ListInsightsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommender_organizations_locations_insight_types_insights_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = recommender_organizations_locations_insight_types_insights_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Recommender organizations locations recommenders get config.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn recommender_organizations_locations_recommenders_get_config(
        &self,
        args: &RecommenderOrganizationsLocationsRecommendersGetConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommenderV1RecommenderConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommender_organizations_locations_recommenders_get_config_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = recommender_organizations_locations_recommenders_get_config_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Recommender organizations locations recommenders recommendations get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn recommender_organizations_locations_recommenders_recommendations_get(
        &self,
        args: &RecommenderOrganizationsLocationsRecommendersRecommendationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommenderV1Recommendation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommender_organizations_locations_recommenders_recommendations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = recommender_organizations_locations_recommenders_recommendations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recommender organizations locations recommenders recommendations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecommenderV1ListRecommendationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn recommender_organizations_locations_recommenders_recommendations_list(
        &self,
        args: &RecommenderOrganizationsLocationsRecommendersRecommendationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommenderV1ListRecommendationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommender_organizations_locations_recommenders_recommendations_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = recommender_organizations_locations_recommenders_recommendations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Recommender projects locations insight types get config.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn recommender_projects_locations_insight_types_get_config(
        &self,
        args: &RecommenderProjectsLocationsInsightTypesGetConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommenderV1InsightTypeConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommender_projects_locations_insight_types_get_config_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = recommender_projects_locations_insight_types_get_config_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Recommender projects locations insight types insights get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn recommender_projects_locations_insight_types_insights_get(
        &self,
        args: &RecommenderProjectsLocationsInsightTypesInsightsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommenderV1Insight, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommender_projects_locations_insight_types_insights_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = recommender_projects_locations_insight_types_insights_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recommender projects locations insight types insights list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecommenderV1ListInsightsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn recommender_projects_locations_insight_types_insights_list(
        &self,
        args: &RecommenderProjectsLocationsInsightTypesInsightsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommenderV1ListInsightsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommender_projects_locations_insight_types_insights_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = recommender_projects_locations_insight_types_insights_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Recommender projects locations recommenders get config.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn recommender_projects_locations_recommenders_get_config(
        &self,
        args: &RecommenderProjectsLocationsRecommendersGetConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommenderV1RecommenderConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommender_projects_locations_recommenders_get_config_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = recommender_projects_locations_recommenders_get_config_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Recommender projects locations recommenders recommendations get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn recommender_projects_locations_recommenders_recommendations_get(
        &self,
        args: &RecommenderProjectsLocationsRecommendersRecommendationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommenderV1Recommendation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommender_projects_locations_recommenders_recommendations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = recommender_projects_locations_recommenders_recommendations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recommender projects locations recommenders recommendations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecommenderV1ListRecommendationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn recommender_projects_locations_recommenders_recommendations_list(
        &self,
        args: &RecommenderProjectsLocationsRecommendersRecommendationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecommenderV1ListRecommendationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recommender_projects_locations_recommenders_recommendations_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = recommender_projects_locations_recommenders_recommendations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
