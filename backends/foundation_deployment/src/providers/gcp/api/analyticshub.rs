//! AnalyticshubProvider - State-aware analyticshub API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       analyticshub API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::analyticshub::{
    analyticshub_organizations_locations_data_exchanges_list_builder, analyticshub_organizations_locations_data_exchanges_list_task,
    analyticshub_projects_locations_data_exchanges_create_builder, analyticshub_projects_locations_data_exchanges_create_task,
    analyticshub_projects_locations_data_exchanges_delete_builder, analyticshub_projects_locations_data_exchanges_delete_task,
    analyticshub_projects_locations_data_exchanges_get_builder, analyticshub_projects_locations_data_exchanges_get_task,
    analyticshub_projects_locations_data_exchanges_get_iam_policy_builder, analyticshub_projects_locations_data_exchanges_get_iam_policy_task,
    analyticshub_projects_locations_data_exchanges_list_builder, analyticshub_projects_locations_data_exchanges_list_task,
    analyticshub_projects_locations_data_exchanges_list_subscriptions_builder, analyticshub_projects_locations_data_exchanges_list_subscriptions_task,
    analyticshub_projects_locations_data_exchanges_patch_builder, analyticshub_projects_locations_data_exchanges_patch_task,
    analyticshub_projects_locations_data_exchanges_set_iam_policy_builder, analyticshub_projects_locations_data_exchanges_set_iam_policy_task,
    analyticshub_projects_locations_data_exchanges_subscribe_builder, analyticshub_projects_locations_data_exchanges_subscribe_task,
    analyticshub_projects_locations_data_exchanges_test_iam_permissions_builder, analyticshub_projects_locations_data_exchanges_test_iam_permissions_task,
    analyticshub_projects_locations_data_exchanges_listings_create_builder, analyticshub_projects_locations_data_exchanges_listings_create_task,
    analyticshub_projects_locations_data_exchanges_listings_delete_builder, analyticshub_projects_locations_data_exchanges_listings_delete_task,
    analyticshub_projects_locations_data_exchanges_listings_get_builder, analyticshub_projects_locations_data_exchanges_listings_get_task,
    analyticshub_projects_locations_data_exchanges_listings_get_iam_policy_builder, analyticshub_projects_locations_data_exchanges_listings_get_iam_policy_task,
    analyticshub_projects_locations_data_exchanges_listings_list_builder, analyticshub_projects_locations_data_exchanges_listings_list_task,
    analyticshub_projects_locations_data_exchanges_listings_list_subscriptions_builder, analyticshub_projects_locations_data_exchanges_listings_list_subscriptions_task,
    analyticshub_projects_locations_data_exchanges_listings_patch_builder, analyticshub_projects_locations_data_exchanges_listings_patch_task,
    analyticshub_projects_locations_data_exchanges_listings_set_iam_policy_builder, analyticshub_projects_locations_data_exchanges_listings_set_iam_policy_task,
    analyticshub_projects_locations_data_exchanges_listings_subscribe_builder, analyticshub_projects_locations_data_exchanges_listings_subscribe_task,
    analyticshub_projects_locations_data_exchanges_listings_test_iam_permissions_builder, analyticshub_projects_locations_data_exchanges_listings_test_iam_permissions_task,
    analyticshub_projects_locations_data_exchanges_query_templates_approve_builder, analyticshub_projects_locations_data_exchanges_query_templates_approve_task,
    analyticshub_projects_locations_data_exchanges_query_templates_create_builder, analyticshub_projects_locations_data_exchanges_query_templates_create_task,
    analyticshub_projects_locations_data_exchanges_query_templates_delete_builder, analyticshub_projects_locations_data_exchanges_query_templates_delete_task,
    analyticshub_projects_locations_data_exchanges_query_templates_get_builder, analyticshub_projects_locations_data_exchanges_query_templates_get_task,
    analyticshub_projects_locations_data_exchanges_query_templates_list_builder, analyticshub_projects_locations_data_exchanges_query_templates_list_task,
    analyticshub_projects_locations_data_exchanges_query_templates_patch_builder, analyticshub_projects_locations_data_exchanges_query_templates_patch_task,
    analyticshub_projects_locations_data_exchanges_query_templates_submit_builder, analyticshub_projects_locations_data_exchanges_query_templates_submit_task,
    analyticshub_projects_locations_subscriptions_delete_builder, analyticshub_projects_locations_subscriptions_delete_task,
    analyticshub_projects_locations_subscriptions_get_builder, analyticshub_projects_locations_subscriptions_get_task,
    analyticshub_projects_locations_subscriptions_get_iam_policy_builder, analyticshub_projects_locations_subscriptions_get_iam_policy_task,
    analyticshub_projects_locations_subscriptions_list_builder, analyticshub_projects_locations_subscriptions_list_task,
    analyticshub_projects_locations_subscriptions_refresh_builder, analyticshub_projects_locations_subscriptions_refresh_task,
    analyticshub_projects_locations_subscriptions_revoke_builder, analyticshub_projects_locations_subscriptions_revoke_task,
    analyticshub_projects_locations_subscriptions_set_iam_policy_builder, analyticshub_projects_locations_subscriptions_set_iam_policy_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::analyticshub::DataExchange;
use crate::providers::gcp::clients::analyticshub::Empty;
use crate::providers::gcp::clients::analyticshub::ListDataExchangesResponse;
use crate::providers::gcp::clients::analyticshub::ListListingsResponse;
use crate::providers::gcp::clients::analyticshub::ListOrgDataExchangesResponse;
use crate::providers::gcp::clients::analyticshub::ListQueryTemplatesResponse;
use crate::providers::gcp::clients::analyticshub::ListSharedResourceSubscriptionsResponse;
use crate::providers::gcp::clients::analyticshub::ListSubscriptionsResponse;
use crate::providers::gcp::clients::analyticshub::Listing;
use crate::providers::gcp::clients::analyticshub::Operation;
use crate::providers::gcp::clients::analyticshub::Policy;
use crate::providers::gcp::clients::analyticshub::QueryTemplate;
use crate::providers::gcp::clients::analyticshub::RevokeSubscriptionResponse;
use crate::providers::gcp::clients::analyticshub::SubscribeListingResponse;
use crate::providers::gcp::clients::analyticshub::Subscription;
use crate::providers::gcp::clients::analyticshub::TestIamPermissionsResponse;
use crate::providers::gcp::clients::analyticshub::AnalyticshubOrganizationsLocationsDataExchangesListArgs;
use crate::providers::gcp::clients::analyticshub::AnalyticshubProjectsLocationsDataExchangesCreateArgs;
use crate::providers::gcp::clients::analyticshub::AnalyticshubProjectsLocationsDataExchangesDeleteArgs;
use crate::providers::gcp::clients::analyticshub::AnalyticshubProjectsLocationsDataExchangesGetArgs;
use crate::providers::gcp::clients::analyticshub::AnalyticshubProjectsLocationsDataExchangesGetIamPolicyArgs;
use crate::providers::gcp::clients::analyticshub::AnalyticshubProjectsLocationsDataExchangesListArgs;
use crate::providers::gcp::clients::analyticshub::AnalyticshubProjectsLocationsDataExchangesListSubscriptionsArgs;
use crate::providers::gcp::clients::analyticshub::AnalyticshubProjectsLocationsDataExchangesListingsCreateArgs;
use crate::providers::gcp::clients::analyticshub::AnalyticshubProjectsLocationsDataExchangesListingsDeleteArgs;
use crate::providers::gcp::clients::analyticshub::AnalyticshubProjectsLocationsDataExchangesListingsGetArgs;
use crate::providers::gcp::clients::analyticshub::AnalyticshubProjectsLocationsDataExchangesListingsGetIamPolicyArgs;
use crate::providers::gcp::clients::analyticshub::AnalyticshubProjectsLocationsDataExchangesListingsListArgs;
use crate::providers::gcp::clients::analyticshub::AnalyticshubProjectsLocationsDataExchangesListingsListSubscriptionsArgs;
use crate::providers::gcp::clients::analyticshub::AnalyticshubProjectsLocationsDataExchangesListingsPatchArgs;
use crate::providers::gcp::clients::analyticshub::AnalyticshubProjectsLocationsDataExchangesListingsSetIamPolicyArgs;
use crate::providers::gcp::clients::analyticshub::AnalyticshubProjectsLocationsDataExchangesListingsSubscribeArgs;
use crate::providers::gcp::clients::analyticshub::AnalyticshubProjectsLocationsDataExchangesListingsTestIamPermissionsArgs;
use crate::providers::gcp::clients::analyticshub::AnalyticshubProjectsLocationsDataExchangesPatchArgs;
use crate::providers::gcp::clients::analyticshub::AnalyticshubProjectsLocationsDataExchangesQueryTemplatesApproveArgs;
use crate::providers::gcp::clients::analyticshub::AnalyticshubProjectsLocationsDataExchangesQueryTemplatesCreateArgs;
use crate::providers::gcp::clients::analyticshub::AnalyticshubProjectsLocationsDataExchangesQueryTemplatesDeleteArgs;
use crate::providers::gcp::clients::analyticshub::AnalyticshubProjectsLocationsDataExchangesQueryTemplatesGetArgs;
use crate::providers::gcp::clients::analyticshub::AnalyticshubProjectsLocationsDataExchangesQueryTemplatesListArgs;
use crate::providers::gcp::clients::analyticshub::AnalyticshubProjectsLocationsDataExchangesQueryTemplatesPatchArgs;
use crate::providers::gcp::clients::analyticshub::AnalyticshubProjectsLocationsDataExchangesQueryTemplatesSubmitArgs;
use crate::providers::gcp::clients::analyticshub::AnalyticshubProjectsLocationsDataExchangesSetIamPolicyArgs;
use crate::providers::gcp::clients::analyticshub::AnalyticshubProjectsLocationsDataExchangesSubscribeArgs;
use crate::providers::gcp::clients::analyticshub::AnalyticshubProjectsLocationsDataExchangesTestIamPermissionsArgs;
use crate::providers::gcp::clients::analyticshub::AnalyticshubProjectsLocationsSubscriptionsDeleteArgs;
use crate::providers::gcp::clients::analyticshub::AnalyticshubProjectsLocationsSubscriptionsGetArgs;
use crate::providers::gcp::clients::analyticshub::AnalyticshubProjectsLocationsSubscriptionsGetIamPolicyArgs;
use crate::providers::gcp::clients::analyticshub::AnalyticshubProjectsLocationsSubscriptionsListArgs;
use crate::providers::gcp::clients::analyticshub::AnalyticshubProjectsLocationsSubscriptionsRefreshArgs;
use crate::providers::gcp::clients::analyticshub::AnalyticshubProjectsLocationsSubscriptionsRevokeArgs;
use crate::providers::gcp::clients::analyticshub::AnalyticshubProjectsLocationsSubscriptionsSetIamPolicyArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// AnalyticshubProvider with automatic state tracking.
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
/// let provider = AnalyticshubProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct AnalyticshubProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> AnalyticshubProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new AnalyticshubProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Analyticshub organizations locations data exchanges list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListOrgDataExchangesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn analyticshub_organizations_locations_data_exchanges_list(
        &self,
        args: &AnalyticshubOrganizationsLocationsDataExchangesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOrgDataExchangesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticshub_organizations_locations_data_exchanges_list_builder(
            &self.http_client,
            &args.organization,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticshub_organizations_locations_data_exchanges_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticshub projects locations data exchanges create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DataExchange result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analyticshub_projects_locations_data_exchanges_create(
        &self,
        args: &AnalyticshubProjectsLocationsDataExchangesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DataExchange, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticshub_projects_locations_data_exchanges_create_builder(
            &self.http_client,
            &args.parent,
            &args.dataExchangeId,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticshub_projects_locations_data_exchanges_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticshub projects locations data exchanges delete.
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
    pub fn analyticshub_projects_locations_data_exchanges_delete(
        &self,
        args: &AnalyticshubProjectsLocationsDataExchangesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticshub_projects_locations_data_exchanges_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticshub_projects_locations_data_exchanges_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticshub projects locations data exchanges get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DataExchange result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn analyticshub_projects_locations_data_exchanges_get(
        &self,
        args: &AnalyticshubProjectsLocationsDataExchangesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DataExchange, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticshub_projects_locations_data_exchanges_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticshub_projects_locations_data_exchanges_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticshub projects locations data exchanges get iam policy.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn analyticshub_projects_locations_data_exchanges_get_iam_policy(
        &self,
        args: &AnalyticshubProjectsLocationsDataExchangesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticshub_projects_locations_data_exchanges_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticshub_projects_locations_data_exchanges_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticshub projects locations data exchanges list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDataExchangesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn analyticshub_projects_locations_data_exchanges_list(
        &self,
        args: &AnalyticshubProjectsLocationsDataExchangesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDataExchangesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticshub_projects_locations_data_exchanges_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticshub_projects_locations_data_exchanges_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticshub projects locations data exchanges list subscriptions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListSharedResourceSubscriptionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn analyticshub_projects_locations_data_exchanges_list_subscriptions(
        &self,
        args: &AnalyticshubProjectsLocationsDataExchangesListSubscriptionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSharedResourceSubscriptionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticshub_projects_locations_data_exchanges_list_subscriptions_builder(
            &self.http_client,
            &args.resource,
            &args.includeDeletedSubscriptions,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticshub_projects_locations_data_exchanges_list_subscriptions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticshub projects locations data exchanges patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DataExchange result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analyticshub_projects_locations_data_exchanges_patch(
        &self,
        args: &AnalyticshubProjectsLocationsDataExchangesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DataExchange, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticshub_projects_locations_data_exchanges_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticshub_projects_locations_data_exchanges_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticshub projects locations data exchanges set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analyticshub_projects_locations_data_exchanges_set_iam_policy(
        &self,
        args: &AnalyticshubProjectsLocationsDataExchangesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticshub_projects_locations_data_exchanges_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticshub_projects_locations_data_exchanges_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticshub projects locations data exchanges subscribe.
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
    pub fn analyticshub_projects_locations_data_exchanges_subscribe(
        &self,
        args: &AnalyticshubProjectsLocationsDataExchangesSubscribeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticshub_projects_locations_data_exchanges_subscribe_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticshub_projects_locations_data_exchanges_subscribe_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticshub projects locations data exchanges test iam permissions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn analyticshub_projects_locations_data_exchanges_test_iam_permissions(
        &self,
        args: &AnalyticshubProjectsLocationsDataExchangesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticshub_projects_locations_data_exchanges_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticshub_projects_locations_data_exchanges_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticshub projects locations data exchanges listings create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Listing result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analyticshub_projects_locations_data_exchanges_listings_create(
        &self,
        args: &AnalyticshubProjectsLocationsDataExchangesListingsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Listing, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticshub_projects_locations_data_exchanges_listings_create_builder(
            &self.http_client,
            &args.parent,
            &args.listingId,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticshub_projects_locations_data_exchanges_listings_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticshub projects locations data exchanges listings delete.
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
    pub fn analyticshub_projects_locations_data_exchanges_listings_delete(
        &self,
        args: &AnalyticshubProjectsLocationsDataExchangesListingsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticshub_projects_locations_data_exchanges_listings_delete_builder(
            &self.http_client,
            &args.name,
            &args.deleteCommercial,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticshub_projects_locations_data_exchanges_listings_delete_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticshub projects locations data exchanges listings get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Listing result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn analyticshub_projects_locations_data_exchanges_listings_get(
        &self,
        args: &AnalyticshubProjectsLocationsDataExchangesListingsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Listing, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticshub_projects_locations_data_exchanges_listings_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticshub_projects_locations_data_exchanges_listings_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticshub projects locations data exchanges listings get iam policy.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn analyticshub_projects_locations_data_exchanges_listings_get_iam_policy(
        &self,
        args: &AnalyticshubProjectsLocationsDataExchangesListingsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticshub_projects_locations_data_exchanges_listings_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticshub_projects_locations_data_exchanges_listings_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticshub projects locations data exchanges listings list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListListingsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn analyticshub_projects_locations_data_exchanges_listings_list(
        &self,
        args: &AnalyticshubProjectsLocationsDataExchangesListingsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListListingsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticshub_projects_locations_data_exchanges_listings_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticshub_projects_locations_data_exchanges_listings_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticshub projects locations data exchanges listings list subscriptions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListSharedResourceSubscriptionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn analyticshub_projects_locations_data_exchanges_listings_list_subscriptions(
        &self,
        args: &AnalyticshubProjectsLocationsDataExchangesListingsListSubscriptionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSharedResourceSubscriptionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticshub_projects_locations_data_exchanges_listings_list_subscriptions_builder(
            &self.http_client,
            &args.resource,
            &args.includeDeletedSubscriptions,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticshub_projects_locations_data_exchanges_listings_list_subscriptions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticshub projects locations data exchanges listings patch.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Listing result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn analyticshub_projects_locations_data_exchanges_listings_patch(
        &self,
        args: &AnalyticshubProjectsLocationsDataExchangesListingsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Listing, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticshub_projects_locations_data_exchanges_listings_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticshub_projects_locations_data_exchanges_listings_patch_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticshub projects locations data exchanges listings set iam policy.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn analyticshub_projects_locations_data_exchanges_listings_set_iam_policy(
        &self,
        args: &AnalyticshubProjectsLocationsDataExchangesListingsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticshub_projects_locations_data_exchanges_listings_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticshub_projects_locations_data_exchanges_listings_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticshub projects locations data exchanges listings subscribe.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SubscribeListingResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn analyticshub_projects_locations_data_exchanges_listings_subscribe(
        &self,
        args: &AnalyticshubProjectsLocationsDataExchangesListingsSubscribeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SubscribeListingResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticshub_projects_locations_data_exchanges_listings_subscribe_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticshub_projects_locations_data_exchanges_listings_subscribe_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticshub projects locations data exchanges listings test iam permissions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn analyticshub_projects_locations_data_exchanges_listings_test_iam_permissions(
        &self,
        args: &AnalyticshubProjectsLocationsDataExchangesListingsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticshub_projects_locations_data_exchanges_listings_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticshub_projects_locations_data_exchanges_listings_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticshub projects locations data exchanges query templates approve.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the QueryTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn analyticshub_projects_locations_data_exchanges_query_templates_approve(
        &self,
        args: &AnalyticshubProjectsLocationsDataExchangesQueryTemplatesApproveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<QueryTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticshub_projects_locations_data_exchanges_query_templates_approve_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticshub_projects_locations_data_exchanges_query_templates_approve_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticshub projects locations data exchanges query templates create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the QueryTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analyticshub_projects_locations_data_exchanges_query_templates_create(
        &self,
        args: &AnalyticshubProjectsLocationsDataExchangesQueryTemplatesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<QueryTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticshub_projects_locations_data_exchanges_query_templates_create_builder(
            &self.http_client,
            &args.parent,
            &args.queryTemplateId,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticshub_projects_locations_data_exchanges_query_templates_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticshub projects locations data exchanges query templates delete.
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
    pub fn analyticshub_projects_locations_data_exchanges_query_templates_delete(
        &self,
        args: &AnalyticshubProjectsLocationsDataExchangesQueryTemplatesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticshub_projects_locations_data_exchanges_query_templates_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticshub_projects_locations_data_exchanges_query_templates_delete_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticshub projects locations data exchanges query templates get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the QueryTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn analyticshub_projects_locations_data_exchanges_query_templates_get(
        &self,
        args: &AnalyticshubProjectsLocationsDataExchangesQueryTemplatesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<QueryTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticshub_projects_locations_data_exchanges_query_templates_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticshub_projects_locations_data_exchanges_query_templates_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticshub projects locations data exchanges query templates list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListQueryTemplatesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn analyticshub_projects_locations_data_exchanges_query_templates_list(
        &self,
        args: &AnalyticshubProjectsLocationsDataExchangesQueryTemplatesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListQueryTemplatesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticshub_projects_locations_data_exchanges_query_templates_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticshub_projects_locations_data_exchanges_query_templates_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticshub projects locations data exchanges query templates patch.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the QueryTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn analyticshub_projects_locations_data_exchanges_query_templates_patch(
        &self,
        args: &AnalyticshubProjectsLocationsDataExchangesQueryTemplatesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<QueryTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticshub_projects_locations_data_exchanges_query_templates_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticshub_projects_locations_data_exchanges_query_templates_patch_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticshub projects locations data exchanges query templates submit.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the QueryTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn analyticshub_projects_locations_data_exchanges_query_templates_submit(
        &self,
        args: &AnalyticshubProjectsLocationsDataExchangesQueryTemplatesSubmitArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<QueryTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticshub_projects_locations_data_exchanges_query_templates_submit_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticshub_projects_locations_data_exchanges_query_templates_submit_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticshub projects locations subscriptions delete.
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
    pub fn analyticshub_projects_locations_subscriptions_delete(
        &self,
        args: &AnalyticshubProjectsLocationsSubscriptionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticshub_projects_locations_subscriptions_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticshub_projects_locations_subscriptions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticshub projects locations subscriptions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Subscription result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn analyticshub_projects_locations_subscriptions_get(
        &self,
        args: &AnalyticshubProjectsLocationsSubscriptionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Subscription, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticshub_projects_locations_subscriptions_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticshub_projects_locations_subscriptions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticshub projects locations subscriptions get iam policy.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn analyticshub_projects_locations_subscriptions_get_iam_policy(
        &self,
        args: &AnalyticshubProjectsLocationsSubscriptionsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticshub_projects_locations_subscriptions_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticshub_projects_locations_subscriptions_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticshub projects locations subscriptions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListSubscriptionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn analyticshub_projects_locations_subscriptions_list(
        &self,
        args: &AnalyticshubProjectsLocationsSubscriptionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSubscriptionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticshub_projects_locations_subscriptions_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticshub_projects_locations_subscriptions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticshub projects locations subscriptions refresh.
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
    pub fn analyticshub_projects_locations_subscriptions_refresh(
        &self,
        args: &AnalyticshubProjectsLocationsSubscriptionsRefreshArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticshub_projects_locations_subscriptions_refresh_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticshub_projects_locations_subscriptions_refresh_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticshub projects locations subscriptions revoke.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RevokeSubscriptionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analyticshub_projects_locations_subscriptions_revoke(
        &self,
        args: &AnalyticshubProjectsLocationsSubscriptionsRevokeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RevokeSubscriptionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticshub_projects_locations_subscriptions_revoke_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticshub_projects_locations_subscriptions_revoke_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticshub projects locations subscriptions set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analyticshub_projects_locations_subscriptions_set_iam_policy(
        &self,
        args: &AnalyticshubProjectsLocationsSubscriptionsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticshub_projects_locations_subscriptions_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticshub_projects_locations_subscriptions_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
