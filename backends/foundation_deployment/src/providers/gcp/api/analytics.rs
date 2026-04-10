//! AnalyticsProvider - State-aware analytics API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       analytics API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::analytics::{
    analytics_management_account_user_links_delete_builder, analytics_management_account_user_links_delete_task,
    analytics_management_account_user_links_insert_builder, analytics_management_account_user_links_insert_task,
    analytics_management_account_user_links_update_builder, analytics_management_account_user_links_update_task,
    analytics_management_client_id_hash_client_id_builder, analytics_management_client_id_hash_client_id_task,
    analytics_management_custom_dimensions_insert_builder, analytics_management_custom_dimensions_insert_task,
    analytics_management_custom_dimensions_patch_builder, analytics_management_custom_dimensions_patch_task,
    analytics_management_custom_dimensions_update_builder, analytics_management_custom_dimensions_update_task,
    analytics_management_custom_metrics_insert_builder, analytics_management_custom_metrics_insert_task,
    analytics_management_custom_metrics_patch_builder, analytics_management_custom_metrics_patch_task,
    analytics_management_custom_metrics_update_builder, analytics_management_custom_metrics_update_task,
    analytics_management_experiments_delete_builder, analytics_management_experiments_delete_task,
    analytics_management_experiments_insert_builder, analytics_management_experiments_insert_task,
    analytics_management_experiments_patch_builder, analytics_management_experiments_patch_task,
    analytics_management_experiments_update_builder, analytics_management_experiments_update_task,
    analytics_management_filters_delete_builder, analytics_management_filters_delete_task,
    analytics_management_filters_insert_builder, analytics_management_filters_insert_task,
    analytics_management_filters_patch_builder, analytics_management_filters_patch_task,
    analytics_management_filters_update_builder, analytics_management_filters_update_task,
    analytics_management_goals_insert_builder, analytics_management_goals_insert_task,
    analytics_management_goals_patch_builder, analytics_management_goals_patch_task,
    analytics_management_goals_update_builder, analytics_management_goals_update_task,
    analytics_management_profile_filter_links_delete_builder, analytics_management_profile_filter_links_delete_task,
    analytics_management_profile_filter_links_insert_builder, analytics_management_profile_filter_links_insert_task,
    analytics_management_profile_filter_links_patch_builder, analytics_management_profile_filter_links_patch_task,
    analytics_management_profile_filter_links_update_builder, analytics_management_profile_filter_links_update_task,
    analytics_management_profile_user_links_delete_builder, analytics_management_profile_user_links_delete_task,
    analytics_management_profile_user_links_insert_builder, analytics_management_profile_user_links_insert_task,
    analytics_management_profile_user_links_update_builder, analytics_management_profile_user_links_update_task,
    analytics_management_profiles_delete_builder, analytics_management_profiles_delete_task,
    analytics_management_profiles_insert_builder, analytics_management_profiles_insert_task,
    analytics_management_profiles_patch_builder, analytics_management_profiles_patch_task,
    analytics_management_profiles_update_builder, analytics_management_profiles_update_task,
    analytics_management_remarketing_audience_delete_builder, analytics_management_remarketing_audience_delete_task,
    analytics_management_remarketing_audience_insert_builder, analytics_management_remarketing_audience_insert_task,
    analytics_management_remarketing_audience_patch_builder, analytics_management_remarketing_audience_patch_task,
    analytics_management_remarketing_audience_update_builder, analytics_management_remarketing_audience_update_task,
    analytics_management_unsampled_reports_delete_builder, analytics_management_unsampled_reports_delete_task,
    analytics_management_unsampled_reports_insert_builder, analytics_management_unsampled_reports_insert_task,
    analytics_management_uploads_delete_upload_data_builder, analytics_management_uploads_delete_upload_data_task,
    analytics_management_uploads_upload_data_builder, analytics_management_uploads_upload_data_task,
    analytics_management_web_property_ad_words_links_delete_builder, analytics_management_web_property_ad_words_links_delete_task,
    analytics_management_web_property_ad_words_links_insert_builder, analytics_management_web_property_ad_words_links_insert_task,
    analytics_management_web_property_ad_words_links_patch_builder, analytics_management_web_property_ad_words_links_patch_task,
    analytics_management_web_property_ad_words_links_update_builder, analytics_management_web_property_ad_words_links_update_task,
    analytics_management_webproperties_insert_builder, analytics_management_webproperties_insert_task,
    analytics_management_webproperties_patch_builder, analytics_management_webproperties_patch_task,
    analytics_management_webproperties_update_builder, analytics_management_webproperties_update_task,
    analytics_management_webproperty_user_links_delete_builder, analytics_management_webproperty_user_links_delete_task,
    analytics_management_webproperty_user_links_insert_builder, analytics_management_webproperty_user_links_insert_task,
    analytics_management_webproperty_user_links_update_builder, analytics_management_webproperty_user_links_update_task,
    analytics_provisioning_create_account_ticket_builder, analytics_provisioning_create_account_ticket_task,
    analytics_provisioning_create_account_tree_builder, analytics_provisioning_create_account_tree_task,
    analytics_user_deletion_user_deletion_request_upsert_builder, analytics_user_deletion_user_deletion_request_upsert_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::analytics::AccountTicket;
use crate::providers::gcp::clients::analytics::AccountTreeResponse;
use crate::providers::gcp::clients::analytics::CustomDimension;
use crate::providers::gcp::clients::analytics::CustomMetric;
use crate::providers::gcp::clients::analytics::EntityAdWordsLink;
use crate::providers::gcp::clients::analytics::EntityUserLink;
use crate::providers::gcp::clients::analytics::Experiment;
use crate::providers::gcp::clients::analytics::Filter;
use crate::providers::gcp::clients::analytics::Goal;
use crate::providers::gcp::clients::analytics::HashClientIdResponse;
use crate::providers::gcp::clients::analytics::Profile;
use crate::providers::gcp::clients::analytics::ProfileFilterLink;
use crate::providers::gcp::clients::analytics::RemarketingAudience;
use crate::providers::gcp::clients::analytics::UnsampledReport;
use crate::providers::gcp::clients::analytics::Upload;
use crate::providers::gcp::clients::analytics::UserDeletionRequest;
use crate::providers::gcp::clients::analytics::Webproperty;
use crate::providers::gcp::clients::analytics::AnalyticsManagementAccountUserLinksDeleteArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementAccountUserLinksInsertArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementAccountUserLinksUpdateArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementClientIdHashClientIdArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementCustomDimensionsInsertArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementCustomDimensionsPatchArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementCustomDimensionsUpdateArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementCustomMetricsInsertArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementCustomMetricsPatchArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementCustomMetricsUpdateArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementExperimentsDeleteArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementExperimentsInsertArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementExperimentsPatchArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementExperimentsUpdateArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementFiltersDeleteArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementFiltersInsertArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementFiltersPatchArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementFiltersUpdateArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementGoalsInsertArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementGoalsPatchArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementGoalsUpdateArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementProfileFilterLinksDeleteArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementProfileFilterLinksInsertArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementProfileFilterLinksPatchArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementProfileFilterLinksUpdateArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementProfileUserLinksDeleteArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementProfileUserLinksInsertArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementProfileUserLinksUpdateArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementProfilesDeleteArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementProfilesInsertArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementProfilesPatchArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementProfilesUpdateArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementRemarketingAudienceDeleteArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementRemarketingAudienceInsertArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementRemarketingAudiencePatchArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementRemarketingAudienceUpdateArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementUnsampledReportsDeleteArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementUnsampledReportsInsertArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementUploadsDeleteUploadDataArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementUploadsUploadDataArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementWebPropertyAdWordsLinksDeleteArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementWebPropertyAdWordsLinksInsertArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementWebPropertyAdWordsLinksPatchArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementWebPropertyAdWordsLinksUpdateArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementWebpropertiesInsertArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementWebpropertiesPatchArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementWebpropertiesUpdateArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementWebpropertyUserLinksDeleteArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementWebpropertyUserLinksInsertArgs;
use crate::providers::gcp::clients::analytics::AnalyticsManagementWebpropertyUserLinksUpdateArgs;
use crate::providers::gcp::clients::analytics::AnalyticsProvisioningCreateAccountTicketArgs;
use crate::providers::gcp::clients::analytics::AnalyticsProvisioningCreateAccountTreeArgs;
use crate::providers::gcp::clients::analytics::AnalyticsUserDeletionUserDeletionRequestUpsertArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// AnalyticsProvider with automatic state tracking.
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
/// let provider = AnalyticsProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct AnalyticsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> AnalyticsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new AnalyticsProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Analytics management account user links delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_account_user_links_delete(
        &self,
        args: &AnalyticsManagementAccountUserLinksDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_account_user_links_delete_builder(
            &self.http_client,
            &args.accountId,
            &args.linkId,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_account_user_links_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management account user links insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EntityUserLink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_account_user_links_insert(
        &self,
        args: &AnalyticsManagementAccountUserLinksInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EntityUserLink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_account_user_links_insert_builder(
            &self.http_client,
            &args.accountId,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_account_user_links_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management account user links update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EntityUserLink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_account_user_links_update(
        &self,
        args: &AnalyticsManagementAccountUserLinksUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EntityUserLink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_account_user_links_update_builder(
            &self.http_client,
            &args.accountId,
            &args.linkId,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_account_user_links_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management client id hash client id.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HashClientIdResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_client_id_hash_client_id(
        &self,
        args: &AnalyticsManagementClientIdHashClientIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HashClientIdResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_client_id_hash_client_id_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_client_id_hash_client_id_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management custom dimensions insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CustomDimension result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_custom_dimensions_insert(
        &self,
        args: &AnalyticsManagementCustomDimensionsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CustomDimension, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_custom_dimensions_insert_builder(
            &self.http_client,
            &args.accountId,
            &args.webPropertyId,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_custom_dimensions_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management custom dimensions patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CustomDimension result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_custom_dimensions_patch(
        &self,
        args: &AnalyticsManagementCustomDimensionsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CustomDimension, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_custom_dimensions_patch_builder(
            &self.http_client,
            &args.accountId,
            &args.webPropertyId,
            &args.customDimensionId,
            &args.ignoreCustomDataSourceLinks,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_custom_dimensions_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management custom dimensions update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CustomDimension result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_custom_dimensions_update(
        &self,
        args: &AnalyticsManagementCustomDimensionsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CustomDimension, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_custom_dimensions_update_builder(
            &self.http_client,
            &args.accountId,
            &args.webPropertyId,
            &args.customDimensionId,
            &args.ignoreCustomDataSourceLinks,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_custom_dimensions_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management custom metrics insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CustomMetric result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_custom_metrics_insert(
        &self,
        args: &AnalyticsManagementCustomMetricsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CustomMetric, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_custom_metrics_insert_builder(
            &self.http_client,
            &args.accountId,
            &args.webPropertyId,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_custom_metrics_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management custom metrics patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CustomMetric result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_custom_metrics_patch(
        &self,
        args: &AnalyticsManagementCustomMetricsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CustomMetric, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_custom_metrics_patch_builder(
            &self.http_client,
            &args.accountId,
            &args.webPropertyId,
            &args.customMetricId,
            &args.ignoreCustomDataSourceLinks,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_custom_metrics_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management custom metrics update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CustomMetric result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_custom_metrics_update(
        &self,
        args: &AnalyticsManagementCustomMetricsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CustomMetric, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_custom_metrics_update_builder(
            &self.http_client,
            &args.accountId,
            &args.webPropertyId,
            &args.customMetricId,
            &args.ignoreCustomDataSourceLinks,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_custom_metrics_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management experiments delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_experiments_delete(
        &self,
        args: &AnalyticsManagementExperimentsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_experiments_delete_builder(
            &self.http_client,
            &args.accountId,
            &args.webPropertyId,
            &args.profileId,
            &args.experimentId,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_experiments_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management experiments insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Experiment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_experiments_insert(
        &self,
        args: &AnalyticsManagementExperimentsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Experiment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_experiments_insert_builder(
            &self.http_client,
            &args.accountId,
            &args.webPropertyId,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_experiments_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management experiments patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Experiment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_experiments_patch(
        &self,
        args: &AnalyticsManagementExperimentsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Experiment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_experiments_patch_builder(
            &self.http_client,
            &args.accountId,
            &args.webPropertyId,
            &args.profileId,
            &args.experimentId,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_experiments_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management experiments update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Experiment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_experiments_update(
        &self,
        args: &AnalyticsManagementExperimentsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Experiment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_experiments_update_builder(
            &self.http_client,
            &args.accountId,
            &args.webPropertyId,
            &args.profileId,
            &args.experimentId,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_experiments_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management filters delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Filter result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_filters_delete(
        &self,
        args: &AnalyticsManagementFiltersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Filter, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_filters_delete_builder(
            &self.http_client,
            &args.accountId,
            &args.filterId,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_filters_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management filters insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Filter result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_filters_insert(
        &self,
        args: &AnalyticsManagementFiltersInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Filter, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_filters_insert_builder(
            &self.http_client,
            &args.accountId,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_filters_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management filters patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Filter result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_filters_patch(
        &self,
        args: &AnalyticsManagementFiltersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Filter, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_filters_patch_builder(
            &self.http_client,
            &args.accountId,
            &args.filterId,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_filters_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management filters update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Filter result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_filters_update(
        &self,
        args: &AnalyticsManagementFiltersUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Filter, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_filters_update_builder(
            &self.http_client,
            &args.accountId,
            &args.filterId,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_filters_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management goals insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Goal result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_goals_insert(
        &self,
        args: &AnalyticsManagementGoalsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Goal, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_goals_insert_builder(
            &self.http_client,
            &args.accountId,
            &args.webPropertyId,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_goals_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management goals patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Goal result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_goals_patch(
        &self,
        args: &AnalyticsManagementGoalsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Goal, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_goals_patch_builder(
            &self.http_client,
            &args.accountId,
            &args.webPropertyId,
            &args.profileId,
            &args.goalId,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_goals_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management goals update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Goal result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_goals_update(
        &self,
        args: &AnalyticsManagementGoalsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Goal, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_goals_update_builder(
            &self.http_client,
            &args.accountId,
            &args.webPropertyId,
            &args.profileId,
            &args.goalId,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_goals_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management profile filter links delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_profile_filter_links_delete(
        &self,
        args: &AnalyticsManagementProfileFilterLinksDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_profile_filter_links_delete_builder(
            &self.http_client,
            &args.accountId,
            &args.webPropertyId,
            &args.profileId,
            &args.linkId,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_profile_filter_links_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management profile filter links insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProfileFilterLink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_profile_filter_links_insert(
        &self,
        args: &AnalyticsManagementProfileFilterLinksInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProfileFilterLink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_profile_filter_links_insert_builder(
            &self.http_client,
            &args.accountId,
            &args.webPropertyId,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_profile_filter_links_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management profile filter links patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProfileFilterLink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_profile_filter_links_patch(
        &self,
        args: &AnalyticsManagementProfileFilterLinksPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProfileFilterLink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_profile_filter_links_patch_builder(
            &self.http_client,
            &args.accountId,
            &args.webPropertyId,
            &args.profileId,
            &args.linkId,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_profile_filter_links_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management profile filter links update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProfileFilterLink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_profile_filter_links_update(
        &self,
        args: &AnalyticsManagementProfileFilterLinksUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProfileFilterLink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_profile_filter_links_update_builder(
            &self.http_client,
            &args.accountId,
            &args.webPropertyId,
            &args.profileId,
            &args.linkId,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_profile_filter_links_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management profile user links delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_profile_user_links_delete(
        &self,
        args: &AnalyticsManagementProfileUserLinksDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_profile_user_links_delete_builder(
            &self.http_client,
            &args.accountId,
            &args.webPropertyId,
            &args.profileId,
            &args.linkId,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_profile_user_links_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management profile user links insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EntityUserLink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_profile_user_links_insert(
        &self,
        args: &AnalyticsManagementProfileUserLinksInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EntityUserLink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_profile_user_links_insert_builder(
            &self.http_client,
            &args.accountId,
            &args.webPropertyId,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_profile_user_links_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management profile user links update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EntityUserLink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_profile_user_links_update(
        &self,
        args: &AnalyticsManagementProfileUserLinksUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EntityUserLink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_profile_user_links_update_builder(
            &self.http_client,
            &args.accountId,
            &args.webPropertyId,
            &args.profileId,
            &args.linkId,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_profile_user_links_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management profiles delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_profiles_delete(
        &self,
        args: &AnalyticsManagementProfilesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_profiles_delete_builder(
            &self.http_client,
            &args.accountId,
            &args.webPropertyId,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_profiles_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management profiles insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Profile result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_profiles_insert(
        &self,
        args: &AnalyticsManagementProfilesInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Profile, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_profiles_insert_builder(
            &self.http_client,
            &args.accountId,
            &args.webPropertyId,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_profiles_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management profiles patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Profile result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_profiles_patch(
        &self,
        args: &AnalyticsManagementProfilesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Profile, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_profiles_patch_builder(
            &self.http_client,
            &args.accountId,
            &args.webPropertyId,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_profiles_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management profiles update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Profile result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_profiles_update(
        &self,
        args: &AnalyticsManagementProfilesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Profile, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_profiles_update_builder(
            &self.http_client,
            &args.accountId,
            &args.webPropertyId,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_profiles_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management remarketing audience delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_remarketing_audience_delete(
        &self,
        args: &AnalyticsManagementRemarketingAudienceDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_remarketing_audience_delete_builder(
            &self.http_client,
            &args.accountId,
            &args.webPropertyId,
            &args.remarketingAudienceId,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_remarketing_audience_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management remarketing audience insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RemarketingAudience result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_remarketing_audience_insert(
        &self,
        args: &AnalyticsManagementRemarketingAudienceInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RemarketingAudience, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_remarketing_audience_insert_builder(
            &self.http_client,
            &args.accountId,
            &args.webPropertyId,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_remarketing_audience_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management remarketing audience patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RemarketingAudience result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_remarketing_audience_patch(
        &self,
        args: &AnalyticsManagementRemarketingAudiencePatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RemarketingAudience, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_remarketing_audience_patch_builder(
            &self.http_client,
            &args.accountId,
            &args.webPropertyId,
            &args.remarketingAudienceId,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_remarketing_audience_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management remarketing audience update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RemarketingAudience result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_remarketing_audience_update(
        &self,
        args: &AnalyticsManagementRemarketingAudienceUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RemarketingAudience, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_remarketing_audience_update_builder(
            &self.http_client,
            &args.accountId,
            &args.webPropertyId,
            &args.remarketingAudienceId,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_remarketing_audience_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management unsampled reports delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_unsampled_reports_delete(
        &self,
        args: &AnalyticsManagementUnsampledReportsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_unsampled_reports_delete_builder(
            &self.http_client,
            &args.accountId,
            &args.webPropertyId,
            &args.profileId,
            &args.unsampledReportId,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_unsampled_reports_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management unsampled reports insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UnsampledReport result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_unsampled_reports_insert(
        &self,
        args: &AnalyticsManagementUnsampledReportsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UnsampledReport, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_unsampled_reports_insert_builder(
            &self.http_client,
            &args.accountId,
            &args.webPropertyId,
            &args.profileId,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_unsampled_reports_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management uploads delete upload data.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_uploads_delete_upload_data(
        &self,
        args: &AnalyticsManagementUploadsDeleteUploadDataArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_uploads_delete_upload_data_builder(
            &self.http_client,
            &args.accountId,
            &args.webPropertyId,
            &args.customDataSourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_uploads_delete_upload_data_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management uploads upload data.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Upload result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_uploads_upload_data(
        &self,
        args: &AnalyticsManagementUploadsUploadDataArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Upload, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_uploads_upload_data_builder(
            &self.http_client,
            &args.accountId,
            &args.webPropertyId,
            &args.customDataSourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_uploads_upload_data_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management web property ad words links delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_web_property_ad_words_links_delete(
        &self,
        args: &AnalyticsManagementWebPropertyAdWordsLinksDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_web_property_ad_words_links_delete_builder(
            &self.http_client,
            &args.accountId,
            &args.webPropertyId,
            &args.webPropertyAdWordsLinkId,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_web_property_ad_words_links_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management web property ad words links insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EntityAdWordsLink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_web_property_ad_words_links_insert(
        &self,
        args: &AnalyticsManagementWebPropertyAdWordsLinksInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EntityAdWordsLink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_web_property_ad_words_links_insert_builder(
            &self.http_client,
            &args.accountId,
            &args.webPropertyId,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_web_property_ad_words_links_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management web property ad words links patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EntityAdWordsLink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_web_property_ad_words_links_patch(
        &self,
        args: &AnalyticsManagementWebPropertyAdWordsLinksPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EntityAdWordsLink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_web_property_ad_words_links_patch_builder(
            &self.http_client,
            &args.accountId,
            &args.webPropertyId,
            &args.webPropertyAdWordsLinkId,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_web_property_ad_words_links_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management web property ad words links update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EntityAdWordsLink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_web_property_ad_words_links_update(
        &self,
        args: &AnalyticsManagementWebPropertyAdWordsLinksUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EntityAdWordsLink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_web_property_ad_words_links_update_builder(
            &self.http_client,
            &args.accountId,
            &args.webPropertyId,
            &args.webPropertyAdWordsLinkId,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_web_property_ad_words_links_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management webproperties insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Webproperty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_webproperties_insert(
        &self,
        args: &AnalyticsManagementWebpropertiesInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Webproperty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_webproperties_insert_builder(
            &self.http_client,
            &args.accountId,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_webproperties_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management webproperties patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Webproperty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_webproperties_patch(
        &self,
        args: &AnalyticsManagementWebpropertiesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Webproperty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_webproperties_patch_builder(
            &self.http_client,
            &args.accountId,
            &args.webPropertyId,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_webproperties_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management webproperties update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Webproperty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_webproperties_update(
        &self,
        args: &AnalyticsManagementWebpropertiesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Webproperty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_webproperties_update_builder(
            &self.http_client,
            &args.accountId,
            &args.webPropertyId,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_webproperties_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management webproperty user links delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_webproperty_user_links_delete(
        &self,
        args: &AnalyticsManagementWebpropertyUserLinksDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_webproperty_user_links_delete_builder(
            &self.http_client,
            &args.accountId,
            &args.webPropertyId,
            &args.linkId,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_webproperty_user_links_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management webproperty user links insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EntityUserLink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_webproperty_user_links_insert(
        &self,
        args: &AnalyticsManagementWebpropertyUserLinksInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EntityUserLink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_webproperty_user_links_insert_builder(
            &self.http_client,
            &args.accountId,
            &args.webPropertyId,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_webproperty_user_links_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics management webproperty user links update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EntityUserLink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_management_webproperty_user_links_update(
        &self,
        args: &AnalyticsManagementWebpropertyUserLinksUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EntityUserLink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_management_webproperty_user_links_update_builder(
            &self.http_client,
            &args.accountId,
            &args.webPropertyId,
            &args.linkId,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_management_webproperty_user_links_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics provisioning create account ticket.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccountTicket result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_provisioning_create_account_ticket(
        &self,
        args: &AnalyticsProvisioningCreateAccountTicketArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccountTicket, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_provisioning_create_account_ticket_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_provisioning_create_account_ticket_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics provisioning create account tree.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccountTreeResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_provisioning_create_account_tree(
        &self,
        args: &AnalyticsProvisioningCreateAccountTreeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccountTreeResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_provisioning_create_account_tree_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_provisioning_create_account_tree_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analytics user deletion user deletion request upsert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UserDeletionRequest result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analytics_user_deletion_user_deletion_request_upsert(
        &self,
        args: &AnalyticsUserDeletionUserDeletionRequestUpsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UserDeletionRequest, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analytics_user_deletion_user_deletion_request_upsert_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = analytics_user_deletion_user_deletion_request_upsert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
