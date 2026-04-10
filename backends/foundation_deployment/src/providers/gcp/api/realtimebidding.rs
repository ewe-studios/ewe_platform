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
    realtimebidding_bidders_creatives_watch_builder, realtimebidding_bidders_creatives_watch_task,
    realtimebidding_bidders_endpoints_patch_builder, realtimebidding_bidders_endpoints_patch_task,
    realtimebidding_bidders_pretargeting_configs_activate_builder, realtimebidding_bidders_pretargeting_configs_activate_task,
    realtimebidding_bidders_pretargeting_configs_add_targeted_apps_builder, realtimebidding_bidders_pretargeting_configs_add_targeted_apps_task,
    realtimebidding_bidders_pretargeting_configs_add_targeted_publishers_builder, realtimebidding_bidders_pretargeting_configs_add_targeted_publishers_task,
    realtimebidding_bidders_pretargeting_configs_add_targeted_sites_builder, realtimebidding_bidders_pretargeting_configs_add_targeted_sites_task,
    realtimebidding_bidders_pretargeting_configs_create_builder, realtimebidding_bidders_pretargeting_configs_create_task,
    realtimebidding_bidders_pretargeting_configs_delete_builder, realtimebidding_bidders_pretargeting_configs_delete_task,
    realtimebidding_bidders_pretargeting_configs_patch_builder, realtimebidding_bidders_pretargeting_configs_patch_task,
    realtimebidding_bidders_pretargeting_configs_remove_targeted_apps_builder, realtimebidding_bidders_pretargeting_configs_remove_targeted_apps_task,
    realtimebidding_bidders_pretargeting_configs_remove_targeted_publishers_builder, realtimebidding_bidders_pretargeting_configs_remove_targeted_publishers_task,
    realtimebidding_bidders_pretargeting_configs_remove_targeted_sites_builder, realtimebidding_bidders_pretargeting_configs_remove_targeted_sites_task,
    realtimebidding_bidders_pretargeting_configs_suspend_builder, realtimebidding_bidders_pretargeting_configs_suspend_task,
    realtimebidding_bidders_publisher_connections_batch_approve_builder, realtimebidding_bidders_publisher_connections_batch_approve_task,
    realtimebidding_bidders_publisher_connections_batch_reject_builder, realtimebidding_bidders_publisher_connections_batch_reject_task,
    realtimebidding_buyers_creatives_create_builder, realtimebidding_buyers_creatives_create_task,
    realtimebidding_buyers_creatives_patch_builder, realtimebidding_buyers_creatives_patch_task,
    realtimebidding_buyers_user_lists_close_builder, realtimebidding_buyers_user_lists_close_task,
    realtimebidding_buyers_user_lists_create_builder, realtimebidding_buyers_user_lists_create_task,
    realtimebidding_buyers_user_lists_open_builder, realtimebidding_buyers_user_lists_open_task,
    realtimebidding_buyers_user_lists_update_builder, realtimebidding_buyers_user_lists_update_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::realtimebidding::BatchApprovePublisherConnectionsResponse;
use crate::providers::gcp::clients::realtimebidding::BatchRejectPublisherConnectionsResponse;
use crate::providers::gcp::clients::realtimebidding::Creative;
use crate::providers::gcp::clients::realtimebidding::Empty;
use crate::providers::gcp::clients::realtimebidding::Endpoint;
use crate::providers::gcp::clients::realtimebidding::PretargetingConfig;
use crate::providers::gcp::clients::realtimebidding::UserList;
use crate::providers::gcp::clients::realtimebidding::WatchCreativesResponse;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBiddersCreativesWatchArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBiddersEndpointsPatchArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBiddersPretargetingConfigsActivateArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBiddersPretargetingConfigsAddTargetedAppsArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBiddersPretargetingConfigsAddTargetedPublishersArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBiddersPretargetingConfigsAddTargetedSitesArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBiddersPretargetingConfigsCreateArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBiddersPretargetingConfigsDeleteArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBiddersPretargetingConfigsPatchArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBiddersPretargetingConfigsRemoveTargetedAppsArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBiddersPretargetingConfigsRemoveTargetedPublishersArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBiddersPretargetingConfigsRemoveTargetedSitesArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBiddersPretargetingConfigsSuspendArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBiddersPublisherConnectionsBatchApproveArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBiddersPublisherConnectionsBatchRejectArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBuyersCreativesCreateArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBuyersCreativesPatchArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBuyersUserListsCloseArgs;
use crate::providers::gcp::clients::realtimebidding::RealtimebiddingBuyersUserListsCreateArgs;
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

    /// Realtimebidding bidders creatives watch.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Realtimebidding bidders pretargeting configs patch.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Realtimebidding bidders pretargeting configs remove targeted apps.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Realtimebidding bidders pretargeting configs remove targeted publishers.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Realtimebidding bidders pretargeting configs remove targeted sites.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Realtimebidding bidders pretargeting configs suspend.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Realtimebidding buyers user lists open.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Realtimebidding buyers user lists update.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
