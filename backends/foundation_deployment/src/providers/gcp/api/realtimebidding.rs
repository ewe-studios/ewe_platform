//! RealtimebiddingProvider - State-aware realtimebidding API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       realtimebidding API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::realtimebidding::{
    realtimebidding_bidders_get_builder, realtimebidding_bidders_get_task,
    realtimebidding_bidders_list_builder, realtimebidding_bidders_list_task,
    realtimebidding_bidders_creatives_list_builder, realtimebidding_bidders_creatives_list_task,
    realtimebidding_bidders_creatives_watch_builder, realtimebidding_bidders_creatives_watch_task,
    realtimebidding_bidders_endpoints_get_builder, realtimebidding_bidders_endpoints_get_task,
    realtimebidding_bidders_endpoints_list_builder, realtimebidding_bidders_endpoints_list_task,
    realtimebidding_bidders_endpoints_patch_builder, realtimebidding_bidders_endpoints_patch_task,
    realtimebidding_bidders_pretargeting_configs_activate_builder, realtimebidding_bidders_pretargeting_configs_activate_task,
    realtimebidding_bidders_pretargeting_configs_add_targeted_apps_builder, realtimebidding_bidders_pretargeting_configs_add_targeted_apps_task,
    realtimebidding_bidders_pretargeting_configs_add_targeted_publishers_builder, realtimebidding_bidders_pretargeting_configs_add_targeted_publishers_task,
    realtimebidding_bidders_pretargeting_configs_add_targeted_sites_builder, realtimebidding_bidders_pretargeting_configs_add_targeted_sites_task,
    realtimebidding_bidders_pretargeting_configs_create_builder, realtimebidding_bidders_pretargeting_configs_create_task,
    realtimebidding_bidders_pretargeting_configs_delete_builder, realtimebidding_bidders_pretargeting_configs_delete_task,
    realtimebidding_bidders_pretargeting_configs_get_builder, realtimebidding_bidders_pretargeting_configs_get_task,
    realtimebidding_bidders_pretargeting_configs_list_builder, realtimebidding_bidders_pretargeting_configs_list_task,
    realtimebidding_bidders_pretargeting_configs_patch_builder, realtimebidding_bidders_pretargeting_configs_patch_task,
    realtimebidding_bidders_pretargeting_configs_remove_targeted_apps_builder, realtimebidding_bidders_pretargeting_configs_remove_targeted_apps_task,
    realtimebidding_bidders_pretargeting_configs_remove_targeted_publishers_builder, realtimebidding_bidders_pretargeting_configs_remove_targeted_publishers_task,
    realtimebidding_bidders_pretargeting_configs_remove_targeted_sites_builder, realtimebidding_bidders_pretargeting_configs_remove_targeted_sites_task,
    realtimebidding_bidders_pretargeting_configs_suspend_builder, realtimebidding_bidders_pretargeting_configs_suspend_task,
    realtimebidding_bidders_publisher_connections_batch_approve_builder, realtimebidding_bidders_publisher_connections_batch_approve_task,
    realtimebidding_bidders_publisher_connections_batch_reject_builder, realtimebidding_bidders_publisher_connections_batch_reject_task,
    realtimebidding_bidders_publisher_connections_get_builder, realtimebidding_bidders_publisher_connections_get_task,
    realtimebidding_bidders_publisher_connections_list_builder, realtimebidding_bidders_publisher_connections_list_task,
    realtimebidding_buyers_get_builder, realtimebidding_buyers_get_task,
    realtimebidding_buyers_get_remarketing_tag_builder, realtimebidding_buyers_get_remarketing_tag_task,
    realtimebidding_buyers_list_builder, realtimebidding_buyers_list_task,
    realtimebidding_buyers_creatives_create_builder, realtimebidding_buyers_creatives_create_task,
    realtimebidding_buyers_creatives_get_builder, realtimebidding_buyers_creatives_get_task,
    realtimebidding_buyers_creatives_list_builder, realtimebidding_buyers_creatives_list_task,
    realtimebidding_buyers_creatives_patch_builder, realtimebidding_buyers_creatives_patch_task,
    realtimebidding_buyers_user_lists_close_builder, realtimebidding_buyers_user_lists_close_task,
    realtimebidding_buyers_user_lists_create_builder, realtimebidding_buyers_user_lists_create_task,
    realtimebidding_buyers_user_lists_get_builder, realtimebidding_buyers_user_lists_get_task,
    realtimebidding_buyers_user_lists_get_remarketing_tag_builder, realtimebidding_buyers_user_lists_get_remarketing_tag_task,
    realtimebidding_buyers_user_lists_list_builder, realtimebidding_buyers_user_lists_list_task,
    realtimebidding_buyers_user_lists_open_builder, realtimebidding_buyers_user_lists_open_task,
    realtimebidding_buyers_user_lists_update_builder, realtimebidding_buyers_user_lists_update_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::realtimebidding::BatchApprovePublisherConnectionsResponse;
use crate::providers::gcp::clients::realtimebidding::BatchRejectPublisherConnectionsResponse;
use crate::providers::gcp::clients::realtimebidding::Bidder;
use crate::providers::gcp::clients::realtimebidding::Buyer;
use crate::providers::gcp::clients::realtimebidding::Creative;
use crate::providers::gcp::clients::realtimebidding::Empty;
use crate::providers::gcp::clients::realtimebidding::Endpoint;
use crate::providers::gcp::clients::realtimebidding::GetRemarketingTagResponse;
use crate::providers::gcp::clients::realtimebidding::ListBiddersResponse;
use crate::providers::gcp::clients::realtimebidding::ListBuyersResponse;
use crate::providers::gcp::clients::realtimebidding::ListCreativesResponse;
use crate::providers::gcp::clients::realtimebidding::ListEndpointsResponse;
use crate::providers::gcp::clients::realtimebidding::ListPretargetingConfigsResponse;
use crate::providers::gcp::clients::realtimebidding::ListPublisherConnectionsResponse;
use crate::providers::gcp::clients::realtimebidding::ListUserListsResponse;
use crate::providers::gcp::clients::realtimebidding::PretargetingConfig;
use crate::providers::gcp::clients::realtimebidding::PublisherConnection;
use crate::providers::gcp::clients::realtimebidding::UserList;
use crate::providers::gcp::clients::realtimebidding::WatchCreativesResponse;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBiddersCreativesListArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBiddersCreativesWatchArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBiddersEndpointsGetArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBiddersEndpointsListArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBiddersEndpointsPatchArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBiddersGetArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBiddersListArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBiddersPretargetingConfigsActivateArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBiddersPretargetingConfigsAddTargetedAppsArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBiddersPretargetingConfigsAddTargetedPublishersArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBiddersPretargetingConfigsAddTargetedSitesArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBiddersPretargetingConfigsCreateArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBiddersPretargetingConfigsDeleteArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBiddersPretargetingConfigsGetArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBiddersPretargetingConfigsListArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBiddersPretargetingConfigsPatchArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBiddersPretargetingConfigsRemoveTargetedAppsArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBiddersPretargetingConfigsRemoveTargetedPublishersArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBiddersPretargetingConfigsRemoveTargetedSitesArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBiddersPretargetingConfigsSuspendArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBiddersPublisherConnectionsBatchApproveArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBiddersPublisherConnectionsBatchRejectArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBiddersPublisherConnectionsGetArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBiddersPublisherConnectionsListArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBuyersCreativesCreateArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBuyersCreativesGetArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBuyersCreativesListArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBuyersCreativesPatchArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBuyersGetArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBuyersGetRemarketingTagArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBuyersListArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBuyersUserListsCloseArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBuyersUserListsCreateArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBuyersUserListsGetArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBuyersUserListsGetRemarketingTagArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBuyersUserListsListArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBuyersUserListsOpenArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBuyersUserListsUpdateArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// RealtimebiddingProvider with automatic state tracking.
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
/// let provider = RealtimebiddingProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct RealtimebiddingProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> RealtimebiddingProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new RealtimebiddingProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Realtimebidding bidders get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Bidder result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn realtimebidding_bidders_get(
        &self,
        args: &RealtimebiddingBiddersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Bidder, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = realtimebidding_bidders_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = realtimebidding_bidders_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Realtimebidding bidders list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListBiddersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn realtimebidding_bidders_list(
        &self,
        args: &RealtimebiddingBiddersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBiddersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = realtimebidding_bidders_list_builder(
            &self.http_client,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = realtimebidding_bidders_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Realtimebidding bidders creatives list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCreativesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn realtimebidding_bidders_creatives_list(
        &self,
        args: &RealtimebiddingBiddersCreativesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCreativesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = realtimebidding_bidders_creatives_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = realtimebidding_bidders_creatives_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Realtimebidding bidders creatives watch.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WatchCreativesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn realtimebidding_bidders_creatives_watch(
        &self,
        args: &RealtimebiddingBiddersCreativesWatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WatchCreativesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = realtimebidding_bidders_creatives_watch_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = realtimebidding_bidders_creatives_watch_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Realtimebidding bidders endpoints get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Endpoint result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn realtimebidding_bidders_endpoints_get(
        &self,
        args: &RealtimebiddingBiddersEndpointsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Endpoint, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = realtimebidding_bidders_endpoints_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = realtimebidding_bidders_endpoints_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Realtimebidding bidders endpoints list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListEndpointsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn realtimebidding_bidders_endpoints_list(
        &self,
        args: &RealtimebiddingBiddersEndpointsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListEndpointsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = realtimebidding_bidders_endpoints_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = realtimebidding_bidders_endpoints_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Realtimebidding bidders endpoints patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Endpoint result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn realtimebidding_bidders_endpoints_patch(
        &self,
        args: &RealtimebiddingBiddersEndpointsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Endpoint, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = realtimebidding_bidders_endpoints_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = realtimebidding_bidders_endpoints_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Realtimebidding bidders pretargeting configs activate.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PretargetingConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn realtimebidding_bidders_pretargeting_configs_activate(
        &self,
        args: &RealtimebiddingBiddersPretargetingConfigsActivateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PretargetingConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = realtimebidding_bidders_pretargeting_configs_activate_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = realtimebidding_bidders_pretargeting_configs_activate_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Realtimebidding bidders pretargeting configs add targeted apps.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PretargetingConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn realtimebidding_bidders_pretargeting_configs_add_targeted_apps(
        &self,
        args: &RealtimebiddingBiddersPretargetingConfigsAddTargetedAppsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PretargetingConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = realtimebidding_bidders_pretargeting_configs_add_targeted_apps_builder(
            &self.http_client,
            &args.pretargetingConfig,
        )
        .map_err(ProviderError::Api)?;

        let task = realtimebidding_bidders_pretargeting_configs_add_targeted_apps_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Realtimebidding bidders pretargeting configs add targeted publishers.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PretargetingConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn realtimebidding_bidders_pretargeting_configs_add_targeted_publishers(
        &self,
        args: &RealtimebiddingBiddersPretargetingConfigsAddTargetedPublishersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PretargetingConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = realtimebidding_bidders_pretargeting_configs_add_targeted_publishers_builder(
            &self.http_client,
            &args.pretargetingConfig,
        )
        .map_err(ProviderError::Api)?;

        let task = realtimebidding_bidders_pretargeting_configs_add_targeted_publishers_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Realtimebidding bidders pretargeting configs add targeted sites.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PretargetingConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn realtimebidding_bidders_pretargeting_configs_add_targeted_sites(
        &self,
        args: &RealtimebiddingBiddersPretargetingConfigsAddTargetedSitesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PretargetingConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = realtimebidding_bidders_pretargeting_configs_add_targeted_sites_builder(
            &self.http_client,
            &args.pretargetingConfig,
        )
        .map_err(ProviderError::Api)?;

        let task = realtimebidding_bidders_pretargeting_configs_add_targeted_sites_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Realtimebidding bidders pretargeting configs create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PretargetingConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn realtimebidding_bidders_pretargeting_configs_create(
        &self,
        args: &RealtimebiddingBiddersPretargetingConfigsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PretargetingConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = realtimebidding_bidders_pretargeting_configs_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = realtimebidding_bidders_pretargeting_configs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Realtimebidding bidders pretargeting configs delete.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn realtimebidding_bidders_pretargeting_configs_delete(
        &self,
        args: &RealtimebiddingBiddersPretargetingConfigsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = realtimebidding_bidders_pretargeting_configs_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = realtimebidding_bidders_pretargeting_configs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Realtimebidding bidders pretargeting configs get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PretargetingConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn realtimebidding_bidders_pretargeting_configs_get(
        &self,
        args: &RealtimebiddingBiddersPretargetingConfigsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PretargetingConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = realtimebidding_bidders_pretargeting_configs_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = realtimebidding_bidders_pretargeting_configs_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Realtimebidding bidders pretargeting configs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListPretargetingConfigsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn realtimebidding_bidders_pretargeting_configs_list(
        &self,
        args: &RealtimebiddingBiddersPretargetingConfigsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListPretargetingConfigsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = realtimebidding_bidders_pretargeting_configs_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = realtimebidding_bidders_pretargeting_configs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Realtimebidding bidders pretargeting configs patch.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PretargetingConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn realtimebidding_bidders_pretargeting_configs_patch(
        &self,
        args: &RealtimebiddingBiddersPretargetingConfigsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PretargetingConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = realtimebidding_bidders_pretargeting_configs_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = realtimebidding_bidders_pretargeting_configs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Realtimebidding bidders pretargeting configs remove targeted apps.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PretargetingConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn realtimebidding_bidders_pretargeting_configs_remove_targeted_apps(
        &self,
        args: &RealtimebiddingBiddersPretargetingConfigsRemoveTargetedAppsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PretargetingConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = realtimebidding_bidders_pretargeting_configs_remove_targeted_apps_builder(
            &self.http_client,
            &args.pretargetingConfig,
        )
        .map_err(ProviderError::Api)?;

        let task = realtimebidding_bidders_pretargeting_configs_remove_targeted_apps_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Realtimebidding bidders pretargeting configs remove targeted publishers.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PretargetingConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn realtimebidding_bidders_pretargeting_configs_remove_targeted_publishers(
        &self,
        args: &RealtimebiddingBiddersPretargetingConfigsRemoveTargetedPublishersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PretargetingConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = realtimebidding_bidders_pretargeting_configs_remove_targeted_publishers_builder(
            &self.http_client,
            &args.pretargetingConfig,
        )
        .map_err(ProviderError::Api)?;

        let task = realtimebidding_bidders_pretargeting_configs_remove_targeted_publishers_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Realtimebidding bidders pretargeting configs remove targeted sites.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PretargetingConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn realtimebidding_bidders_pretargeting_configs_remove_targeted_sites(
        &self,
        args: &RealtimebiddingBiddersPretargetingConfigsRemoveTargetedSitesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PretargetingConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = realtimebidding_bidders_pretargeting_configs_remove_targeted_sites_builder(
            &self.http_client,
            &args.pretargetingConfig,
        )
        .map_err(ProviderError::Api)?;

        let task = realtimebidding_bidders_pretargeting_configs_remove_targeted_sites_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Realtimebidding bidders pretargeting configs suspend.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PretargetingConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn realtimebidding_bidders_pretargeting_configs_suspend(
        &self,
        args: &RealtimebiddingBiddersPretargetingConfigsSuspendArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PretargetingConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = realtimebidding_bidders_pretargeting_configs_suspend_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = realtimebidding_bidders_pretargeting_configs_suspend_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Realtimebidding bidders publisher connections batch approve.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchApprovePublisherConnectionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn realtimebidding_bidders_publisher_connections_batch_approve(
        &self,
        args: &RealtimebiddingBiddersPublisherConnectionsBatchApproveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchApprovePublisherConnectionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = realtimebidding_bidders_publisher_connections_batch_approve_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = realtimebidding_bidders_publisher_connections_batch_approve_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Realtimebidding bidders publisher connections batch reject.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchRejectPublisherConnectionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn realtimebidding_bidders_publisher_connections_batch_reject(
        &self,
        args: &RealtimebiddingBiddersPublisherConnectionsBatchRejectArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchRejectPublisherConnectionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = realtimebidding_bidders_publisher_connections_batch_reject_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = realtimebidding_bidders_publisher_connections_batch_reject_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Realtimebidding bidders publisher connections get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PublisherConnection result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn realtimebidding_bidders_publisher_connections_get(
        &self,
        args: &RealtimebiddingBiddersPublisherConnectionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PublisherConnection, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = realtimebidding_bidders_publisher_connections_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = realtimebidding_bidders_publisher_connections_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Realtimebidding bidders publisher connections list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListPublisherConnectionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn realtimebidding_bidders_publisher_connections_list(
        &self,
        args: &RealtimebiddingBiddersPublisherConnectionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListPublisherConnectionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = realtimebidding_bidders_publisher_connections_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = realtimebidding_bidders_publisher_connections_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Realtimebidding buyers get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Buyer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn realtimebidding_buyers_get(
        &self,
        args: &RealtimebiddingBuyersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Buyer, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = realtimebidding_buyers_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = realtimebidding_buyers_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Realtimebidding buyers get remarketing tag.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GetRemarketingTagResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn realtimebidding_buyers_get_remarketing_tag(
        &self,
        args: &RealtimebiddingBuyersGetRemarketingTagArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GetRemarketingTagResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = realtimebidding_buyers_get_remarketing_tag_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = realtimebidding_buyers_get_remarketing_tag_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Realtimebidding buyers list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListBuyersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn realtimebidding_buyers_list(
        &self,
        args: &RealtimebiddingBuyersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBuyersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = realtimebidding_buyers_list_builder(
            &self.http_client,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = realtimebidding_buyers_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Realtimebidding buyers creatives create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Creative result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn realtimebidding_buyers_creatives_create(
        &self,
        args: &RealtimebiddingBuyersCreativesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Creative, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = realtimebidding_buyers_creatives_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = realtimebidding_buyers_creatives_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Realtimebidding buyers creatives get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Creative result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn realtimebidding_buyers_creatives_get(
        &self,
        args: &RealtimebiddingBuyersCreativesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Creative, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = realtimebidding_buyers_creatives_get_builder(
            &self.http_client,
            &args.name,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = realtimebidding_buyers_creatives_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Realtimebidding buyers creatives list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCreativesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn realtimebidding_buyers_creatives_list(
        &self,
        args: &RealtimebiddingBuyersCreativesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCreativesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = realtimebidding_buyers_creatives_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = realtimebidding_buyers_creatives_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Realtimebidding buyers creatives patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Creative result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn realtimebidding_buyers_creatives_patch(
        &self,
        args: &RealtimebiddingBuyersCreativesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Creative, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = realtimebidding_buyers_creatives_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = realtimebidding_buyers_creatives_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Realtimebidding buyers user lists close.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UserList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn realtimebidding_buyers_user_lists_close(
        &self,
        args: &RealtimebiddingBuyersUserListsCloseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UserList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = realtimebidding_buyers_user_lists_close_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = realtimebidding_buyers_user_lists_close_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Realtimebidding buyers user lists create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UserList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn realtimebidding_buyers_user_lists_create(
        &self,
        args: &RealtimebiddingBuyersUserListsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UserList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = realtimebidding_buyers_user_lists_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = realtimebidding_buyers_user_lists_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Realtimebidding buyers user lists get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UserList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn realtimebidding_buyers_user_lists_get(
        &self,
        args: &RealtimebiddingBuyersUserListsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UserList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = realtimebidding_buyers_user_lists_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = realtimebidding_buyers_user_lists_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Realtimebidding buyers user lists get remarketing tag.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GetRemarketingTagResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn realtimebidding_buyers_user_lists_get_remarketing_tag(
        &self,
        args: &RealtimebiddingBuyersUserListsGetRemarketingTagArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GetRemarketingTagResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = realtimebidding_buyers_user_lists_get_remarketing_tag_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = realtimebidding_buyers_user_lists_get_remarketing_tag_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Realtimebidding buyers user lists list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListUserListsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn realtimebidding_buyers_user_lists_list(
        &self,
        args: &RealtimebiddingBuyersUserListsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListUserListsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = realtimebidding_buyers_user_lists_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = realtimebidding_buyers_user_lists_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Realtimebidding buyers user lists open.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UserList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn realtimebidding_buyers_user_lists_open(
        &self,
        args: &RealtimebiddingBuyersUserListsOpenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UserList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = realtimebidding_buyers_user_lists_open_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = realtimebidding_buyers_user_lists_open_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Realtimebidding buyers user lists update.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UserList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn realtimebidding_buyers_user_lists_update(
        &self,
        args: &RealtimebiddingBuyersUserListsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UserList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = realtimebidding_buyers_user_lists_update_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = realtimebidding_buyers_user_lists_update_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
